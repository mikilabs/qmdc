# QMD-69: Result

## Workspace qualifiers are parsed, enforced, and agreed on by every surface [[qmd69_result: Result]]

Cross-workspace references now work, and the three surfaces that disagreed about them —
`qmdc query`, `qmdc workspace validate`, and MCP/LSP diagnostics — give one answer. The
reference grammar became the `__global_id` grammar, so a reference and an identity are written
the same way, and the `Kind` segment that never participated in identity is gone.

All eight goals are complete. `make test` is green end to end: **3325 cases, 0 failures**,
including cross-parser validation comparison and the guide token budget.

Late addition worth reading before the code: the operator asked whether hierarchical dotted ids
still worked. They did — but nothing in the corpus pinned a qualifier together with a dotted id,
and building that matrix out found **three more resolution paths that bypassed the qualifier
filter entirely**, two of them pre-existing defects that only became reachable once the
workspace qualifier existed. See [[#qmd69_finding_bypass]].

- feature: [[#qmd69_cross_ws_refs]]
- files_changed: [qmdc-rs/src/core/reference_scan.rs, qmdc-rs/src/db/mod.rs, qmdc-rs/src/workspace.rs, qmdc-rs/src/parser_modules/references.rs, qmdc-rs/src/lsp/document.rs, qmdc-rs/src/qmdc-guide.qmd.md, qmdc-py/qmdc/workspace.py, qmdc-py/qmdc/db.py, qmdc-py/qmdc/parser.py, qmdc-ts/src/workspace.ts, qmdc-ts/src/db.ts, qmdc-ts/src/parser.ts, qmdc-mkdocs/qmdc_mkdocs/references.py, docs/format/references.qmd.md, docs/format/workspaces.qmd.md, docs/format/validation-errors.qmd.md, docs/guides/qmdc-guide.qmd.md, docs/guides/small-workspace.qmd.md, docs/guides/vscode.qmd.md, docs/architecture/algorithms.qmd.md, docs/lsp/diagnostics.qmd.md, docs/lsp/information.qmd.md, docs/lsp/navigation.qmd.md, docs/tutorials/first-file.qmd.md]
- tests_added: [tests/cli/011-validate-cross-workspace, tests/cli/012-validate-qualified-hierarchical, tests/cli/013-validate-qualified-local-id-unknown-workspace, tests/cli/014-validate-id-shapes, tests/workspace/workspace-qualified-reference, tests/workspace/cross-workspace-qualifier, tests/workspace/cross-workspace-elision, tests/workspace/cross-workspace-hierarchical-ids, tests/workspace/id-shapes-qualified, tests/workspace/container-root-single-workspace/tests, tests/lsp/microtests/diagnostics/033-workspace-qualified-ref, tests/lsp/microtests/diagnostics/034-qualified-local-id-and-field-path]

### What changed [[qmd69_result_changes: text]]

- about: [[#qmd69_result]]

**The grammar.** A reference target is a right-aligned suffix of `workspace:namespace:id`
with an optional `.field` on the id. One segment is an id, two a `namespace:id`, three a
`workspace:namespace:id`; an empty middle segment (`ws::id`) ELIDES the namespace and matches
any namespace of that workspace. There is no `Kind` segment, and the uppercase-first-segment
heuristic that used to tell `Kind:id` from `namespace:id` is gone from all three parsers.

**Enforcement, on every path.** The validator's candidate filter and the graph's edge resolver
consume the same parsed qualifiers, and so do the three fallback paths that used to decide on
their own: the `__local_id` fallback on both surfaces and the field-reference escape. Two shared
predicates carry the rule — `workspace_matches` and `qualifiers_match` — so a path cannot drift
again. A reference that names a workspace matches only inside it and outranks a same-named local
object; one that names none stays inside its own workspace; an unknown workspace is a broken
link. Ambiguity means **no edge**, not a diagnostic printed beside one — which is what makes
`query` and `validate` agree.

One rule is deliberately NOT shared: on the `__local_id` path an unqualified reference is scoped
to the referring object's own namespace exactly, not "any namespace". Conflating that with the
elided form is the mistake this work made first, and two shipped fixtures caught it — it is
documented at each call site.

**The container.** `parse_all_workspaces` had already unioned the sibling workspaces' objects
and only validated references before the union, so a cross-workspace target was absent from the
index it was checked against. Reference findings are now recomputed once over the composed set;
structural findings stay per-workspace, because identity is workspace-scoped (QMD-67).

**Parity.** Python and TypeScript carry the same grammar, filter, resolver and composition as
Rust. The Python crash that aborted an entire validation run on a qualified reference is fixed.
`qmdc-mkdocs`, which reached into the private resolver and reimplemented the old grammar
inline, was ported with it.

### Verification [[qmd69_result_verification: text]]

- about: [[#qmd69_result]]

`make test`: 3325 cases, 0 failures. Per suite: py 882, ts 850, rs 1006, plus mkdocs 354 and the
remaining packages. `make validate-compare` reports 0 errors from each of the three parsers on
the docs workspace, `qmdc workspace validate ./docs` returns `[]`, MCP `qmdc_validate_references`
returns 0 diagnostics, `make md-lint` passes, and the agent guide is at 52% of its token budget.

A five-by-four id-shape matrix was added on top of the original ten cases, after the operator
asked whether hierarchical dotted ids were covered — they were not, in any form. It crosses five
id shapes (flat, hierarchical dotted, the `__local_id` leaf of a hierarchical object, and a field
path on each of the flat and hierarchical objects) with every qualifier form, on all four
surfaces: 20 intra-workspace and 8 cross-workspace graph combinations, 15 negatives asserted both
as "no edge" and as "reported by the validator", plus an LSP/MCP microtest. See
[[#qmd69_finding_bypass]] for the table.

Ten regression cases were written before the fix and now pass: the reported container scenario
through `qmdc workspace validate` (all three languages), the qualified form in a single
workspace (all three), the same form through the LSP and MCP, a qualified reference with a
colliding id, an unknown workspace qualifier, two unqualified cross-workspace references, an
elided namespace reaching a namespaced target, and an elided namespace with two candidates. The
root-namespace guard still passes, and one guard was ADDED during implementation for a
regression the corpus did not cover — a reference to the workspace root object.

### Out of scope, and what is left [[qmd69_result_scope: text]]

- about: [[#qmd69_result]]

`resolve_root` is untouched: QMD-63's decision not to auto-pick one of several workspaces for an
MCP call still stands, and `tests/mcp/qmd63-ambiguous` stays green. QMD-69 did not need it
overturned — see [[#qmd69_finding_impl]].

One defect found while authoring this task's documents was filed separately as
[[#qmd70_table_scope]] (a Markdown table inside an object-array element is captured by the parent
array) and parked by the operator. It shares no code path with this work, and its own regression
tests fail by design until it is implemented.

Issue #10 (explicit `--with` composition) is unaffected: this change composes a container that
is already being read as one, and adds no new way to name several roots.
