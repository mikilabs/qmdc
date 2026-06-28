/**
 * Test QMD parser against microtests
 */

import { readFileSync, existsSync, readdirSync } from 'fs';
import { resolve, dirname } from 'path';
import { fileURLToPath } from 'url';
import { parse, rebuild, type ParseResult } from '../src/parser.js';
import { CaseReport } from './_report.js';

const __dirname = dirname(fileURLToPath(import.meta.url));

type ParseFormat = 'minimal' | 'standard' | 'full';

/**
 * Deep equality comparison (order-sensitive for both arrays AND object keys).
 * Order matters for rebuild to preserve field order in documents.
 */
function deepEqual(a: unknown, b: unknown): boolean {
  if (a === b) return true;
  if (typeof a !== typeof b) return false;
  if (a === null || b === null) return a === b;

  if (Array.isArray(a) && Array.isArray(b)) {
    if (a.length !== b.length) return false;
    return a.every((val, i) => deepEqual(val, b[i]));
  }

  if (typeof a === 'object' && typeof b === 'object') {
    const aObj = a as Record<string, unknown>;
    const bObj = b as Record<string, unknown>;
    const aKeys = Object.keys(aObj);
    const bKeys = Object.keys(bObj);
    // Check same keys in same order
    if (aKeys.length !== bKeys.length) return false;
    if (!aKeys.every((key, i) => key === bKeys[i])) return false;
    return aKeys.every((key) => deepEqual(aObj[key], bObj[key]));
  }

  return false;
}

/**
 * Normalize a string for content-loss comparison.
 * Strips [[...]] brackets, quotes, HTML comments, heading markers, and whitespace.
 */
