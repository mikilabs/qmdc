/**
 * Data-driven CLI conformance tests (shared corpus in tests/cli/).
 *
 * Every parser runs the same corpus so the `cli` suite reaches parity by
 * construction. Impl-specific CLI tests live in test-cli.ts (-> unit-ts).
 * See tests/cli/README.md for the fixture format.
 */
import { readFileSync, readdirSync, existsSync } from 'fs';
import { dirname, join, resolve } from 'path';
import { fileURLToPath } from 'url';
import { spawnSync } from 'child_process';
import { CaseReport } from './_report.js';

const __dirname = dirname(fileURLToPath(import.meta.url));
const CORPUS = resolve(__dirname, '../../tests/cli');
const CLI = resolve(__dirname, '../qmdc'); // ts CLI wrapper

// Order-insensitive object-key comparison (matches the py dict / rust serde semantics).
function deepEqual(a: unknown, b: unknown): boolean {
  if (a === b) return true;
  if (Array.isArray(a) && Array.isArray(b)) {
    return a.length === b.length && a.every((x, i) => deepEqual(x, b[i]));
  }
  if (a && b && typeof a === 'object' && typeof b === 'object') {
    const ka = Object.keys(a as object);
    const kb = Object.keys(b as object);
    if (ka.length !== kb.length) return false;
    return ka.every((k) =>
      deepEqual((a as Record<string, unknown>)[k], (b as Record<string, unknown>)[k])
    );
  }
  return false;
}

function runTests() {
  const cases = existsSync(CORPUS)
    ? readdirSync(CORPUS, { withFileTypes: true })
        .filter((e) => e.isDirectory() && existsSync(join(CORPUS, e.name, 'cmd')))
        .map((e) => e.name)
        .sort()
    : [];

  const report = new CaseReport('ts-cliconf');

  for (const name of cases) {
    const t0 = performance.now();
    const dir = join(CORPUS, name);
    const args = readFileSync(join(dir, 'cmd'), 'utf-8').trim().split(/\s+/);
    const stdin = existsSync(join(dir, 'stdin'))
      ? readFileSync(join(dir, 'stdin'), 'utf-8')
      : undefined;
    const exitExpected = existsSync(join(dir, 'exit'))
      ? parseInt(readFileSync(join(dir, 'exit'), 'utf-8').trim(), 10)
      : 0;

    const result = spawnSync(CLI, args, { cwd: dir, input: stdin, encoding: 'utf-8' });
    const actualExit = result.status ?? -1;

    let problem: string | null = null;
    if (actualExit !== exitExpected) {
      problem = `exit ${actualExit} != expected ${exitExpected}`;
    }

    const expJson = join(dir, 'expected.json');
    const expTxt = join(dir, 'expected.txt');
    if (!problem && existsSync(expJson)) {
      const expected = JSON.parse(readFileSync(expJson, 'utf-8'));
      try {
        const actual = JSON.parse(result.stdout);
        if (!deepEqual(actual, expected)) problem = 'stdout JSON mismatch';
      } catch (e) {
        problem = `stdout not JSON: ${e instanceof Error ? e.message : e}`;
      }
    } else if (!problem && existsSync(expTxt)) {
      if (
        result.stdout.replace(/\r\n/g, '\n').trim() !==
        readFileSync(expTxt, 'utf-8').replace(/\r\n/g, '\n').trim()
      ) {
        problem = 'stdout text mismatch';
      }
    }

    const sec = (performance.now() - t0) / 1000;
    if (problem) {
      console.log(`  ✗ ${name}: ${problem}`);
      console.log(`    stdout: ${result.stdout}`);
      report.fail(name, problem, sec);
    } else {
      console.log(`  ✓ ${name}`);
      report.pass(name, sec);
    }
  }

  report.finish();
}

runTests();
