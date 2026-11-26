# QMD-96: Result

## Touch screens show the Mermaid toolbar above the diagram [[qmd96_result: Result]]

On devices without hover the Mermaid toolbar is fully visible, sits in its own row above the
diagram, and has 44x44 px buttons. Mouse devices are unchanged.

- feature: [[#qmd96_touch_toolbar]]
- completed: 2026-11-26
- files_changed: [qmdc-mkdocs/qmdc_mkdocs/templates/css/qmdc-mermaid.css, qmdc-mkdocs/tests/UI_TEST_PLAN.md]
- tests_changed: [qmdc-mkdocs/tests/test_mermaid.py]

### Verification [[qmd96_result_verification: text]]

- about: [[#qmd96_result]]

- The issue's reproduction, built with `qmdc-mkdocs build` and measured in Chromium (Playwright)
  at 390x844 with touch emulation: before, opacity `0.35`, button 26x26, toolbar overlapping the
  diagram; after, opacity `1`, button 44x44, toolbar above the diagram with no overlap. A
  1280x900 mouse profile measures the same before and after (opacity `0.35` floating, 26x26).
- A phone screenshot after the fix shows the toolbar in its own row above the nodes.
- `TestTouchToolbar` (3 tests) fails on the old stylesheet and passes on the new one.
- `make mkdocs-test`, `make test-fast`, `make validate-docs` and `make md-lint` exit 0.
- Code review: self-review only.
