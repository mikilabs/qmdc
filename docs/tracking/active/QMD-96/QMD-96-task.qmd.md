# QMD-96: Mermaid toolbar on touch screens

## The Mermaid toolbar is faint, small and covers the diagram on phones [[qmd96_touch_toolbar: Bug]]

GitHub [#45](https://github.com/mikilabs/qmdc/issues/45). The Mermaid toolbar sits at
`opacity: 0.35` until `:hover` or `:focus-within`, which a touch screen never gives before the
first tap. On a 390 px phone it is drawn over the diagram and covers node labels, and its buttons
are 26x26 px.

- status: done_review
- priority: medium
- category: mkdocs
- findings: [[#qmd96_finding_cause]], [[#qmd96_finding_fix]]
- result: [[#qmd96_result]]

### Goals [[goals: [Goal]]]

#### A1: The phone state is measured and pinned by tests [[qmd96_goal_a1]]

In Chromium at 390x844 with touch emulation the toolbar measures opacity 0.35, 26x26 buttons and
overlaps the diagram; stylesheet tests for the touch rules fail on the current CSS.

- group: A_tests
- done: true

#### B1: Touch screens get a visible, separate, tappable toolbar [[qmd96_goal_b1]]

Under `@media (hover: none)` the toolbar is fully opaque, in its own row above the diagram, with
44x44 px buttons. Mouse devices keep the current floating toolbar.

- group: B_fix
- done: true
