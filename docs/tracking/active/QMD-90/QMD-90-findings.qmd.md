# QMD-90: Findings

## The preamble split never sees a blank line [[qmd90_finding_cause: Finding]]

Reproduced on `d7374cc` with the issue's repro: Rust and TypeScript give `overview` for CRLF,
`about` for LF. Python gives `about` for both, but only because its CLI and workspace scan open
files in text mode, which turns `\r\n` into `\n`; `parse()` on an in-memory CRLF string keeps the
`\r` like the other two.

The edge type of a text field's leading `- key: [[#ref]]` list comes from the preamble extractor
(`extract_preamble_refs` in Rust, `extractPreambleRefs` in TypeScript, `_extract_preamble_refs` in
Python). It cuts the value at the first blank line with `split("\n\n")`. With CRLF the value is
`- about: [[#api]]\r\n\r\nThe API accepts requests.`, which has no `\n\n`, so the prose line lands
in the preamble block, fails the `- ` check, the whole preamble is rejected, and the edge falls
back to the field name. That is why only "bullet, blank line, more text" triggers it.

No fixture caught it: `.gitattributes` pins `*.qmd.md` to `eol=lf` (QMD-65), so every fixture is
LF on every platform.

- category: bug
- related_to: [[#qmd90_crlf_edge_type]]
- affected_files: [qmdc-rs/src/parser.rs, qmdc-ts/src/parser.ts, qmdc-py/qmdc/parser.py]
- affected_functions: [parse, extract_preamble_refs]
- solution: Normalise line endings once at the start of `parse()`.

## Normalise at parse(), not in the extractor [[qmd90_finding_fix: Finding]]

Fixing only the three preamble extractors would correct the edge type but leave a `\r` in every
multi-line field value from Rust and TypeScript, and from Python's `parse()` on a string, so the
same file would still give different values depending on how it was read. Normalising at the top
of `parse()` covers every caller (CLI, workspace scan, LSP buffers, library calls) and matches
what Python's text-mode file read already did. The spec already strips a trailing `\r` from
`__TextBlock` lines; `docs/format/readme.qmd.md` now states the general rule.

Line numbers and columns are unchanged for CRLF: the `\r` sits at the end of a line. A lone `\r`
now ends a line, which is how LSP clients count lines too.

- category: design
- related_to: [[#qmd90_crlf_edge_type]]
- solution: `normalize_line_endings` (Rust, Python) / `normalizeLineEndings` (TypeScript) replaces `\r\n` and then `\r` with `\n`; it returns the input unchanged when there is no `\r`.

## Tests [[qmd90_finding_tests: Finding]]

Existing tests are all LF, so none covers this. A workspace fixture `tests/workspace/crlf-line-endings`
is stored with CRLF bytes and exempted from `eol=lf` in `.gitattributes`. Its two SQL tests pin the
edge type (`about`) and the `overview` value without `\r`; they run in all three parsers' SQL
suites. Python's suite reads the fixture in text mode, so `test_qmd90_parse_reads_crlf_as_lf` in
`qmdc-py/tests/test_workspace.py` passes the raw CRLF bytes, and a lone-CR copy, to `parse()`.
It is a unit test so the parser-suite case counts stay equal across languages.

- category: testing
- related_to: [[#qmd90_crlf_edge_type]]
- solution: One CRLF workspace fixture with two SQL tests, one Python unit test.
