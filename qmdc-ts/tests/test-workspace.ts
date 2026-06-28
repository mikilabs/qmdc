/**
 * Data-driven tests for QMD workspace functionality.
 *
 * Tests are auto-discovered from workspace directories containing _expected.json.
 *
 * Format of _expected.json:
 * {
 *   "workspace_id": "my_workspace",
 *   "files": ["readme.qmd.md", "other.qmd.md"],
 *   "objects": {
 *     "Kind": ["id1", "id2"],
 *     "__Workspace": ["my_workspace"]
 *   },
 *   "errors": [
 *     {"type": "broken_link", "object": "obj_id", "reference": "[[#ref]]"}
 *   ]
 * }
 */

import { readFileSync, readdirSync, existsSync } from 'fs';
import { dirname, join } from 'path';
import { fileURLToPath } from 'url';
import { parseWorkspace, scanWorkspace } from '../src/workspace.js';
import { CaseReport } from './_report.js';

const __dirname = dirname(fileURLToPath(import.meta.url));
const WORKSPACE_TESTS_ROOT = join(__dirname, '../../tests/workspace');

interface ExpectedError {
  type: string;
  object?: string;
  reference?: string;
  file?: string;
  line?: number;
  candidates?: string[];
}

interface NestedWorkspaceExpected {
  workspace_id: string;
  root: string;
  files: string[];
  objects: Record<string, string[]>;
  errors: ExpectedError[];
}

interface ExpectedConfig {
  workspace_id: string;
  files: string[];
  objects: Record<string, string[]>;
  errors: ExpectedError[];
  nested_workspaces?: NestedWorkspaceExpected[];
}

interface WorkspaceTest {
  name: string;
  path: string;
  expected: ExpectedConfig;
}

function findWorkspaceTests(rootDir: string, prefix = ''): WorkspaceTest[] {
  const tests: WorkspaceTest[] = [];
  if (!existsSync(rootDir)) return tests;

  const entries = readdirSync(rootDir, { withFileTypes: true });
  for (const entry of entries.sort((a, b) => a.name.localeCompare(b.name))) {
    if (!entry.isDirectory()) continue;

    const dirPath = join(rootDir, entry.name);
    const expectedFile = join(dirPath, '_expected.json');
    const readmeFile = join(dirPath, 'readme.qmd.md');

    if (existsSync(expectedFile) && existsSync(readmeFile)) {
      const testName = prefix ? `${prefix}${entry.name}` : entry.name;
      const expected = JSON.parse(readFileSync(expectedFile, 'utf-8'));
      tests.push({ name: testName, path: dirPath, expected });
    } else {
      const newPrefix = prefix ? `${prefix}${entry.name}/` : `${entry.name}/`;
      tests.push(...findWorkspaceTests(dirPath, newPrefix));
    }
  }
  return tests;
}

async function runTests() {
  const workspaceTests = findWorkspaceTests(WORKSPACE_TESTS_ROOT);

  if (workspaceTests.length === 0) {
    console.log('No workspace tests found!');
    process.exit(1);
  }

  const report = new CaseReport('ts-workspace');

  for (const test of workspaceTests) {
    console.log(`\nWorkspace: ${test.name}`);

    // Test: workspace_id (the per-fixture parse cost lands on this case)
    report.time(`${test.name}/workspace_id`, () => {
      const result = parseWorkspace(test.path);
      if (result.workspaceId !== test.expected.workspace_id) {
        throw new Error(`expected '${test.expected.workspace_id}', got '${result.workspaceId}'`);
      }
    });

    // Test: files
    report.time(`${test.name}/files`, () => {
      const files = scanWorkspace(test.path);
      const actualSorted = [...files].sort();
      const expectedSorted = [...test.expected.files].sort();
      if (JSON.stringify(actualSorted) !== JSON.stringify(expectedSorted)) {
        throw new Error(
          `files mismatch\n    expected: ${JSON.stringify(expectedSorted)}\n    got:      ${JSON.stringify(actualSorted)}`
        );
      }
    });

    // Test: objects_by_kind
    report.time(`${test.name}/objects_by_kind`, () => {
      const result = parseWorkspace(test.path);
      const actual: Record<string, string[]> = {};
      for (const obj of result.objects) {
        const kind = (obj.__kind as string) || '';
        if (!actual[kind]) actual[kind] = [];
        actual[kind].push(obj.__id);
      }
      for (const k in actual) {
        actual[k] = actual[k].sort();
      }
      const actualSortedKeys = Object.fromEntries(
        Object.keys(actual)
          .sort()
          .map((k) => [k, actual[k]])
      );
      const expected: Record<string, string[]> = {};
      for (const [k, v] of Object.entries(test.expected.objects)) {
        expected[k] = [...v].sort();
      }
      const expectedSortedKeys = Object.fromEntries(
        Object.keys(expected)
          .sort()
          .map((k) => [k, expected[k]])
      );
      if (JSON.stringify(actualSortedKeys) !== JSON.stringify(expectedSortedKeys)) {
        const allKinds = new Set([...Object.keys(actual), ...Object.keys(expected)]);
        const diffs: string[] = [];
        for (const kind of allKinds) {
          const a = actual[kind] || [];
          const e = expected[kind] || [];
          if (JSON.stringify(a) !== JSON.stringify(e)) {
            diffs.push(`kind '${kind}': expected ${JSON.stringify(e)}, got ${JSON.stringify(a)}`);
          }
        }
        throw new Error(`objects_by_kind mismatch\n    ${diffs.join('\n    ')}`);
      }
    });

    // Test: errors (skipped-but-counted when the fixture declares none)
    if (!test.expected.errors) {
      report.skip(`${test.name}/errors`);
    } else {
      report.time(`${test.name}/errors`, () => {
        const result = parseWorkspace(test.path);
        const actualErrors: ExpectedError[] = result.errors.map((e) => {
          const err: ExpectedError = { type: e.type };
          if (e.objectId) err.object = e.objectId;
          if (e.reference) err.reference = e.reference;
          if (e.file) err.file = e.file;
          if (e.line) err.line = e.line;
          if (e.candidates) err.candidates = e.candidates;
          return err;
        });
        const sortKey = (x: ExpectedError) =>
          `${x.type}|${x.object}|${x.reference || ''}|${x.file || ''}|${x.line || ''}|${(x.candidates || []).join(',')}`;
        const actualSorted = [...actualErrors].sort((a, b) => sortKey(a).localeCompare(sortKey(b)));
        const expectedSorted = [...test.expected.errors].sort((a, b) =>
          sortKey(a).localeCompare(sortKey(b))
        );
        if (JSON.stringify(actualSorted) !== JSON.stringify(expectedSorted)) {
          throw new Error(
            `errors mismatch\n    expected: ${JSON.stringify(expectedSorted, null, 2)}\n    got:      ${JSON.stringify(actualSorted, null, 2)}`
          );
        }
      });
    }

    // Test: objects_have_metadata
    report.time(`${test.name}/objects_have_metadata`, () => {
      const result = parseWorkspace(test.path);
      for (const obj of result.objects) {
        const kind = (obj.__kind as string) || '';
        if (kind.startsWith('__') && kind !== '__Workspace' && kind !== '__Namespace') {
          continue;
        }
        if (typeof obj.__file !== 'string' || typeof obj.__line !== 'number') {
          throw new Error(`object '${obj.__id}' missing __file or __line`);
        }
      }
    });
  }

  report.finish();
}

runTests();
