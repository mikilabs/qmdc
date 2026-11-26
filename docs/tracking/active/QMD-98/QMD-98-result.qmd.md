# QMD-98: Result

## Headings carry the QMDC id and clean text [[qmd98_result: Result]]

Object and field headings get the QMDC id as their own `id`, and their text holds no marker or kind
name, so permalinks, "On this page" and search titles are clean. The kind badge is drawn by CSS.

- feature: [[#qmd98_heading_marker_leak]]
- completed: 2026-11-26
- files_changed: [qmdc-mkdocs/qmdc_mkdocs/syntax.py, qmdc-mkdocs/qmdc_mkdocs/converter.py, qmdc-mkdocs/qmdc_mkdocs/plugin.py, qmdc-mkdocs/qmdc_mkdocs/templates/css/qmdc-extra.css, qmdc-mkdocs/tests/UI_TEST_PLAN.md]
- tests_changed: [qmdc-mkdocs/tests/test_syntax.py]

### Verification [[qmd98_result_verification: text]]

- about: [[#qmd98_result]]

- Five `TestHeadingTextStaysClean` tests and `TestHintIconFindsHeading` fail on the old code and
  pass after the fix; the content-generator guard test passes before and after. Existing
  `transform_definitions` tests were updated to the new markup.
- The issue's reproduction with `qmdc-mkdocs init` and `build`: search titles `Demo`, `Question`,
  `Part`; heading ids `demo`, `question`, `demo.part`. In Chromium the headings read "Demo",
  "Question", "Part" with the badges Page, text and Section drawn beside them, no page errors.
- The repository's own `docs/` site, built with `main` and with the branch: no warnings in either;
  search titles containing `[[` drop from 572 to 14 of 845 entries. The 14 left come from headings
  inside `markdown example` blocks, which are not parsed objects.
- `make mkdocs-test`, `make test-fast`, `make validate-docs` and `make md-lint` exit 0.
- Code review: self-review only.
