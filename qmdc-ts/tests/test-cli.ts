/**
 * Test CLI commands
 */

import { execSync } from 'child_process';
import { readFileSync, writeFileSync, existsSync, mkdtempSync, rmSync } from 'fs';
import { resolve, dirname } from 'path';
import { fileURLToPath } from 'url';
import { tmpdir } from 'os';
import { CaseReport } from './_report.js';

const report = new CaseReport('ts-cli');

/** Thrown by a test body to record a skip rather than a pass/fail. */
class SkipError extends Error {}

const __dirname = dirname(fileURLToPath(import.meta.url));
const MICROTESTS_DIR = resolve(__dirname, '../../tests/parser');

function runCLI(
  args: string[],
  input?: string
): { stdout: string; stderr: string; status: number } {
  try {
    const cmd = `./qmdc ${args.join(' ')}`;
    const stdout = execSync(cmd, {
      cwd: resolve(__dirname, '..'),
      encoding: 'utf-8',
      input,
    });
    return { stdout, stderr: '', status: 0 };
  } catch (error: unknown) {
    const err = error as { stdout?: string; stderr?: string; status?: number };
    return {
      stdout: err.stdout || '',
      stderr: err.stderr || '',
      status: err.status || 1,
    };
  }
}

function test(name: string, fn: () => void) {
  console.log(`Testing ${name}...`);
  const t0 = performance.now();
  try {
    fn();
    console.log('  ✓ PASS');
    report.pass(name, (performance.now() - t0) / 1000);
  } catch (error) {
    const sec = (performance.now() - t0) / 1000;
    if (error instanceof SkipError) {
      console.log('  ⚠ SKIP:', error.message);
      report.skip(name, sec);
      return;
    }
    const message = error instanceof Error ? error.message : String(error);
    console.log('  ✗ FAIL:', message);
    report.fail(name, message, sec);
  }
}

function assert(condition: boolean, message: string) {
  if (!condition) throw new Error(message);
}

// Test 1: CLI parse with stdin
test('CLI parse with stdin', () => {
  const result = runCLI(['parse'], '## Test [[test]]');
  assert(result.status === 0, 'Should exit with code 0');

  const output = JSON.parse(result.stdout);
  assert(Array.isArray(output), 'Output must be an array');
  assert(output.length === 1, 'Must be exactly one object');
  assert(output[0].__id === 'test', 'Should parse object ID');
});

// Test 2: CLI parse with file input
test('CLI parse with file input', () => {
  const qmdFile = resolve(MICROTESTS_DIR, '001-empty-object.qmd.md');
  const result = runCLI(['parse', '-i', qmdFile]);

  assert(result.status === 0, 'Should exit with code 0');

  const output = JSON.parse(result.stdout);
  assert(Array.isArray(output), 'Output must be an array');
  // 001-empty-object.qmd.md returns Document + TextBlock
  assert(output.length === 2, 'Must be two objects (Document + TextBlock)');
  assert(output[0].__kind === '__Document', 'First should be __Document');
  assert(output[1].__kind === '__TextBlock', 'Second should be __TextBlock');
});

// Test 3: CLI parse with file output
test('CLI parse with file output', () => {
  const tmpDir = mkdtempSync(resolve(tmpdir(), 'qmdc-test-'));
  const outputFile = resolve(tmpDir, 'output.json');

  try {
    const result = runCLI(['parse', '-o', outputFile], '## Test [[test]]');
    assert(result.status === 0, 'Should exit with code 0');
    assert(existsSync(outputFile), 'Should create output file');

    const output = JSON.parse(readFileSync(outputFile, 'utf-8'));
    assert(Array.isArray(output), 'Output must be an array');
    assert(output.length === 1, 'Must be exactly one object');
    assert(output[0].__id === 'test', 'Should parse object ID');
  } finally {
    rmSync(tmpDir, { recursive: true, force: true });
  }
});

// Test 4: CLI parse with --no-comments
test('CLI parse with --no-comments', () => {
  // Use input that generates __comments
  const result = runCLI(
    ['parse', '--no-comments'],
    '## Test [[test]]\n\n- name: Alice\n\nThis is a comment after field.'
  );

  assert(result.status === 0, 'Should exit with code 0');

  const output = JSON.parse(result.stdout);
  assert(Array.isArray(output), 'Output must be an array');
  // Check that __comments is removed from all objects
  for (const obj of output) {
    assert(!('__comments' in obj), `Object ${obj.__id} should not have __comments`);
  }
});

// Test 5: CLI parse with --no-pretty (compact JSON)
test('CLI parse with --no-pretty', () => {
  const result = runCLI(['parse', '--no-pretty'], '## Test [[test]]');

  assert(result.status === 0, 'Should exit with code 0');

  // Should be valid JSON and parse correctly
  const output = JSON.parse(result.stdout);
  assert(Array.isArray(output), 'Output must be an array');
  assert(output.length === 1, 'Must be exactly one object');
  assert(output[0].__id === 'test', 'Should parse correctly');
});

