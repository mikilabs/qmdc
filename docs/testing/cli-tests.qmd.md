# CLI Tests [[suite_cli_tests: TestSuite]]

- location: qmdc-*/tests/
- format: Hardcoded test data in test code
- test_count: 22
- implementations: [test_cli.py, test-cli.ts, cli_unit.rs]
- is_data_driven: false
- tests: [[#parsers]]

## Description [[description: text]]

CLI command verification via subprocess — testing the command-line interface.

These tests are NOT data-driven — test data is hardcoded in the test code. This is legacy. Do not create new CLI tests this way. For new CLI tests, use a data-driven approach.

Problems with the current approach:

- Cannot reuse test data across parsers
- Hard to add new tests
- Code duplication

Use CLI tests when:

- Verifying CLI options (--input, --output, --format, --no-comments, --no-pretty)
- Testing stdin/stdout
- Checking exit codes
- Testing command integration

Do not use for QMDC parsing (use parser microtests) or workspace functionality (use workspace tests).

## What Is Tested

1. **parse with stdin** — reading from stdin, output to stdout, correct JSON
2. **parse with file** — reading from file, `-i` / `--input` option
3. **parse with file output** — `-o` / `--output` option, file creation
4. **parse --no-comments** — removing `__comments` from output
5. **parse --no-pretty** — compact JSON (no formatting)
6. **rebuild** — `rebuild` command, JSON → QMD.md conversion
7. **Multiple microtests** — parsing multiple files, CLI stability

Test counts: 7 (Python), 7 (TypeScript), 8 (Rust).

## Migration to Data-Driven

To migrate CLI tests to a data-driven approach:

1. Create a `tasks/CLI-TESTS/` directory
2. For each test create:
   - `NNN-name/command.txt` — command to run
   - `NNN-name/input.txt` — stdin (if needed)
   - `NNN-name/expected.json` — expected output
   - `NNN-name/expected_exit_code.txt` — expected exit code
3. Update test code to read from files
