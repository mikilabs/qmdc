# QMD-102: "On this page" lists every heading

## "On this page" lists only typed objects and renders them flat [[qmd102_page_toc: Bug]]

GitHub [#36](https://github.com/mikilabs/qmdc/issues/36). The graph sidebar replaces Material's
table of contents, and its "On this page" section lists only headings that define a non-system
object. Text-field headings (`[[x: text]]`) and plain Markdown headings are missing, so a page of
prose has no TOC on desktop or in the phone drawer, and every entry is rendered flat.

- status: done_review
- priority: medium
- category: mkdocs
- findings: [[#qmd102_finding_cause]], [[#qmd102_finding_fix]]
- result: [[#qmd102_result]]

### Goals [[goals: [Goal]]]

#### A1: The missing entries are reproduced and pinned by tests [[qmd102_goal_a1]]

On the issue's page the TOC lists only `Part one`; build tests that expect all four headings in
order, with the h3 nested, fail on the current template.

- group: A_tests
- done: true

#### B1: The TOC comes from the page's own headings [[qmd102_goal_b1]]

"On this page" lists every rendered heading below the page title, in order and nested, with
anchors that exist on the page, on desktop and in the phone drawer. The graph sections after it
are unchanged.

- group: B_fix
- done: true
