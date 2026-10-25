# QMD-87: graph traverse and find_path follow reference text, not resolved references

## Graph edges are not resolved [[qmd87_graph_unresolved: Bug]]

GitHub [#18](https://github.com/mikilabs/qmdc/issues/18). `qmdc_traverse_graph` and
`qmdc_find_path` build edges with a regex over field text (`core/graph.rs::extract_edges`) and use
the text inside `[[#...]]` as the target id. Any reference whose text is not the target's `__id`
becomes a dangling edge the BFS cannot follow: a hierarchical local id (`[[#b]]` for `part.b`), a
namespaced reference (`[[#svc:gateway]]`) and a cross-workspace reference (`[[#lib::util]]`). An
unresolvable reference (`[[#nowhere]]`) becomes an edge to a node that does not exist.
`workspace validate` and the SQL `edges` table resolve all of these, so the tools disagree with both.

- status: triage_review
- priority: high
- category: mcp
- related_task: [[#qmd86_graph_quadratic]]
- findings: [[#qmd87_finding_cause]], [[#qmd87_finding_source]], [[#qmd87_finding_find_references]], [[#qmd87_finding_tests]], [[#qmd87_finding_questions]]
- result: [[#qmd87_result]]

### Goals [[goals: [Goal]]]

#### A1: Failing fixtures for each kind of reference [[qmd87_goal_a1]]

Data-driven `tests/mcp/qmd87-*` fixtures on one container with two workspaces: traverse in both
directions and find_path across a hierarchical, a namespaced and a cross-workspace reference, plus
a `qmdc_query_sql` fixture that pins the `edges` table the graph must match.

- group: A_tests
- done: true

#### B1: Graph edges are the resolved edges [[qmd87_goal_b1]]

`traverse` and `find_path` take their edges from the index's resolved `edges` table: the target is
the resolved `__id`, and an unresolvable or ambiguous reference is not an edge.

- group: B_fix
- done: false
