# QMD-98: Heading markers leak into search titles and anchors

## `[[id: Kind]]` text ends up in heading slugs and search titles [[qmd98_heading_marker_leak: Bug]]

GitHub [#38](https://github.com/mikilabs/qmdc/issues/38). Heading markers were turned into
`<span>`s inside the heading, and MkDocs builds the heading slug and the search entry from the
heading's full text, span text included. Search results read `Question [[question: text]]text`,
and every permalink looked like `#question-question-texttext`.

- status: done_review
- priority: medium
- category: mkdocs
- findings: [[#qmd98_finding_cause]], [[#qmd98_finding_fix]]
- result: [[#qmd98_result]]

### Goals [[goals: [Goal]]]

#### A1: Tests reproduce the polluted heading text [[qmd98_goal_a1]]

Tests that run the transformed headings through Python-Markdown's `toc` fail on the current
code: the names carry the marker and kind text, and the ids are slugs of it.

- group: A_tests
- done: true

#### B1: Headings carry the QMDC id and clean text [[qmd98_goal_b1]]

The heading itself has the object id, and its text (anchor slug, "On this page", search title)
is the heading text alone. The kind badge still shows. Content-generator target headings are
still hidden, and hint icons still land in their heading.

- group: B_fix
- done: true
