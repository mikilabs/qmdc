# QMD-101: Result

## Wide Mermaid diagrams open at a readable scale [[qmd101_result: Result]]

A wide Mermaid diagram opens fitted to the column but not below 75%, and the part that does not
fit scrolls horizontally. The fit-to-width button still fits the whole diagram. The VS Code
preview shares the change.

- feature: [[#qmd101_mermaid_fit_floor]]
- completed: 2026-11-26
- files_changed: [qmdc-mkdocs/qmdc_mkdocs/templates/js/qmdc-mermaid-core.js, qmdc-vscode/templates/qmdc-mermaid-core.js, qmdc-mkdocs/README.md, qmdc-mkdocs/tests/UI_TEST_PLAN.md]
- tests_changed: [qmdc-mkdocs/tests/test_mermaid.py, qmdc-vscode/e2e/preview-render.spec.ts]

### Verification [[qmd101_result_verification: text]]

- about: [[#qmd101_result]]

- The issue's reproduction, built with `qmdc-mkdocs build` and measured in Chromium (Playwright).
  At 1400x1000: before, the label reads `28%` (SVG 686 of 2446 px); after, `75%` (SVG 1835 px
  in a 686 px viewport that scrolls, page width unchanged at 1400 px). At 390x844 with touch:
  before `15%`, after `75%`, page width unchanged at 390 px. A resize while in the default
  view keeps `75%`. The fit-to-width button and the `0` key give `28%` and `15%` as before.
  A two-node diagram stays at `100%`.
- A desktop screenshot after the fix shows readable node labels.
- `TestDefaultFitFloor` (3 tests) fails on the old core and passes on the new one.
- Two VS Code preview e2e tests pinned the old default (a large diagram fills the column, so
  it is not pannable on load). They now assert the 75% floor on load and the full fit after
  the Fit button; both fail on the old core and pass on the new one.
- `make mkdocs-test`, `make test-fast`, `make validate-docs` and `make md-lint` exit 0.
- Code review: self-review only.
