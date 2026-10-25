# QMD-85: Result

## rebuild quotes exactly what the reader would reinterpret [[qmd85_result: Result]]

The three writers now share one quoting rule, derived from the reader that all three parsers already
agree on (see [[#qmd85_finding_rule]]). A string is written bare only when the reader returns it
unchanged; otherwise it is wrapped in `"`, or in `'` when it contains `"`. This covers array items
with commas, spaces, quotes or brackets (or a leading `>` in a multiline array), quoted keywords and
numbers, values that themselves start with a quote, scalar fields, multiline arrays and table cells. The three writers produce the same
text for every case in the new CLI fixture, and every value reads back unchanged in all three
parsers.

Not covered, by decision: an array item holding both quote characters has no inline spelling (none
in 1.9 million measured items, [[#qmd85_finding_no_spelling]]), and Markdown-list items are read
differently by Rust than by Python and TypeScript ([[#qmd85_finding_list_items]]), so each writer
keeps matching its own reader there.

Behaviour change for producers: a value the producer pre-wrapped in quotes now keeps those quotes
after a round trip ([[#qmd85_finding_issue_table]]). qmdc-wiki should stop pre-quoting; rebuild
quotes `true`, `42` or `[x]` itself.

- feature: [[#qmd85_rebuild_quoting]]
- files_changed: [qmdc-rs/src/rebuild.rs, qmdc-rs/src/parser_modules/value_parser.rs, qmdc-rs/src/parser_modules/mod.rs, qmdc-py/qmdc/parser.py, qmdc-ts/src/parser.ts, qmdc-ts/src/parsers/field.ts, docs/format/arrays.qmd.md]
- tests_added: [tests/parser/295-yaml-array-item-with-comma, tests/parser/296-value-with-literal-quotes, tests/parser/297-quoted-value-keeps-string-type, tests/cli/041-rebuild-quotes-values]

### Verification [[qmd85_result_verification: text]]

- about: [[#qmd85_result]]

- `make test-fast`: 4,559 cases, 0 failures; parse parity exact over 148 documents; validation
  errors identical across the three parsers. Each new case was found by name in every parser's
  report.
- Every new fixture, and the CLI case, fails on the 2.0.1 code for its documented reason and passes
  after the fix.
- A cross-parser probe (nine shapes, each parsed, rebuilt by rs, py and ts, re-parsed) showed every
  value lost before and kept after, with byte-identical rebuild text from the three writers.
- Independent cold review (separate agent, no access to this reasoning): verdict PASS, no blocking
  items. Its probe of 63 strings in scalar, inline-array and multiline-array position found the
  three writers byte-identical and one gap, a multiline-array item starting with `>` (a blockquote
  on its own line), which was pre-existing. Fixed in all three writers and pinned in fixture 295;
  re-running the reviewer's probe afterwards leaves only the documented both-quotes case. It also
  noted that map values diverge between writers (Rust raw, Python and TypeScript YAML-quoted); that
  code is untouched by this task and the divergence predates it.
- Second cold review on the final diff (after the `>` fix): verdict PASS, no blocking items. Its
  probes (63 + 42 + 30 strings, including table cells) found the writers byte-identical in every
  position; the only non-round-trip is the documented both-quotes case. Pre-existing gaps it noted,
  outside this task's hunks: Rust writes one extra blank line before a table's `### Rows`
  sub-heading, and a lone `|` in a table cell cannot round-trip in any parser.
