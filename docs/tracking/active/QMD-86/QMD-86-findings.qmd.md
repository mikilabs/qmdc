# QMD-86: Findings

## Every BFS step scans the whole graph [[qmd86_finding_cause: Finding]]

Both operations build `all_nodes` (`extract_nodes`) and `all_edges` (`extract_edges`) once per
call, which is linear, and then search them linearly on every step. `traverse` calls
`find_adjacent_edges`, a loop over every edge, for each dequeued node, and finds the node itself
with `all_nodes.iter().find(..)`. `find_path` has the same inner `for edge in &all_edges` loop and
the same linear node search when it assembles the path. Cost is O(visited × (E + N)).

Reproduced on `db3b912` (2.0.1 plus #16) with the issue's script and a release build: traverse
0.56 s at 8k objects and 7.44 s at 32k; find_path 0.35 s and 7.99 s. Parse and index time grow
linearly (0.86 s, 3.64 s) and are not part of the problem.

- category: bug
- related_to: [[#qmd86_graph_quadratic]]
- affected_files: [qmdc-rs/src/core/graph.rs, qmdc-rs/src/core/ops/traverse.rs, qmdc-rs/src/core/ops/find_path.rs]
- affected_functions: [traverse, find_adjacent_edges, find_path]
- solution: A `GraphView` in `core/graph.rs`, built once per call from the same `extract_nodes`/`extract_edges` output, holds a first-wins id -> node map and, per id, the ascending indices of its outgoing and incoming edges. Both operations ask it for a node or for the adjacent edges of a node in a direction, so every step is O(1) plus its own degree.

## The output order must survive [[qmd86_finding_order: Finding]]

The old scan visited edges in edge-list order and, for direction `both` and for `find_path`,
took an edge as outgoing when its source was the node and as incoming only otherwise, so a
self-loop counted once with the node as its target. The view keeps that exactly: each id's edge
lists are in edge-list order, and for `both` the two lists are merged by index, with an index in
both (a self-loop) taken once from the outgoing side. The first node of a duplicated id wins, as
`iter().find` did. A dangling target (a `[[#ref]]` with no object) is still visited and expanded
but produces no node, as before.

- category: design
- related_to: [[#qmd86_graph_quadratic]]
- solution: Verified by diffing full tool output, old binary against new, over 966 calls (three directions, three depths, edge-type filters, path pairs and the error cases) on the `docs` workspace, the issue's 2,000-object ring and a workspace with self-loops, repeated and dangling references. All 966 are byte-identical.

## Out of scope [[qmd86_finding_scope: Finding]]

- category: design
- related_to: [[#qmd86_graph_quadratic]]
- solution: Edges still come from the regex scan of `[[#ref]]` text. Building them from resolved references is GitHub #18, which changes output and is a separate task; the view gives it one place to change. The view is not cached across calls either: per call it is O(V + E), the same order as the parse and index work every MCP call already does, and the issue allows either.

## Tests [[qmd86_finding_tests: Finding]]

- category: testing
- related_to: [[#qmd86_graph_quadratic]]
- solution: One Rust regression test for the scaling and three data-driven MCP fixtures for the output order.

### Test plan [[qmd86_finding_test_plan: text]]

- about: [[#qmd86_finding_tests]]

Existing tests: none. `tests/mcp` had no `traverse` or `find_path` fixture, and
`qmdc-rs/tests/mcp.rs` only checks that the two tools are listed.

New tests:

- `qmdc-rs/tests/graph_scaling.rs`: builds the issue's ring workspace (30k objects, 90k
  references) as an in-memory `ResolvedIndex`, so no parsing is timed, and asserts that a depth-50
  outgoing `traverse` reaches all 30k nodes and a `find_path` from `f0` to `f15000` succeeds, each
  within 5 s. A timing bound cannot be expressed as an `.sql` + `.expected.json` pair; the MCP tools
  exist only in qmdc-rs, so there is nothing to share with the other parsers. Unoptimized, the old
  code takes 32.7 s and 35.1 s (fails); the fix takes about 1.1 s for both tests together.
- `tests/mcp/qmd86-traverse-both-order`, `qmd86-traverse-incoming-edge-type`,
  `qmd86-find-path-undirected`: one workspace with a self-loop, an array with a repeated
  reference, two references in one string and a dangling reference. Expected output is the old
  binary's exact output, so these pass before and after and pin the order.

How to verify: `cargo test --test graph_scaling` fails on 2.0.1 for the documented reason (time
bound exceeded) and passes after the fix; `make test-fast` is green.

## Open questions [[qmd86_finding_questions: Finding]]

- category: process
- related_to: [[#qmd86_graph_quadratic]]
- solution: None. The issue states the expected complexity and that output must not change, and both are measurable.
