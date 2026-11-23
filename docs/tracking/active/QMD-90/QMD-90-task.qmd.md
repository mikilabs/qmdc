# QMD-90: CRLF line endings change edge types

## A bullet ref in a multi-line field gets the field name with CRLF [[qmd90_crlf_edge_type: Bug]]

GitHub [#23](https://github.com/mikilabs/qmdc/issues/23). With CRLF line endings, a
`- key: [[#ref]]` line followed by a blank line and more text inside a multi-line text field gets
the edge type of the field (`overview`) instead of the bullet key (`about`). With LF the same file
gives `about`. On Windows git's `core.autocrlf=true` checks `.qmd.md` files out with CRLF, so a
workspace that is right on Linux and macOS resolves different edges there.

Expected: the same objects, field values and edges whatever the line endings.

- status: done_review
- priority: high
- category: parser
- findings: [[#qmd90_finding_cause]], [[#qmd90_finding_fix]], [[#qmd90_finding_tests]]
- result: [[#qmd90_result]]

### Goals [[goals: [Goal]]]

#### A1: Failing CRLF fixture [[qmd90_goal_a1]]

A workspace fixture stored with CRLF bytes checks the edge type and the field value in all three
parsers, and a Python test checks `parse()` on an in-memory CRLF string. Both fail before the fix.

- group: A_tests
- done: true

#### B1: Parsers read CRLF and CR as LF [[qmd90_goal_b1]]

`parse()` in Rust, TypeScript and Python normalises `\r\n` and a lone `\r` to `\n` first.

- group: B_fix
- done: true

#### B2: The spec states the rule [[qmd90_goal_b2]]

`docs/format/readme.qmd.md` says line endings don't change the result.

- group: B_fix
- done: true
