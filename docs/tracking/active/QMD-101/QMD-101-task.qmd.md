# QMD-101: Readable default scale for wide Mermaid diagrams

## A wide Mermaid diagram opens shrunk to an unreadable scale [[qmd101_mermaid_fit_floor: Bug]]

GitHub [#34](https://github.com/mikilabs/qmdc/issues/34). The Mermaid viewport fits every diagram
to the column width on load with no lower bound. The issue's 8-node `flowchart LR` opens at 28% on
a 1400 px desktop and at 15% on a 390 px phone, so its labels cannot be read. The README says
diagrams render at "natural (readable) size".

- status: done_review
- priority: medium
- category: mkdocs
- findings: [[#qmd101_finding_cause]], [[#qmd101_finding_fix]]
- result: [[#qmd101_result]]

### Goals [[goals: [Goal]]]

#### A1: The default scale is measured and pinned by tests [[qmd101_goal_a1]]

In Chromium on the issue's reproduction the default view measures 28% (1400x1000) and 15%
(390x844); core tests for a floor on the default fit fail on the current script.

- group: A_tests
- done: true

#### B1: The default view never shrinks below a readable floor [[qmd101_goal_b1]]

On load and on resize a wide diagram fits the column but not below 75%, and the rest scrolls
horizontally inside the viewport. The fit-to-width button and the `0` key still fit the whole
diagram. The VS Code preview gets the same core, and the README describes the behaviour.

- group: B_fix
- done: true
