# QMD-102: Result

## "On this page" lists every heading, nested [[qmd102_result: Result]]

The graph sidebar's "On this page" lists every rendered heading below the page title, in order
and nested, with anchors that exist on the page, on desktop and in the phone drawer.

- feature: [[#qmd102_page_toc]]
- completed: 2026-11-26
- files_changed: [qmdc-mkdocs/qmdc_mkdocs/templates/partials/toc.html, qmdc-mkdocs/qmdc_mkdocs/templates/css/qmdc-extra.css, qmdc-mkdocs/qmdc_mkdocs/graph.py, qmdc-mkdocs/qmdc_mkdocs/converter.py, qmdc-mkdocs/tests/UI_TEST_PLAN.md]
- tests_changed: [qmdc-mkdocs/tests/test_graph.py, qmdc-mkdocs/tests/test_converter.py]

### Verification [[qmd102_result_verification: text]]

- about: [[#qmd102_result]]

- The issue's reproduction, built with `qmdc-mkdocs build` and checked in Chromium (Playwright).
  At 1400 px: before, the TOC lists `Part one` only; after, `Part one`, `Summary`,
  `Plain subsection` (indented 14 px) and `Plain heading`. Clicking `Plain subsection` scrolls to
  it. In the phone drawer's TOC panel at 390x844: before `Part one` only, after the same four
  entries, nested. No console errors.
- The docs site built from `main` and from the branch: pages with a TOC go from 37 to 66 of 70,
  entries from 145 to 695, and links to a missing anchor from 13 to 0. Every entry dropped from
  `main` is one of those 13 (ContentGenerator headings with no anchor on the page).
- `TestPageToc` (3 tests) fails on the old template and passes on the new one.
- `make mkdocs-test`, `make test-fast`, `make validate-docs` and `make md-lint` exit 0.
- Code review: self-review only.
