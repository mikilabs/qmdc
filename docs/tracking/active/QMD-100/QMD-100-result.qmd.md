# QMD-100: Result

## Long props-card values wrap inside the card [[qmd100_result: Result]]

The props card's value track can shrink and unbroken values wrap, so a long URL no longer widens
the card or the page.

- feature: [[#qmd100_props_overflow]]
- completed: 2026-11-26
- files_changed: [qmdc-mkdocs/qmdc_mkdocs/templates/css/qmdc-extra.css]
- tests_changed: [qmdc-mkdocs/tests/test_converter.py]

### Verification [[qmd100_result_verification: text]]

- about: [[#qmd100_result]]

- The issue's reproduction, built with `qmdc-mkdocs build` and measured in Chromium (Playwright):
  at 390x844 with touch emulation, before: page `scrollWidth` 665, value track 610 px, value
  outside the card; after: `scrollWidth` 390, track 300 px, value inside the card. At 1280x900
  the card keeps two columns (`100px 506px`) and the value, which overflowed before, fits.
- A phone screenshot after the fix shows the URL wrapped over three lines inside the card.
- `TestPropsCardWraps` (2 tests) fails on the old stylesheet and passes on the new one.
- `make mkdocs-test`, `make test-fast`, `make validate-docs` and `make md-lint` exit 0.
- Code review: self-review only.
