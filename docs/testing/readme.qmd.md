# Testing [[testing:__Namespace]]

- suites: [[#suite_parser_microtests]], [[#suite_workspace_tests]], [[#suite_sql_tests]], [[#suite_sql_rewrite_tests]], [[#suite_lsp_microtests]], [[#suite_lsp_sql_tests]], [[#suite_cli_tests]]

QMDC testing follows a data-driven approach where test data is stored separately from test code.

## Testing Philosophy

Data-driven tests are the standard:

- Test data lives in the repo-root `tests/` tree (organized by purpose)
- Test runners in each language (Python, TypeScript, Rust) read these files
- The same test data is reused across all three parsers
- Adding new tests means creating data files — no code changes needed

Non-data-driven tests are legacy. Do not create new ones.

## Where to Create Tests

All data-driven tests must be created under the repo-root `tests/` tree, organized by purpose. Test runners automatically scan these directories. Do not create tests elsewhere — they will not run automatically.

Scanned paths:

- `tests/parser/` — parser microtests (incl. code-fence cases)
- `tests/workspace/` — workspace tests + SQL workspace tests
- `tests/lsp/microtests/` — LSP microtests
- `tests/lsp/sql/` — LSP SQL integration tests
- `tests/sql/rewrite/` — SQL rewrite tests
- `tests/sql/multi-workspace-isolation/` — SQL-rewrite test database
- `tests/mcp/` — MCP envelope tests
- `tests/cli/` — CLI conformance tests

## Running Tests

```bash
# All tests (format + lint + tests) — sequential
make test

# Parallel (~2x faster, ~22s instead of ~52s)
make test-fast

# Per-language
make py-test
make ts-test
make rs-test

# Extension E2E tests (Playwright)
make ext-test
```

`make test-fast` runs all 5 targets (`py-test`, `ts-test`, `rs-test`, `validate-docs`, `validate-compare`) in parallel via `make -j`. The dependency graph ensures format → lint → build → test execute in the correct order within each language.

If `gmake` (GNU Make 4+, `brew install make`) is installed, output is automatically grouped per target via `--output-sync=target`. The system Make on macOS (3.81) does not support this, so parallel output will interleave.

## Statistics

| TestSuite | Data-driven | Python | TypeScript | Rust | Tests |
|-----------|-------------|--------|------------|------|-------|
| Parser Microtests | ✅ | ✅ | ✅ | ✅ | 181 |
| LSP Microtests | ✅ | ❌ | ❌ | ✅ | 100 |
| LSP SQL Integration | ✅ | ❌ | ❌ | ✅ | 3 |
| Workspace Tests | ✅ | ✅ | ✅ | ✅ | 12 |
| SQL Tests | ✅ | ✅ | ✅ | ✅ | 41 |
| SQL Rewrite Tests | ✅ | ❌ | ❌ | ✅ | 25 |
| CLI Tests | ✅ | ✅ | ✅ | ✅ | 10 |
