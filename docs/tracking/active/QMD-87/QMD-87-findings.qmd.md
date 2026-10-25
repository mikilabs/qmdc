# QMD-87: Findings

## The graph uses the reference text as the target [[qmd87_finding_cause: Finding]]

`extract_edges` captures `[[#(...)]]` and stores the capture as `GraphEdge.target`. `GraphView`
(QMD-86) then compares targets with `__id` by string equality. On the fixture workspace,
`traverse` from `part.a` returns only `part.a`, with edges to `b`, `svc:gateway`, `lib::util` and
`nowhere`. `find_path part.a -> part.b` returns `part.a -> part -> part.b` through the parent's
structural fields, and `find_path gateway -> util` finds no path at all.

- category: bug
- related_to: [[#qmd87_graph_unresolved]]
- affected_files: [qmdc-rs/src/core/graph.rs, qmdc-rs/src/db/mod.rs]
- affected_functions: [extract_edges, GraphView::build]
- solution: Build the graph's edges from the resolved `edges` table instead of from text.

## Which resolver: the edges table, not ObjectIndex [[qmd87_finding_source: Finding]]

The issue names two references for "resolved": the `ObjectIndex` that `find_references` uses, and
the SQL `edges` table. They are not the same resolver. The edges table is built at index time by
`resolve_target_global_id` (QMD-69), which `workspace validate` agrees with: it understands
`ws::id` and `ws:ns:id`, never widens past the referring object's workspace, and drops ambiguous
targets. `ObjectIndex::resolve_object` has no workspace awareness: it reads `lib::util` as
namespace `lib`, id `util`. Parser `__references` also carry no field name, so building edges
from them would lose `edge_type`.

- category: design
- related_to: [[#qmd87_graph_unresolved]]
- solution: `GraphView::build` reads `QmdcDatabase::resolved_edges()`, the edges table joined to `objects` to turn global ids back into `__id`, in insertion order. Insertion order is object, then field, then reference order, so the order of the edges that survive does not change. Two visible differences follow from the table and are intended: a reference repeated in one field is one edge (the table's UNIQUE key), and a text-field preamble line (`- about: [[#a]]`) has its preamble key as `edge_type`, as in the table. A field-path reference (`[[#b.colour]]`) is an edge to `b`; the table's `target_field` is not exposed, since that would add a field to the tool output.

## find_references still disagrees with the edges table [[qmd87_finding_find_references: Finding]]

Measured on the fixture workspace and on a namespaced one: `find_references` builds referrers from
parser `__references`, which do not include a parent's structural child fields (`part` -> `part.a`
through field `a`, or a `__Namespace` -> its objects) nor the content of generated document
objects. The edges table and now the graph include both. So `find_references(part.a)` is `part.b`,
`gateway`, `util`, while the graph's incoming edges also have `part`. The other direction exists
too: `find_references` counts references made from a `__Workspace` root object, which the table's
resolver does not resolve: the root carries no `__workspace` of its own, so its references are
looked up in an empty workspace. On `docs` the two disagree for 632 of 777 objects.

- category: design
- related_to: [[#qmd87_graph_unresolved]]
- solution: Not changed here. Whether a parent's child fields count as references to the child is a decision about what `find_references` means; the issue asked for agreement assuming the two already matched. Recorded as an open question.

## Tests [[qmd87_finding_tests: Finding]]

- category: testing
- related_to: [[#qmd87_graph_unresolved]]
- solution: Five data-driven MCP fixtures; the QMD-86 fixtures are re-derived where the edges table changes them.

### Test plan [[qmd87_finding_test_plan: text]]

- about: [[#qmd87_finding_tests]]

Existing tests: the QMD-86 fixtures pin the old text-based edges, including a repeated reference
and a dangling one; their expectations change with this fix.

New fixtures, all on one container (`app` workspace with a `svc` namespace, sibling `lib`
workspace): `qmd87-traverse-outgoing` and `qmd87-traverse-incoming` from `part.a`,
`qmd87-find-path-hierarchical` (`part.a -> part.b`, length 1), `qmd87-find-path-cross-workspace`
(`gateway -> util` through `part.a`), and `qmd87-query-edges`, which pins the `edges` table the
four graph fixtures are derived from. The four graph fixtures fail on the current code for the
documented reason; the edges-table fixture passes before and after.

The fixtures assert no order that depends on which workspace of the container is discovered
first: container discovery follows the directory listing, which differs between hosts (Linux
lists `lib` before `app` here, macOS the other way round), and object order, so edge order,
follows it. So no graph fixture has `lib` edges adjacent to the start node, and the edges-table
fixture sorts its rows. Verified by running all five against a copy laid out so that `lib` is
discovered first.

How to verify: `cargo test --test mcp` and `make test-fast`.

## Open questions [[qmd87_finding_questions: Finding]]

- category: process
- related_to: [[#qmd87_graph_unresolved]]
- solution: Should `find_references` count structural child fields and generated-document content, as the edges table does? Out of scope here; raised on the issue.
