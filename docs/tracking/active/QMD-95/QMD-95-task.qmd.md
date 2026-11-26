# QMD-95: Breadcrumb on a namespace readme names its first child

## Index pages are labelled by their first child, and ancestors are not links [[qmd95_index_breadcrumb: Bug]]

GitHub [#41](https://github.com/mikilabs/qmdc/issues/41). On a namespace's `readme.qmd.md`, the
breadcrumb's last item and the page's Siblings entry use the first non-system object on the page.
For a news index whose first child is `### First update [[u1: Update]]`, the breadcrumb reads
`… › News › First update`. The breadcrumb items are plain `<span>`s, not links.

- status: done_review
- priority: medium
- category: mkdocs
- findings: [[#qmd95_finding_cause]], [[#qmd95_finding_fix]]
- result: [[#qmd95_result]]

### Goals [[goals: [Goal]]]

#### A1: Tests reproduce the wrong label and the missing links [[qmd95_goal_a1]]

Tests built from the issue's workspace fail on the current code: the namespace readme is labelled
`First update`, the workspace readme `Readme`, and the rendered breadcrumb has no links.

- group: A_tests
- done: true

#### B1: Index pages use their own heading, ancestors link to their index [[qmd95_goal_b1]]

A namespace or workspace readme is named by its `__Namespace` or `__Workspace` object in the
breadcrumb and Siblings, and the breadcrumb ends at that item. The workspace and namespace items
link to their index pages when those pages are in the built site and are not the current page.

- group: B_fix
- done: true
