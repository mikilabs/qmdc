# QMD-99: Findings

## The edge queries sort labels as text [[qmd99_finding_cause: Finding]]

`compute_graph_context()` in `qmdc-mkdocs/qmdc_mkdocs/graph.py` ordered both edge queries with
`ORDER BY e.edge_type, t.__label` (and `s.__label`). SQLite compares text byte by byte, so `10. …`
sorts before `2. …`. The lists are now sorted in Python after the query by edge type, then by a
natural key of the displayed label (digit runs as numbers, text case-insensitively), with the
label itself as the last tie-break so the order is fully determined. Siblings and the generated
nav are covered by their own issues (#40, #39) and are not changed here.

- category: bug
- related_to: [[#qmd99_edge_order]]
- affected_files: [qmdc-mkdocs/qmdc_mkdocs/graph.py]
- solution: `_natural_key` / `_edge_sort_key` applied to both edge lists.
