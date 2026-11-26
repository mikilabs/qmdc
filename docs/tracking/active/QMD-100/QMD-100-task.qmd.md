# QMD-100: A long props-card value widens the page on phones

## An unbroken value sets the width of the props card [[qmd100_props_overflow: Bug]]

GitHub [#35](https://github.com/mikilabs/qmdc/issues/35). On a narrow viewport, an unbroken
property value such as a URL sets the width of the properties card's value track. The value runs
out of the card and the whole page scrolls sideways.

- status: done_review
- priority: medium
- category: mkdocs
- findings: [[#qmd100_finding_cause]]
- result: [[#qmd100_result]]

### Goals [[goals: [Goal]]]

#### A1: The overflow is measured and pinned [[qmd100_goal_a1]]

In Chromium at 390x844 the issue's page is wider than the viewport; stylesheet tests for the
wrapping rules fail on the current CSS.

- group: A_tests
- done: true

#### B1: Values wrap inside the card [[qmd100_goal_b1]]

The value track can shrink below its content and an unbroken value wraps, so the page keeps the
viewport width. Desktop keeps the two-column card.

- group: B_fix
- done: true
