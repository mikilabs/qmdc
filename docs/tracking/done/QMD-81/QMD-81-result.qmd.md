# QMD-81: Result

## parse exits 1 when the result holds a __ParsingError [[qmd81_result: Result]]

All three CLIs count `__ParsingError` objects after writing the JSON in full and exit 1 with
`error: document has N parsing error(s)` on stderr; a clean document still exits 0. In
`--format minimal` Python and TypeScript drop system `__kind`, so they count on a standard re-parse
there; Rust keeps `__kind` on errors in every format and counts its own output. That minimal-format
divergence is pre-existing and left as is.

Fixture `040-parse-exit-1-on-parsing-error` asserts exit 1, the full JSON and the stderr reason; it
exited 0 on the 2.0.0 binaries of all three (rs from PyPI 2.0.0, py and ts with the change stashed).
`docs/parsers/commands.qmd.md` now states the exit code. `tests/parser/verify_roundtrip.py`, the only
in-repo caller of bare `parse`, accepts exit 1. SOP review `reviews/10-cr-gh6-7-10-11.md`: APPROVE.

- feature: [[#qmd81_parse_exit]]
- completed: 2026-10-03
- files_changed: [qmdc-rs/src/main.rs, qmdc-py/qmdc/cli.py, qmdc-ts/src/cli.ts, docs/parsers/commands.qmd.md, tests/parser/verify_roundtrip.py, CHANGELOG.md]
- tests_changed: [040-parse-exit-1-on-parsing-error]
