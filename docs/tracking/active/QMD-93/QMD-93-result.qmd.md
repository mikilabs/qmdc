# QMD-93: Result

## Hidden objects' backlinks credit their page [[qmd93_result: Result]]

With `hidden_kinds`, an incoming edge from a hidden object now shows the page that holds it in
"Linked from", once per page and edge type, matching what that page shows in "Links to".

- feature: [[#qmd93_hidden_kinds_backlink]]
- completed: 2026-11-26
- files_changed: [qmdc-mkdocs/qmdc_mkdocs/graph.py]
- tests_changed: [qmdc-mkdocs/tests/test_graph.py]

### Verification [[qmd93_result_verification: text]]

- about: [[#qmd93_result]]

- `TestHiddenKinds::test_hidden_source_is_credited_to_its_page` fails on the old code (empty
  "Linked from") and passes after the fix. Two `Score` objects in one file give one Product entry;
  a file whose top-level object is a `Score` gives none.
- `test_without_hidden_kinds_sources_are_listed` and `test_links_to_side_is_unchanged` pass before
  and after.
- `make mkdocs-test`, `make test-fast`, `make validate-docs` and `make md-lint` exit 0.
- Code review: self-review only.
