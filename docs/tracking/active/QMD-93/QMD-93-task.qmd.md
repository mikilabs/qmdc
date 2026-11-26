# QMD-93: hidden_kinds drops the backlink instead of crediting the page

## A hidden object's incoming edge disappears from "Linked from" [[qmd93_hidden_kinds_backlink: Bug]]

GitHub [#42](https://github.com/mikilabs/qmdc/issues/42). With `hidden_kinds: [Score]`, a Product
page holding a `Score` sub-object that references Quality still shows "Links to: Quality", but the
Quality page shows no "Linked from" entry. The same edge is hidden on one side only.

- status: done_review
- priority: medium
- category: mkdocs
- findings: [[#qmd93_finding_cause]], [[#qmd93_finding_fix]]
- result: [[#qmd93_result]]

### Goals [[goals: [Goal]]]

#### A1: A test reproduces the missing backlink [[qmd93_goal_a1]]

A test built from the issue's workspace fails on the current code: Quality gets no "Linked from"
entry with `hidden_kinds: [Score]`.

- group: A_tests
- done: true

#### B1: The containing page is credited [[qmd93_goal_b1]]

An incoming edge from a hidden-kind object is listed as the page that holds it, once per page and
edge type. Without `hidden_kinds` and on the "Links to" side nothing changes.

- group: B_fix
- done: true