// Test 6: Multiple microtests via CLI
test('Multiple microtests via CLI', () => {
  const testFiles = ['001', '002', '003', '004', '005'];

  for (const num of testFiles) {
    const qmdFile = resolve(MICROTESTS_DIR, `${num}-*.qmd.md`);
    const matches = execSync(`ls ${qmdFile.replace('*', '*')}`, {
      encoding: 'utf-8',
      cwd: MICROTESTS_DIR,
    }).trim();

    if (matches) {
      const file = matches.split('\n')[0];
      const result = runCLI(['parse', '-i', resolve(MICROTESTS_DIR, file)]);
      assert(result.status === 0, `Test ${num} should succeed`);

      const output = JSON.parse(result.stdout);
      assert(Array.isArray(output), 'Output must be an array');
      assert(output.length > 0, `Test ${num} should have objects`);
    }
  }
});

// Test 7: rebuild - verify round-trip
test('CLI rebuild with stdin', () => {
  const result = runCLI(['rebuild'], '[{"__id": "test", "__label": "Test", "__level": 2}]');

  assert(result.status === 0, `rebuild failed! stderr: ${result.stderr}`);
  assert(
    result.stdout.includes('## Test [[test]]') || result.stdout.includes('# Test [[test]]'),
    'rebuild should generate QMD output'
  );
});

// Test 8: workspace validate - verify it returns a JSON array of errors
test('CLI workspace validate returns JSON array', () => {
  const workspaceTestsRoot = resolve(__dirname, '../../tests/workspace');

  // Find first workspace test
  const testDirs = execSync(`find ${workspaceTestsRoot} -name "_expected.json" -type f | head -1`, {
    encoding: 'utf-8',
    cwd: workspaceTestsRoot,
  }).trim();

  if (!testDirs) {
    throw new SkipError('No workspace tests found');
  }

  const workspacePath = dirname(testDirs);

  // Get errors from workspace parse
  const parseResult = runCLI(['workspace', 'parse', workspacePath]);
  const parseOutput = JSON.parse(parseResult.stdout);
  const parseErrors = parseOutput.errors || [];

  // Get errors from workspace validate
  const validateResult = runCLI(['workspace', 'validate', workspacePath]);

  // Validate should return JSON array directly (not wrapped in object)
  const validateErrors = JSON.parse(validateResult.stdout);
  assert(Array.isArray(validateErrors), 'validate should return array');

  // Check that validate returns same number of errors as parse
  assert(
    validateErrors.length === parseErrors.length,
    `validate returned ${validateErrors.length} errors, but parse returned ${parseErrors.length} errors`
  );

  // Check that validate returns correct format
  for (const error of validateErrors) {
    assert('type' in error, `Error should have 'type' field: ${JSON.stringify(error)}`);
    assert('message' in error, `Error should have 'message' field: ${JSON.stringify(error)}`);
    assert('severity' in error, `Error should have 'severity' field: ${JSON.stringify(error)}`);
    // Check optional fields exist (can be null)
    assert(
      'file' in error || error.file === null,
      `Error should have 'file' field: ${JSON.stringify(error)}`
    );
    assert(
      'line' in error || error.line === null,
      `Error should have 'line' field: ${JSON.stringify(error)}`
    );
    assert(
      'objectId' in error || error.objectId === null,
      `Error should have 'objectId' field: ${JSON.stringify(error)}`
    );
    assert(
      'fieldName' in error || error.fieldName === null,
      `Error should have 'fieldName' field: ${JSON.stringify(error)}`
    );
    assert(
      'reference' in error || error.reference === null,
      `Error should have 'reference' field: ${JSON.stringify(error)}`
    );
    assert(
      'candidates' in error || error.candidates === null,
      `Error should have 'candidates' field: ${JSON.stringify(error)}`
    );
  }

  // Check exit code: 0 if no errors, 1 if errors
  const expectedExitCode = validateErrors.length === 0 ? 0 : 1;
  assert(
    validateResult.status === expectedExitCode,
    `validate should exit with code ${expectedExitCode}, but exited with ${validateResult.status}`
  );
});

// Test 9: spaced __Workspace kind must be detected by the CLI
// `[[id: __Workspace]]` (space after colon) is valid QMD and must be found by
// `workspace parse` (exit 0), same as the unspaced form.
test('CLI workspace parse detects spaced __Workspace kind', () => {
  const ws = mkdtempSync(resolve(tmpdir(), 'qmdc-spaced-ws-'));
  try {
    writeFileSync(
      resolve(ws, 'readme.qmd.md'),
      '# Spaced Project [[spaced_proj: __Workspace]]\n\n' +
        '- description: workspace with a space after the colon\n\n' +
        '## Thing [[thing]]\n\n' +
        '- value: 1\n'
    );

    const result = runCLI(['workspace', 'parse', ws]);
    assert(
      result.status === 0,
      `spaced __Workspace should be detected, got exit ${result.status}: ${result.stderr}`
    );
    const output = JSON.parse(result.stdout);
    assert(
      output.workspace === 'spaced_proj',
      `expected workspace 'spaced_proj', got ${JSON.stringify(output.workspace)}`
    );
  } finally {
    rmSync(ws, { recursive: true, force: true });
  }
});

console.log('\nAll CLI tests completed!');
report.finish();
