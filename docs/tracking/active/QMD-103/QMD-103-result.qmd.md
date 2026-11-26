# QMD-103: Result

## Generated nav entries use the page heading [[qmd103_result: Result]]

In the generated nav a namespace or workspace readme is named by its heading, not `Readme`, and
pages in a namespace build are named by their headings, not their file names.

- feature: [[#qmd103_readme_nav_label]]
- completed: 2026-11-26
- files_changed: [qmdc-mkdocs/qmdc_mkdocs/navigation.py, docs/nav.yml, qmdc-mkdocs/tests/UI_TEST_PLAN.md]
- tests_changed: [qmdc-mkdocs/tests/test_navigation.py, qmdc-mkdocs/tests/test_navigation_property.py]

### Verification [[qmd103_result_verification: text]]

- about: [[#qmd103_result]]

- The issue's workspace, built with `qmdc-mkdocs build`: before, `nav.yml` and the rendered left
  nav read `Readme`, `Readme`, `Zebra`, `Middle`, `Alpha`; after, `Repro`, `Products`, `Zebra`,
  `Middle`, `Alpha`. Page order is the same.
- The docs site nav: the 14 `Readme` entries become their namespace and workspace titles, and no
  other line changes. A namespace build of `lsp` goes from `Readme`, `Commands`, `Visual` to
  `LSP Server`, `LSP Commands`, `Visual Features`.
- `TestReadmeLabels` (2 tests) fails on the old code and passes on the new one.
- The Hypothesis property `test_page_titles_match_object_labels` derived its expected label
  with the old query, so it expected `Readme` for a namespace readme. It now uses the new rule
  (first level-1 heading, `__Namespace` / `__Workspace` included).
- `make mkdocs-test`, `make test-fast`, `make validate-docs` and `make md-lint` exit 0.
- Code review: self-review only.
