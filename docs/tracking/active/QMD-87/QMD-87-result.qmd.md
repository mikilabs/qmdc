# QMD-87: Result

## Graph edges are the resolved edges table [[qmd87_result: Result]]

`GraphView::build` (QMD-86) now takes its edges from `QmdcDatabase::resolved_edges()`, the
index's `edges` table joined back to `__id`, in insertion order, instead of from the `[[#...]]`
text. Every target is a resolved `__id` from the resolver `workspace validate` agrees with
([[#qmd87_finding_source]]): `[[#b]]` reaches `part.b`, `[[#svc:gateway]]` and `[[#lib::util]]`
are followed, and `[[#nowhere]]` is no edge. `traverse` and `find_path` therefore return what
`SELECT ... FROM edges` returns. A failure to read the in-memory database is an
`internal-error` envelope rather than an empty graph.

Behaviour changes beyond the bug, both taken from the edges table: a reference repeated within
one field is a single edge, and a text-field preamble line uses its key as `edge_type`.
`describe_metamodel` still counts edge types from the text scan and is unchanged.

Not changed: `find_references` ([[#qmd87_finding_find_references]]).

- feature: [[#qmd87_graph_unresolved]]
- files_changed: [qmdc-rs/src/core/graph.rs, qmdc-rs/src/core/ops/traverse.rs, qmdc-rs/src/core/ops/find_path.rs, qmdc-rs/src/db/mod.rs, qmdc-rs/tests/graph_scaling.rs, tests/mcp/qmd86-traverse-both-order/mcp-expected.json, tests/mcp/qmd86-traverse-incoming-edge-type/mcp-expected.json]
- tests_added: [tests/mcp/qmd87-traverse-outgoing, tests/mcp/qmd87-traverse-incoming, tests/mcp/qmd87-find-path-hierarchical, tests/mcp/qmd87-find-path-cross-workspace, tests/mcp/qmd87-query-edges]

### Verification [[qmd87_result_verification: text]]

- about: [[#qmd87_result]]

- The four qmd87 graph fixtures fail on the QMD-86 code (items 1 instead of 4, path length 2
  instead of 1, no path) and pass after; `qmd87-query-edges` passes both times.
- The two QMD-86 fixtures with a repeated reference and a dangling one were re-derived. The new
  expected edges and nodes were written out by hand from the edges table first, then checked
  against the tool output: identical.
- Cross-check of every object's outgoing edges (traverse, depth 1) against the `edges` table:
  identical on the fixture container, on a namespaced workspace and on the 1,705 edges of `docs`.
  Before the fix, `docs` had 701 graph-only and 257 table-only edges.
- `graph_scaling` still passes; it now fills the index database the way the real index does. The
  issue #17 script: 32k objects, traverse 0.18 s, find_path 0.07 s (release).
- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` exit 0.
- `find_references` disagrees with the `edges` table for 632 of the 777 objects in `docs`, both
  before and after this change, in both directions (for example it counts a reference from the
  `docs` workspace root, which the table does not resolve, and it does not count generated
  document objects, which the table does).
- Not run locally: `make py-test` (`npm ci` refuses on this checkout, see QMD-86).
- Code review: self-review, no review agent could be started from this session.
