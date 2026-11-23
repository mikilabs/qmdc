# QMD-86: Result

## traverse and find_path run in O(V + E) per call [[qmd86_result: Result]]

A `GraphView` in `core/graph.rs` is built once per call from the same `extract_nodes` and
`extract_edges` output as before. It maps each id to its node (first object wins) and to the
ascending indices of its outgoing and incoming edges. `traverse` and `find_path` ask it for a node
and for the edges adjacent to a node, so each BFS step costs its own degree instead of a scan of
the whole graph. For direction `both`, and for `find_path`, the two index lists are merged in
edge-list order and a self-loop is taken once from the outgoing side, which is exactly what the
old full scan produced ([[#qmd86_finding_order]]). The old `find_adjacent_edges` is gone.

Output is unchanged. Edges still come from `[[#ref]]` text; resolving them is GitHub #18.

- feature: [[#qmd86_graph_quadratic]]
- files_changed: [qmdc-rs/src/core/graph.rs, qmdc-rs/src/core/ops/traverse.rs, qmdc-rs/src/core/ops/find_path.rs]
- tests_added: [qmdc-rs/tests/graph_scaling.rs, tests/mcp/qmd86-traverse-both-order, tests/mcp/qmd86-traverse-incoming-edge-type, tests/mcp/qmd86-find-path-undirected]

### Verification [[qmd86_result_verification: text]]

- about: [[#qmd86_result]]

- The issue's script, release build, before and after (seconds, parse and index subtracted):

  ```text
       N  edges  traverse before/after  find_path before/after
    2000   6000         0.03 / 0.01           0.04 / 0.00
    8000  24000         0.56 / 0.04           0.35 / 0.03
   32000  96000         7.44 / 0.20           7.99 / 0.08
  ```

- `cargo test --test graph_scaling` (debug): fails on the old code (traverse 32.7 s, find_path
  35.1 s against a 5 s bound) and passes after the fix (1.1 s for both tests).
- Full tool output diffed, old binary against new, over 966 calls on the `docs` workspace, the
  issue's 2,000-object ring and the fixture workspace (three directions, depths 1, 3 and 50,
  edge-type filters, 25 random path pairs per workspace, error cases): all byte-identical.
- The three new `tests/mcp` fixtures pass, and a planted wrong value in one of them fails the
  `mcp` suite, so the runner reads them.
- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` (the Rust CI
  job) all exit 0; `make validate-docs` passes.
- Not run locally: `make py-test`. Its cross-parser tests need `qmdc-ts` dependencies, and
  `npm ci` refuses on this checkout because `package-lock.json` lacks the optional `@qmdc/cli-*`
  packages (pre-existing on main, unrelated to this change). CI runs that job.
- Code review: done as a self-review against the old code, not in a separate agent, because no
  review agent could be started from this session. Checked self-loops in every direction, edges
  that sit in both lists, duplicate ids, dangling targets, the edge-type filter, depth cut-off, the
  `find_path` predecessor order and the same-node case; the 966-call diff covers each of them.
