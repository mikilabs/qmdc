/**
 * Data-driven SQL tests for QMD workspace.
 *
 * Automatically discovers all directories with tests/ subdirectory containing .sql files.
 */

import { readFileSync, readdirSync, existsSync, statSync } from 'fs';
import { dirname, join } from 'path';
import { fileURLToPath } from 'url';
import { QmdcDatabase } from '../src/db.js';
import { parseAllWorkspaces } from '../src/workspace.js';
import { CaseReport } from './_report.js';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

// Path to scan for test workspaces
const SCAN_PATHS = [join(__dirname, '../../tests/workspace')];

interface ExpectedResult {
  columns: string[];
  rows: (string | number | boolean | null)[][];
}

interface SqlTest {
  name: string;
  workspaceName: string;
  workspacePath: string;
  sqlFile: string;
  expectedFile: string;
}

/** Get SQL tests from a workspace directory */
function getSqlTests(workspacePath: string, workspaceName: string): SqlTest[] {
  const testsDir = join(workspacePath, 'tests');
  const tests: SqlTest[] = [];

  if (!existsSync(testsDir)) {
    return tests;
  }

  const sqlFiles = readdirSync(testsDir)
    .filter((f) => f.endsWith('.sql'))
    .sort();

  for (const sqlFile of sqlFiles) {
    const name = sqlFile.replace('.sql', '');
    const expectedFile = join(testsDir, `${name}.expected.json`);

    if (existsSync(expectedFile)) {
      tests.push({
        name,
        workspaceName,
        workspacePath,
        sqlFile: join(testsDir, sqlFile),
        expectedFile,
      });
    }
  }

  return tests;
}

/** Recursively find all directories containing tests/ with .sql files */
function findTestWorkspaces(dir: string, prefix: string = ''): SqlTest[] {
  const tests: SqlTest[] = [];

  if (!existsSync(dir)) {
    return tests;
  }

  const entries = readdirSync(dir);

  for (const entry of entries) {
    const fullPath = join(dir, entry);

    if (!statSync(fullPath).isDirectory()) {
      continue;
    }

    const testsDir = join(fullPath, 'tests');
    const workspaceName = prefix ? `${prefix}/${entry}` : entry;

    // Check if this directory has tests/
    if (existsSync(testsDir) && statSync(testsDir).isDirectory()) {
      const sqlFiles = readdirSync(testsDir).filter((f) => f.endsWith('.sql'));
      if (sqlFiles.length > 0) {
        tests.push(...getSqlTests(fullPath, workspaceName));
      }
    }

    // Also check subdirectories (but not tests/ itself)
    if (entry !== 'tests') {
      tests.push(...findTestWorkspaces(fullPath, workspaceName));
    }
  }

  return tests;
}

/** Collect all tests from all scan paths */
function collectAllTests(): SqlTest[] {
  const tests: SqlTest[] = [];

  for (const scanPath of SCAN_PATHS) {
    tests.push(...findTestWorkspaces(scanPath));
  }

  return tests.sort((a, b) => a.workspaceName.localeCompare(b.workspaceName));
}

async function runTests(report: CaseReport) {
  const allTests = collectAllTests();

  // Group by workspace
  const byWorkspace = new Map<string, SqlTest[]>();
  for (const test of allTests) {
    const key = test.workspaceName;
    if (!byWorkspace.has(key)) {
      byWorkspace.set(key, []);
    }
    byWorkspace.get(key)!.push(test);
  }

  for (const [workspaceName, tests] of byWorkspace) {
    console.log(`\nSQL Tests: ${workspaceName} (${tests.length} tests)`);

    // Load workspace
    const workspacePath = tests[0].workspacePath;
    const result = parseAllWorkspaces(workspacePath);
    const allObjects = result.objects;

    // Create database and sync
    const db = await QmdcDatabase.create();
    db.syncObjects(allObjects);

    for (const test of tests) {
      report.time(`${workspaceName}/${test.name}`, () => {
        const sql = readFileSync(test.sqlFile, 'utf-8').trim();
        const expected: ExpectedResult = JSON.parse(readFileSync(test.expectedFile, 'utf-8'));
        const result = db.query(sql);

        // Compare columns
        if (JSON.stringify(result.columns) !== JSON.stringify(expected.columns)) {
          throw new Error(
            `Columns mismatch: expected ${JSON.stringify(expected.columns)}, got ${JSON.stringify(result.columns)}`
          );
        }

        // Compare rows
        if (JSON.stringify(result.rows) !== JSON.stringify(expected.rows)) {
          throw new Error(
            `Rows mismatch:\nexpected: ${JSON.stringify(expected.rows)}\ngot: ${JSON.stringify(result.rows)}`
          );
        }
      });
    }
  }
}

function testParseAllWorkspaces(report: CaseReport) {
  const artifactsPath = SCAN_PATHS[0];
  console.log('\n=== Testing parseAllWorkspaces ===');
  console.log(`Test directory: ${artifactsPath}`);

  report.time('parse_all_workspaces', () => {
    const result = parseAllWorkspaces(artifactsPath);
    const workspaceCount = result.objects.filter((obj) => obj.__kind === '__Workspace').length;
    console.log(`Found ${workspaceCount} workspace objects`);
    if (workspaceCount < 3) {
      throw new Error(`Expected at least 3 workspaces, found ${workspaceCount}`);
    }
    const workspaceIds = result.objects
      .filter((obj) => obj.__kind === '__Workspace' && obj.__id)
      .map((obj) => obj.__id);
    console.log(`Workspace IDs: ${workspaceIds.join(', ')}`);
    for (const expected of ['ecommerce', 'backend', 'frontend']) {
      if (!workspaceIds.includes(expected)) {
        throw new Error(`Should find '${expected}' workspace`);
      }
    }
  });
}

async function runAllTests() {
  const report = new CaseReport('ts-sql');
  testParseAllWorkspaces(report);
  await runTests(report);
  report.finish();
}

runAllTests().catch((error) => {
  console.error('Test execution failed:', error);
  process.exit(1);
});
