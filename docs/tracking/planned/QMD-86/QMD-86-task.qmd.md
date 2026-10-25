# QMD-86: graph traverse and find_path are quadratic in workspace size

## traverse and find_path scan every edge for every node they visit [[qmd86_graph_quadratic: Bug]]

GitHub [#17](https://github.com/mikilabs/qmdc/issues/17). `qmdc_traverse_graph` and
`qmdc_find_path` (qmdc-rs MCP) take O(visited × (E + N)) time. For every node the BFS dequeues,
both scan the whole edge list for neighbours, and both find the node itself with a linear search
over all nodes. On the issue's synthetic ring workspace a 32k-object traversal takes 7.4 s and a
path 8.0 s in a release build, about 4× per doubling of N; on a 387k-object, 5.56M-edge workspace
that extrapolates to minutes for a traversal and an hour for a path.

The fix must not change any output: same nodes, edges and order.

- status: triage_review
- priority: high
- category: mcp
- findings: [[#qmd86_finding_cause]], [[#qmd86_finding_order]], [[#qmd86_finding_scope]], [[#qmd86_finding_tests]], [[#qmd86_finding_questions]]
- result: [[#qmd86_result]]

### Goals [[goals: [Goal]]]

#### A1: Failing regression test for the scaling [[qmd86_goal_a1]]

A Rust test builds a 30k-object ring workspace in memory and runs a whole-graph `traverse` and a
cross-graph `find_path`, each under a time bound that linear work meets with a wide margin and the
quadratic code misses by a wide margin.

- group: A_tests
- done: true

#### A2: Golden fixtures pin the current output order [[qmd86_goal_a2]]

Data-driven `tests/mcp` fixtures record today's exact output of `traverse` (both directions, and
incoming with an edge-type filter) and `find_path` on a workspace with a self-loop, repeated
references and a dangling reference. No such fixture existed.

- group: A_tests
- done: true

#### B1: O(1) node and neighbour lookup [[qmd86_goal_b1]]

`traverse` and `find_path` build the node map and outgoing/incoming edge lists once per call and
use them for every BFS step. Output is unchanged.

- group: B_fix
- done: false
