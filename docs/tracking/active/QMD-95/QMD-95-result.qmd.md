# QMD-95: Result

## Index pages are named by their own heading, and ancestors are links [[qmd95_result: Result]]

A namespace readme reads `<workspace> › News` in the breadcrumb and `News` in Siblings; the
workspace readme shows the workspace title alone. On other pages the workspace and namespace items
link to their index pages.

- feature: [[#qmd95_index_breadcrumb]]
- completed: 2026-11-26
- files_changed: [qmdc-mkdocs/qmdc_mkdocs/graph.py, qmdc-mkdocs/qmdc_mkdocs/converter.py, qmdc-mkdocs/qmdc_mkdocs/templates/partials/toc.html, qmdc-mkdocs/tests/UI_TEST_PLAN.md]
- tests_changed: [qmdc-mkdocs/tests/test_graph.py]

### Verification [[qmd95_result_verification: text]]

- about: [[#qmd95_result]]

- `TestIndexPageBreadcrumb` (7 tests, the issue's workspace) fails on the old code (`First update`,
  `Readme`, no links) and passes after the fix. It covers the namespace readme, a page in the
  namespace, the workspace readme, a single-namespace build, an ignored index page, and the
  rendered template.
- The issue's reproduction with `qmdc-mkdocs init` and `build`: `news/index.html` shows a linked
  workspace item and `News` as the current item, Siblings lists `Archive` and `News`;
  `news/archive/index.html` links both ancestors (`../../` and `../`).
- `make mkdocs-test`, `make test-fast`, `make validate-docs` and `make md-lint` exit 0.
- Code review: self-review only.
