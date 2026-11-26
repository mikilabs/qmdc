# QMD-99: Result

## Sidebar edge lists are in natural order [[qmd99_result: Result]]

"Links to" and "Linked from" are ordered by edge type and then naturally by label, so numbered
labels read 1, 2, 3, 10, 11.

- feature: [[#qmd99_edge_order]]
- completed: 2026-11-26
- files_changed: [qmdc-mkdocs/qmdc_mkdocs/graph.py]
- tests_changed: [qmdc-mkdocs/tests/test_graph.py]

### Verification [[qmd99_result_verification: text]]

- about: [[#qmd99_result]]

- `TestEdgeOrder` (links-to and linked-from, the issue's five axes) fails on the old code
  (`10` before `2`) and passes after the fix.
- The issue's reproduction with `qmdc-mkdocs init` and `build`: "Links to" on the built page
  reads Axis number 1, 2, 3, 10, 11.
- `make mkdocs-test`, `make test-fast`, `make validate-docs` and `make md-lint` exit 0.
- Code review: self-review only.
