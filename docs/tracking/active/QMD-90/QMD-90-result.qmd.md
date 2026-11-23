# QMD-90: Result

## Parsers read CRLF and CR as LF [[qmd90_result: Result]]

`parse()` in all three parsers first replaces `\r\n` and a lone `\r` with `\n`. A file now gives the
same objects, field values and edges whatever line endings it was saved with, so the preamble
`- about: [[#api]]` gets edge type `about` with CRLF too, and multi-line field values no longer
keep a `\r`. The spec states the rule in `docs/format/readme.qmd.md`.

- feature: [[#qmd90_crlf_edge_type]]
- completed: 2026-11-23
- files_changed: [qmdc-rs/src/parser.rs, qmdc-ts/src/parser.ts, qmdc-py/qmdc/parser.py, docs/format/readme.qmd.md, .gitattributes]
- tests_added: [tests/workspace/crlf-line-endings, qmdc-py/tests/test_workspace.py::test_qmd90_parse_reads_crlf_as_lf]

### Verification [[qmd90_result_verification: text]]

- about: [[#qmd90_result]]

- The issue's repro, LF and CRLF, through each CLI: `about` in all six runs (before: `overview`
  for CRLF in Rust and TypeScript).
- `crlf-line-endings` SQL tests: both fail before the fix in Rust and TypeScript (edge type
  `overview`, value with `\r\n`) and pass after it in all three parsers.
- `test_qmd90_parse_reads_crlf_as_lf` fails with the Python normalisation disabled and passes
  with it.
- `make test-fast` exits 0: 4,639 cases, 0 failures, parser case counts equal across languages,
  parse parity exact across 160 documents.
- `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` exit 0; `make validate-docs`
  passes.
- Code review: self-review only.
