# QMD-97: Properties card shows the quotes of a quoted value

## The props card renders source quoting the parser drops [[qmd97_props_card_quotes: Bug]]

GitHub [#37](https://github.com/mikilabs/qmdc/issues/37). A value written as `"2026-09-08"`, quoted
to keep it a string, is read by the parser as the string `2026-09-08`, but the properties card on
the built page shows `"2026-09-08"` with the quotes.

- status: done_review
- priority: low
- category: mkdocs
- findings: [[#qmd97_finding_cause]]
- result: [[#qmd97_result]]

### Goals [[goals: [Goal]]]

#### A1: A test reproduces the visible quotes [[qmd97_goal_a1]]

A converter test with the issue's page fails on the current code: the card value is
`"2026-09-08"`.

- group: A_tests
- done: true

#### B1: The card shows the parsed value [[qmd97_goal_b1]]

A value wholly wrapped in one matching pair of quotes is shown without them, the same rule as the
parser. Partly quoted values and values with reference links are shown as before.

- group: B_fix
- done: true
