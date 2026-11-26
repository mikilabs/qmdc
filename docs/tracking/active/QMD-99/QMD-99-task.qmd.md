# QMD-99: Sidebar edge lists sort numbered labels as text

## "Links to" / "Linked from" put 10 before 2 [[qmd99_edge_order: Bug]]

GitHub [#44](https://github.com/mikilabs/qmdc/issues/44). Edge lists in the graph sidebar are
sorted by label as plain strings, so labels `1.`, `2.`, `3.`, `10.`, `11.` come out as 1, 10,
11, 2, 3.

- status: done_review
- priority: low
- category: mkdocs
- findings: [[#qmd99_finding_cause]]
- result: [[#qmd99_result]]

### Goals [[goals: [Goal]]]

#### A1: Tests reproduce the text order [[qmd99_goal_a1]]

Tests with the issue's five axes fail on the current code for both "Links to" and
"Linked from".

- group: A_tests
- done: true

#### B1: Edge lists sort numbers as numbers [[qmd99_goal_b1]]

Both lists are ordered by edge type and then naturally by the displayed label.

- group: B_fix
- done: true
