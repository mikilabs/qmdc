# QMD-93: Findings

## The two edge lists filter on different ends [[qmd93_finding_cause: Finding]]

`compute_graph_context()` in `qmdc-mkdocs/qmdc_mkdocs/graph.py` builds "Links to" from edges whose
source is any object in the page's file and then drops rows by the target's kind. "Linked from" is
dropped by the source's kind. An edge from a hidden `Score` in `product.qmd.md` to Quality therefore
stays on Product's side and vanishes from Quality's side. No test covered `hidden_kinds`.

- category: bug
- related_to: [[#qmd93_hidden_kinds_backlink]]
- affected_files: [qmdc-mkdocs/qmdc_mkdocs/graph.py]
- solution: Credit hidden incoming sources to their page instead of dropping them.

## Credit the page, keep the option as it is [[qmd93_finding_fix: Finding]]

A hidden incoming source is replaced by the top-level non-system object of its file, the same rule
`_get_file_label` uses for the page title, and listed once per page and edge type. When that page
object is itself a hidden kind, the row is dropped, as before. The issue also suggested a
`hidden_kinds_backlinks: page | drop` option to keep today's behaviour; it is left out, since
nothing in the report needs the old behaviour and it would add a config option. "Links to" is
unchanged.

- category: design
- related_to: [[#qmd93_hidden_kinds_backlink]]
- solution: `_credit_hidden_sources` in `graph.py`, three tests in `tests/test_graph.py`.
