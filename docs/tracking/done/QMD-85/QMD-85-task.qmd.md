# QMD-85: rebuild writes values a re-parse reads differently

## rebuild does not quote what the reader would reinterpret [[qmd85_rebuild_quoting: Bug]]

GitHub [#15](https://github.com/mikilabs/qmdc/issues/15). `rebuild` writes a string value in a
form a re-parse reads as something else. The issue reports array items with a comma: qmdc-py and
qmdc-ts write `["x, y", z]` as `[x, y, z]` (three items on re-parse), and qmdc-rs doubles the
quotes of an item that already carries them (`""x, y""`, split in two). Measured during triage, the
same defect is wider than commas: a string `"true"`, `"42"`, `"null"` or `"[x]"` comes back as a
boolean, number, null or array, and a string that itself begins with a quote character loses it.
All three parsers READ every one of these shapes identically; only the three writers are wrong, in
different ways.

Found in qmdc-wiki: 653 damaged `calls_unresolved` lines in 224 of 8,400 generated files.

- status: done
- priority: high
- category: parser
- related_task: [[#qmd71]]
- findings: [[#qmd85_finding_reader]], [[#qmd85_finding_writers]], [[#qmd85_finding_rule]], [[#qmd85_finding_list_items]], [[#qmd85_finding_issue_table]], [[#qmd85_finding_no_spelling]], [[#qmd85_finding_tests]], [[#qmd85_finding_questions]]
- result: [[#qmd85_result]]

### Goals [[goals: [Goal]]]

#### A1: Failing regression fixtures for every lost shape [[qmd85_goal_a1]]

Parser microtests whose parse output all three parsers already agree on, and whose
parse -> rebuild -> parse round trip fails today for the documented reason: comma inside an array
item, literal quote characters inside a value, and quoted keywords, numbers and brackets.

- group: A_tests
- done: true

#### B1: One quoting rule in all three writers [[qmd85_goal_b1]]

`rebuild` in qmdc-rs, qmdc-py and qmdc-ts quotes a string exactly when the reader would otherwise
read something else, and picks a quote character the value does not contain. The same rule covers
scalar fields, inline and multiline YAML arrays and table cells; map values stay raw because the
reader keeps them raw. Markdown-list items follow each parser's own reader, which differ (finding
`qmd85_finding_list_items`, left for a separate task).

- group: B_fix
- done: true

#### B2: The three writers produce the same text [[qmd85_goal_b2]]

A shared `tests/cli` rebuild case feeds the issue's JSON (items with commas, with literal quotes,
quoted keywords) to `qmdc rebuild` and pins one expected text for every parser.

- group: B_fix
- done: true

#### C1: The format guide states the rule [[qmd85_goal_c1]]

`docs/format/arrays.qmd.md` says that a quoted item cannot contain its own quote character and uses
the other one.

- group: C_docs
- done: true
