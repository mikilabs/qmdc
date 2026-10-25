# QMD-85: Findings

## The reader is the same in all three parsers [[qmd85_finding_reader: Finding]]

Every shape in this task parses identically in qmdc-rs, qmdc-py and qmdc-ts, so the reader is the
specification the writers must meet. Parse parity over the `docs` corpus is exact before and after
the change.

- category: parser
- related_to: [[#qmd85_rebuild_quoting]]
- affected_files: [qmdc-rs/src/parser_modules/value_parser.rs, qmdc-py/qmdc/parsers/field.py, qmdc-ts/src/parsers/field.ts]
- affected_functions: [parse_field_value, parse_yaml_array_items, parse_array_item, _split_yaml_array, splitYamlArray]
- solution: No reader change. The writers are fixed to produce text this reader returns unchanged.

### What the reader does with a value [[qmd85_finding_reader_rules: text]]

- about: [[#qmd85_finding_reader]]

A scalar field value, a Markdown-list item (in Python and TypeScript; see
[[#qmd85_finding_list_items]]) and a table cell all go through the same value reader:

1. `true`, `false` and `null` become a boolean or null.
2. Text matching the number grammar (an optional minus, digits, optionally a dot and digits) becomes
   a number.
3. Text that starts with `[` and ends with `]` becomes an array, unless it is a single reference
   (starts with two brackets, not three) or several references separated by commas.
4. Text that starts and ends with the same quote character loses exactly those two characters.
   Nothing inside is unescaped: a backslash is an ordinary character.
5. Anything else is the string as written.

Inside an inline array the reader first splits on commas. A quote character opens a quoted run
wherever it appears, not only at the start of an item, and the run ends at the next occurrence of the
same character. Each part then goes through the value reader above. There is no escape character.
Rust additionally keeps commas inside nested brackets together; Python and TypeScript do not, so a
writer that never leaves a bracket unquoted inside an item is safe for all three.

Map values are different: the reader keeps them raw, so the map writer must not quote them.

## Three writers, three different gaps [[qmd85_finding_writers: Finding]]

- category: parser
- related_to: [[#qmd85_rebuild_quoting]]
- affected_files: [qmdc-rs/src/rebuild.rs, qmdc-py/qmdc/parser.py, qmdc-ts/src/parser.ts]
- affected_functions: [format_value_inner, format_value, _format_value, _format_primitive_field, formatValue, formatPrimitiveField]
- solution: Replace the per-writer heuristics with the single rule in the next finding.

### What each writer does today [[qmd85_finding_writers_detail: text]]

- about: [[#qmd85_finding_writers]]

qmdc-py and qmdc-ts quote a string only when it has leading or trailing whitespace, and escape an
inner `"` as a backslash and a quote. The reader has no escapes, so even that case reads back with
an extra backslash. Commas, quotes, keywords, numbers and brackets are written bare.

qmdc-rs quotes an array item that contains a space, a comma or a leading single bracket, by wrapping
it in `"` without looking inside. An item that already contains `"` therefore splits. Scalars,
keywords and numbers are written bare, and Markdown-list items are written as the raw string.

All three format the items of a multiline array with the scalar formatter, so an item with a comma
there is split even in Rust.

### Measured round trip before the fix [[qmd85_finding_writers_matrix: text]]

- about: [[#qmd85_finding_writers]]

Authored value parsed by all three, rebuilt by each, re-parsed:

| authored | rs rebuild | py and ts rebuild |
| --- | --- | --- |
| `["x, y", z]` | kept | three items |
| `['"x, y"', z]` | quotes doubled, split | quotes lost |
| `['type("P", (), {"a": 1})', z]` | kept | split into three |
| `["true", "42", "null"]` | boolean, number, null | boolean, number, null |
| scalar `"true"`, `"42"`, `"[x]"` | boolean, number, array | boolean, number, array |
| scalar `'"q"'` | quotes lost | quotes lost |
| scalar `" a\"b "` | spaces lost | backslash added |

## The rule [[qmd85_finding_rule: Finding]]

One rule, mirrored in the three writers, derived from the reader above.

- category: parser
- related_to: [[#qmd85_rebuild_quoting]]
- solution: A string is written bare only when the reader returns it unchanged; otherwise it is wrapped in a quote character it does not contain.

### When a string is quoted [[qmd85_finding_rule_detail: text]]

- about: [[#qmd85_finding_rule]]

A string value is quoted when any of these holds:

- it is empty, or has leading or trailing whitespace;
- it is `true`, `false` or `null`, or matches the number grammar;
- it starts with `"` or `'`;
- it starts with `[` and is not a single reference;
- it starts with a dash and a space, or is a lone pipe (the multiline marker).

Inside an inline or multiline array, it is also quoted when it contains a space, a comma, a quote
character or a bracket, except a single reference with none of those inside, or when it starts with
`>`: a multiline array puts each item at the start of its own line, where `>` opens a blockquote.
The other Markdown block openers need a space after them, so the space rule already covers them.
Spaces are included because the format guide requires quotes for them and Rust already wrote them
so.

The quote character is `"`, or `'` when the value contains `"` but not `'`. In a scalar the reader
removes only the first and last character, so either works for any content; in an array the quoted
run must not contain its own quote character. Numbers, booleans and null are written bare as
before; map values stay raw. Markdown-list items follow the rule in Python and TypeScript only; see
[[#qmd85_finding_list_items]].

## Rust reads a Markdown-list item as raw text; Python and TypeScript type it [[qmd85_finding_list_items: Finding]]

Found while writing the shared CLI case. This is a READER divergence that predates the task, so no
writer can produce one text that all three read back the same way.

- category: parser
- related_to: [[#qmd85_rebuild_quoting]]
- affected_files: [qmdc-rs/src/rebuild.rs]
- solution: Out of scope, needs its own task and a decision on which reading is correct (the format guide does not say). Each writer keeps matching its own reader: Rust writes list items raw as before, Python and TypeScript quote them by the rule. The shared CLI case therefore has no Markdown-list field.

### Measured [[qmd85_finding_list_items_measure: text]]

- about: [[#qmd85_finding_list_items]]

One `[[items: array]]` subheading with the items `"true"`, `true`, `42`, `"x, y"`, `[x]` and `null`:

| parser | items read |
| --- | --- |
| rs | all six as the raw strings, quotes included |
| py, ts | `true` (string), true, 42, `x, y`, an array of one, null |

Table cells, which go through the same value reader as scalar fields, read identically in all three
and follow the rule in all three writers.

## The issue's own table marks a lossy case as correct [[qmd85_finding_issue_table: Finding]]

The issue's summary table marks Python's output for repro 2 as correct: the JSON item `"x, y"`, with
the quote characters as part of the value, is written as `"x, y"` and reads back as `x, y`. The
quotes are lost, which breaks the issue's own first expectation, that `parse(rebuild(x)) == x` for
every value. The reader agrees: `['"x, y"', z]` parses to an item that keeps its quotes, in all
three parsers.

- category: parser
- related_to: [[#qmd85_rebuild_quoting]]
- solution: Follow expectation 1. The writer now writes that item as `'"x, y"'`, which reads back with its quotes. A producer that pre-wraps values in quotes to keep `true`, `42` or `[x]` as strings no longer needs to: rebuild quotes those itself. A producer that keeps pre-wrapping will now see the quotes in the parsed value, which is what it wrote.

## A value with both quote characters has no inline-array spelling [[qmd85_finding_no_spelling: Finding]]

An array item that contains both `"` and `'` cannot be quoted without its own quote character
inside, and the reader has no escape. Unless it is the last item, no text reads back as that item.

- category: parser
- related_to: [[#qmd85_rebuild_quoting]]
- solution: Out of scope. The writer wraps it in `"` as Rust did before. Fixing it needs a format decision (an escape in the grammar, or falling back to a Markdown list), and none of the data measured needs it.

### How often it occurs [[qmd85_finding_no_spelling_measure: text]]

- about: [[#qmd85_finding_no_spelling]]

Counted over the inline-array string items of two qmdc-wiki generated knowledge bases, after
removing the producer's own outer quotes: fastapi, 856 files and 62,678 items, none with both quote
characters (35 with `"`); kubernetes, 10,203 files and 1,847,836 items, none with both (182 with
`"`). Every item that contains `"` is covered by the rule above.

## Tests [[qmd85_finding_tests: Finding]]

- category: testing
- related_to: [[#qmd85_rebuild_quoting]]
- solution: Three parser microtests (data-driven, run by all three parsers) and one shared CLI rebuild case.

### Existing coverage [[qmd85_finding_tests_existing: text]]

- about: [[#qmd85_finding_tests]]

`tests/parser/022-yaml-array` and `019-quoted-string` round-trip only values that need no quoting,
and `tests/cli/009-rebuild` has no array item with a comma or a quote. Nothing reproduces the bug.

### New regression tests and their expected failures before the fix [[qmd85_finding_tests_new: text]]

- about: [[#qmd85_finding_tests]]

The parse expectation of each microtest is the output all three parsers already produce, so only
the round trip fails, and it fails in the parsers the issue names:

| fixture | rs | py | ts |
| --- | --- | --- | --- |
| `295-yaml-array-item-with-comma` | multiline item split | inline and multiline items split | inline and multiline items split |
| `296-value-with-literal-quotes` | rebuild and text fail | rebuild and text fail | rebuild and text fail |
| `297-quoted-value-keeps-string-type` | scalars, items and table cells retyped | same | same |

With `make -k test` every suite ran: py 4, rs 3 and ts 4 failing cases, all of them these
fixtures; every other suite, parse parity and validation parity were green. The multiline-array part
of 295 and the table part of 297 were added during implementation and confirmed red against the
2.0.1 code (Python and Rust run from a clean checkout of the tag) before the fix.

The CLI case `tests/cli/041-rebuild-quotes-values` is added with the fix. It feeds JSON straight to
`rebuild`, which is the path the issue's repro 2 and the qmdc-wiki producer use, and pins one
expected text for all three parsers.

## Open questions [[qmd85_finding_questions: Finding]]

Decided while the operator had pre-authorized the whole workflow; listed for review.

- category: parser
- related_to: [[#qmd85_rebuild_quoting]]
- open_questions: [Should pre-quoted producer values keep their quotes? Decided yes - see the issue-table finding., Should inline array items with a space be quoted? Decided yes - the format guide requires it and Rust already did., Both quote characters in one item - out of scope - see the no-spelling finding.]
- solution: Each answer is recorded in the finding it names.