function normalizeForContentComparison(s: string): string {
  const chars = [...s];
  const len = chars.length;
  const result: string[] = [];
  let i = 0;

  while (i < len) {
    // Skip HTML comments <!-- ... -->
    if (
      i + 3 < len &&
      chars[i] === '<' &&
      chars[i + 1] === '!' &&
      chars[i + 2] === '-' &&
      chars[i + 3] === '-'
    ) {
      let j = i + 4;
      let found = false;
      while (j + 2 < len) {
        if (chars[j] === '-' && chars[j + 1] === '-' && chars[j + 2] === '>') {
          j += 3;
          found = true;
          break;
        }
        j++;
      }
      if (!found) {
        j = len;
      }
      i = j;
      continue;
    }

    // Skip [[...]] bracket tokens
    if (i + 1 < len && chars[i] === '[' && chars[i + 1] === '[') {
      let j = i + 2;
      let depth = 1;
      while (j < len && depth > 0) {
        if (j + 1 < len && chars[j] === '[' && chars[j + 1] === '[') {
          depth++;
          j += 2;
        } else if (j + 1 < len && chars[j] === ']' && chars[j + 1] === ']') {
          depth--;
          j += 2;
        } else {
          j++;
        }
      }
      i = j;
      continue;
    }

    // Skip quotes
    if (chars[i] === '"') {
      i++;
      continue;
    }

    result.push(chars[i]);
    i++;
  }

  // Strip heading markers, normalize table separators, and strip all whitespace
  let text = result
    .join('')
    .split('\n')
    .map((line) => line.replace(/^#+/, ''))
    .join('\n');

  // Normalize table separator rows (|---|---|, |-------|-----|, etc.) to just pipes
  text = text.replace(/\|[-:]+(?:\|[-:]+)*\|/g, (m) => '|'.repeat((m.match(/\|/g) || []).length));

  return text.replace(/\s/g, '');
}

/**
 * Check if difference between original and rebuilt is normalization-only.
 * Returns list of problem descriptions. Empty means no content loss.
 */
function checkContentLoss(original: string, rebuilt: string): string[] {
  const origLines = original.split('\n');
  const rebuiltLines = rebuilt.split('\n');
  const problems: string[] = [];

  // Check 1: Heading level changes (positional matching)
  const getHeadings = (lines: string[]) =>
    lines
      .filter((l) => l.startsWith('#'))
      .map((l) => {
        const level = l.match(/^#+/)![0].length;
        const label = l.replace(/^#+\s*/, '');
        return { level, label };
      });

  const origHeadings = getHeadings(origLines);
  const rebuiltHeadings = getHeadings(rebuiltLines);

  const headingCount = Math.min(origHeadings.length, rebuiltHeadings.length);
  for (let idx = 0; idx < headingCount; idx++) {
    const oh = origHeadings[idx];
    const rh = rebuiltHeadings[idx];
    const ol = normalizeForContentComparison(oh.label);
    const rl = normalizeForContentComparison(rh.label);
    if (ol === rl && ol && oh.level !== rh.level) {
      problems.push(
        `  HEADING LEVEL CHANGE: "${oh.label}" was h${oh.level}, now h${rh.level} ("${rh.label}")`
      );
    }
  }

  // Check 2: Content loss via LCS diff
  const n = origLines.length;
  const m = rebuiltLines.length;
  const dp: number[][] = Array.from({ length: n + 1 }, () => new Array(m + 1).fill(0));
  for (let i = 1; i <= n; i++) {
    for (let j = 1; j <= m; j++) {
      if (origLines[i - 1] === rebuiltLines[j - 1]) {
        dp[i][j] = dp[i - 1][j - 1] + 1;
      } else {
        dp[i][j] = Math.max(dp[i - 1][j], dp[i][j - 1]);
      }
    }
  }

  // Backtrack
  type DiffOp =
    | { type: 'equal' }
    | { type: 'removed'; line: string }
    | { type: 'added'; line: string };
  const ops: DiffOp[] = [];
  let i = n,
    j = m;
  while (i > 0 || j > 0) {
    if (i > 0 && j > 0 && origLines[i - 1] === rebuiltLines[j - 1]) {
      ops.push({ type: 'equal' });
      i--;
      j--;
    } else if (i > 0 && (j === 0 || dp[i - 1][j] >= dp[i][j - 1])) {
      ops.push({ type: 'removed', line: origLines[i - 1] });
      i--;
    } else {
      ops.push({ type: 'added', line: rebuiltLines[j - 1] });
      j--;
    }
  }
  ops.reverse();

  // Group into hunks
  const hunks: { removed: string[]; added: string[] }[] = [];
  let curRemoved: string[] = [];
  let curAdded: string[] = [];
  for (const op of ops) {
    if (op.type === 'equal') {
      if (curRemoved.length || curAdded.length) {
        hunks.push({ removed: [...curRemoved], added: [...curAdded] });
        curRemoved = [];
        curAdded = [];
      }
    } else if (op.type === 'removed') {
      curRemoved.push(op.line);
    } else {
      curAdded.push(op.line);
    }
  }
  if (curRemoved.length || curAdded.length) {
    hunks.push({ removed: curRemoved, added: curAdded });
  }

  for (const { removed, added } of hunks) {
    const removedText = removed.join('\n');
    const addedText = added.join('\n');
    const rn = normalizeForContentComparison(removedText);
    const an = normalizeForContentComparison(addedText);
    if (rn !== an) {
      problems.push(
        `  CONTENT LOSS:\n    REMOVED: ${JSON.stringify(removedText)}\n    ADDED:   ${JSON.stringify(addedText)}` +
          `\n    (normalized: ${JSON.stringify(rn)} vs ${JSON.stringify(an)})`
      );
    }
  }

  return problems;
}

const MICROTESTS_DIR = resolve(__dirname, '../../tests/parser');

interface TestCase {
  name: string;
  qmdFile: string;
  expectedFile: string;
  format: ParseFormat;
}

async function getMicrotestFiles(): Promise<TestCase[]> {
  const fs = await import('fs');
  const files = fs
    .readdirSync(MICROTESTS_DIR)
    .filter((f: string) => f.endsWith('.qmd.md'))
    .sort();

  const tests: TestCase[] = [];

  for (const file of files) {
    const qmdFile = resolve(MICROTESTS_DIR, file);
    const baseName = file.replace('.qmd.md', '');

    // Check for format-specific expected files
    const formats: { suffix: string; format: ParseFormat }[] = [
      { suffix: '.expected.json', format: 'standard' },
      { suffix: '.expected.minimal.json', format: 'minimal' },
      { suffix: '.expected.full.json', format: 'full' },
    ];

    for (const { suffix, format } of formats) {
      const expectedFile = resolve(MICROTESTS_DIR, baseName + suffix);
      if (existsSync(expectedFile)) {
        tests.push({
          name: format === 'standard' ? baseName : `${baseName} [${format}]`,
          qmdFile,
          expectedFile,
          format,
        });
      }
    }
  }

  return tests;
}

async function runTests() {
  let tests = await getMicrotestFiles();
  const report = new CaseReport('ts-parser');

  // Filter by MICROTEST_FILTER env var or command line argument
  const filter = process.env.MICROTEST_FILTER || process.argv[2];
  if (filter) {
    tests = tests.filter((t) => t.name.includes(filter));
    console.log(`Filtering tests by "${filter}": ${tests.length} tests\n`);
  }

  // Test parsing
  for (const test of tests) {
    report.time(test.name, () => {
      const markdown = readFileSync(test.qmdFile, 'utf-8');
      const result = parse(markdown, { randomSeed: 666, format: test.format });
      const expected = JSON.parse(readFileSync(test.expectedFile, 'utf-8'));
      if (!deepEqual(result, expected)) {
        throw new Error(
          `Objects mismatch\nExpected: ${JSON.stringify(expected, null, 2)}\nGot: ${JSON.stringify(result, null, 2)}`
        );
      }
    });
  }

  // Test rebuild (only for standard format - minimal/full don't support rebuild)
  let standardTests = tests.filter((t) => t.format === 'standard');

  // Normalize for rebuild comparison - skip ParsingErrors entirely (they can't survive round-trip)
  function normalizeForRebuildComparison(data: ParseResult): ParseResult {
    return data.filter((obj) => obj.__kind !== '__ParsingError');
  }

  for (const test of standardTests) {
    const originalMarkdown = readFileSync(test.qmdFile, 'utf-8');
    const json1 = parse(originalMarkdown);
    // Skip if parsing errors - rebuild of invalid docs is undefined
    if (json1.some((obj: any) => obj.__kind === '__ParsingError')) {
      report.skip(`${test.name}/rebuild`);
      continue;
    }
    report.time(`${test.name}/rebuild`, () => {
      const canonical = rebuild(json1);
      const json2 = parse(canonical);
      const json1Normalized = normalizeForRebuildComparison(json1);
      const json2Normalized = normalizeForRebuildComparison(json2);
      if (!deepEqual(json1Normalized, json2Normalized)) {
        throw new Error(
          `Data loss in round-trip\n=== ORIGINAL MD ===\n${originalMarkdown}\n=== CANONICAL MD ===\n${canonical}`
        );
      }
    });
  }

  // Test text-level round-trip (content loss detection)
  const allQmdFiles = readdirSync(MICROTESTS_DIR)
    .filter((f: string) => f.endsWith('.qmd.md'))
    .sort()
    .map((f: string) => ({
      name: f.replace('.qmd.md', ''),
      qmdFile: resolve(MICROTESTS_DIR, f),
    }))
    .filter((t: { name: string }) => !filter || t.name.includes(filter));

  for (const test of allQmdFiles) {
    const markdown = readFileSync(test.qmdFile, 'utf-8');
    const parsed = parse(markdown);
    // Skip if parsing errors
    if (parsed.some((obj) => obj.__kind === '__ParsingError')) {
      report.skip(`${test.name}/text`);
      continue;
    }
    report.time(`${test.name}/text`, () => {
      const rebuiltText = rebuild(parsed);
      const problems = checkContentLoss(markdown, rebuiltText);
      if (problems.length > 0) {
        throw new Error(`Content loss detected\n${problems.join('\n')}`);
      }
    });
  }

  report.finish();
}

runTests();
