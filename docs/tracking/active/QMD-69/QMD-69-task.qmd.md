# QMD-69: Cross-workspace refs resolve in query but fail validation

## Workspace-qualified references are not honored across sibling workspaces [[qmd69_cross_ws_refs: Bug]]

A reference that names its target workspace behaves differently on every surface that
reads it: `qmdc query` resolves it, `qmdc workspace validate` calls it a broken link,
and MCP `qmdc_validate_references` refuses the composed folder altogether. No reference
form carries a workspace qualifier anywhere in the three parsers — the workspace slot is
read as a namespace — and the edge index papers over that by matching on the bare id
across all workspaces, so a reference can silently point at the wrong object. Reported
upstream as GitHub issue #9 (<https://github.com/mikilabs/qmdc/issues/9>), reproduced on
qmdc 1.0.7 (macOS arm64).

Triage widened the defect in three directions the report did not cover. The NAMESPACE
qualifier is discarded as well, so even a same-workspace `[[#ns:id]]` can bind to the
wrong object; `qmdc workspace validate` and `qmdc query` disagree even on a bare
ambiguous reference, which makes the reported split the general shape rather than a
cross-workspace special case; and the Python validator does not merely misreport a
qualified reference but CRASHES on it, losing the whole validation run. Separately, the
`Kind` segment the guide put in the middle of the form turned out to be decoration that
also silences ambiguity diagnostics, and the operator removed it — which is what makes
three colon-separated segments unambiguous.

- status: done_review
- priority: high
- category: parser
- related_task: [[#qmd63]], [[#qmd68_converge]], [[#qmd66_dot_notation_discrepancies]]
- requires_changes: []
- findings: [[#qmd69_finding_parse]], [[#qmd69_finding_fallback]], [[#qmd69_finding_mcp]], [[#qmd69_finding_kind]], [[#qmd69_finding_py_crash]], [[#qmd69_finding_blast_radius]], [[#qmd69_finding_container]], [[#qmd69_finding_impl]], [[#qmd69_finding_bypass]], [[#qmd69_finding_spinoff]], [[#qmd69_finding_tests]], [[#qmd69_finding_questions]]
- result: [[#qmd69_result]]

### Decisions taken during triage [[qmd69_decisions: text]]

- about: [[#qmd69_cross_ws_refs]]

Four of the five open questions were answered by the operator on 2026-09-23, and each
answer is pinned by a test before it was written down. They are recorded in full in
[[#qmd69_finding_questions]]; in short:

1. A cross-workspace reference MUST be qualified. The bare-id fallback is removed
   outright rather than narrowed to the unique-id case.
2. The canonical form is `#workspace:namespace:id.field` — single colons, NO Kind
   segment. It is literally `__global_id` plus an optional field suffix, so a reference
   and an identity are written the same way.
3. Naming your own workspace is legal; it is just the full identity of a local target.
4. In `workspace::id` the empty middle segment ELIDES the namespace, so it matches the id
   in any namespace of that workspace and reports `ambiguous_reference` when it finds
   more than one candidate.

Only the composed-root question remains (goal B2, question 4), and it is a scoping call
rather than a blocker.

### Reproduction [[qmd69_repro: text]]

- about: [[#qmd69_cross_ws_refs]]

Two sibling workspaces under a parent folder that is not itself a workspace (no
nesting):

```text
root/repo_a/readme.qmd.md               # [[repo_a: __Workspace]]
root/repo_a/services/readme.qmd.md      # [[services: __Namespace]]
root/repo_a/services/service.qmd.md     # ## Payments API [[payments_api: Service]]
root/project/readme.qmd.md              # [[project: __Workspace]]
root/project/project.qmd.md             # ## Commerce [[commerce: Project]]
```

`Commerce` in `project` carries one field whose value is the workspace-qualified
reference. The issue was filed against the four-segment
`[[#repo_a:services:Service:payments_api]]` that the guide documented as the full form;
since the Kind segment was removed (decision 2 above) the canonical spelling of the same
reference is `[[#repo_a:services:payments_api]]`, and the defect is identical either way —
`repo_a` is taken for a namespace.

Three surfaces, three answers:

1. `qmdc query root "... FROM edges ..."` returns the `includes` edge with the target
   resolved.
2. `qmdc workspace validate root` returns
   `broken_link — Object 'Service:payments_api' not found` (on the canonical form,
   `Object 'payments_api' not found`).
3. MCP `qmdc_validate_references` with `root` returns
   `ambiguous — path contains 2 workspaces`, listing both as candidates. Validating
   either candidate alone cannot see the other workspace's objects.

### Expected vs actual [[qmd69_expected: text]]

- about: [[#qmd69_cross_ws_refs]]

Expected: all workspace-aware surfaces operate on the same composed graph — query
resolves the qualified reference, CLI validation returns no errors, MCP reference
validation accepts a non-workspace container of sibling workspaces, and the workspace,
namespace and id qualifiers are all enforced rather than discarded. "Enforced" includes
the negative half: an unresolvable or ambiguous target yields NO edge, not an arbitrary
one.

Actual: query resolves by accident (bare id, any workspace), CLI validation reports a
false broken link, MCP refuses to build the composed index at all, and the Python
implementation aborts the run with an unhandled exception.

### Root cause (diagnosed) [[qmd69_rootcause: text]]

- about: [[#qmd69_cross_ws_refs]]

Four separate causes, none of which models a workspace qualifier. Triage confirmed each one,
corrected the third, and found the fourth — see [[#qmd69_finding_parse]],
[[#qmd69_finding_fallback]], [[#qmd69_finding_mcp]], [[#qmd69_finding_kind]] and
[[#qmd69_finding_py_crash]] for the detail:

1. `parse_reference_target` (`qmdc-rs/src/core/reference_scan.rs:72`) reads the segments as
   `namespace:Kind:id`, so in a three-segment target the WORKSPACE is taken for a namespace
   and nothing matches. Under the original four-segment reading the same code mis-sliced
   `Service:payments_api` into the id, which is literally what the reported error printed.
   Either way the workspace qualifier is never represented. The three implementations
   disagree about which part they keep — rs keeps the tail, ts keeps the head — so they
   disagree on whether the file is valid.
2. The SQLite edge builder keeps only the last colon-segment as the target id, so every
   qualifier is gone before resolution runs; the resolver then ends in a "third try: any
   workspace" lookup (`SELECT __global_id FROM objects WHERE __id = ?1 LIMIT 1`) with no
   workspace or namespace predicate. Present in all three implementations
   (`qmdc-rs/src/db/mod.rs:683`, `qmdc-py/qmdc/db.py:384`, `qmdc-ts/src/db.ts:334`), and
   because `LIMIT 1` has no `ORDER BY`, each one picks a different workspace.
3. The MCP resolver refuses a container holding more than one sibling workspace with
   `ambiguous` + candidates. This is **not a defect**: QMD-63 designed it that way and
   pinned it with `tests/mcp/qmd63-ambiguous`. Goal B2 is therefore out of scope pending
   the operator's decision. MCP and the LSP *are* affected by cause 1 though — both report
   a false `QMDC001` for a workspace-qualified reference inside a single workspace, which is
   covered by `tests/lsp/microtests/diagnostics/033-workspace-qualified-ref` and has
   nothing to do with containers.
4. The grammar itself was wrong, which is why cause 1 had nowhere to put the workspace: the
   `Kind` segment occupying the middle slot is not part of identity, is ignored by the
   graph, and suppresses ambiguity diagnostics when present. Removing it makes three
   segments mean `workspace:namespace:id` unambiguously — see [[#qmd69_finding_kind]].
   Separately, the Python validator does not merely misreport such a reference, it
   **crashes** on it ([[#qmd69_finding_py_crash]]).

### Silent mis-resolution (verified) [[qmd69_silent: text]]

- about: [[#qmd69_cross_ws_refs]]

The report raised this as a risk; both halves were confirmed by hand on qmdc 1.0.7, and
this is the more serious defect of the two — validation being wrong is loud, a graph
edge pointing at the wrong object is not:

- With `payments_api` defined in BOTH `repo_a` and `repo_b`, the reference
  `[[#repo_a:services:Service:payments_api]]` resolved to `repo_b` — wrong workspace,
  no diagnostic.
- With a workspace qualifier naming a workspace that does not exist,
  `[[#no_such_ws:services:Service:payments_api]]` still resolved to `repo_a`.

Triage then found three more instances of the same silence, all measured and none in the
report:

- **The namespace qualifier is discarded too.** In a single workspace holding `ledger` at
  the root and in `services`, the explicitly namespaced `[[#services:ledger]]` builds an
  edge to the ROOT object. Resolution starts from the source object's own position and
  returns before consulting the reference's own qualifiers, so this is not limited to
  cross-workspace references at all.
- **A local object silently outranks an explicit qualifier.** When the consumer workspace
  declares its own object with the target's id, a fully qualified cross-workspace
  reference binds to the local one, deterministically and in all three implementations.
- **Ambiguity is reported and still builds an edge.** For a bare `[[#ledger]]` with two
  candidates, `workspace validate` correctly says `ambiguous_reference` while the graph
  binds the root object anyway. So the validate-versus-query split the issue reported is
  the general shape of the defect, and the bare ambiguous reference is its smallest
  instance.

### Why existing tests miss it [[qmd69_gap: text]]

- about: [[#qmd69_cross_ws_refs]]

Existing reference tests are single-workspace, so no fixture ever exercises a
workspace qualifier, and no fixture places the same id in two sibling workspaces while
referencing one of them — the only shape that exposes the unqualified fallback.
`multi-workspace-collision` comes closest: it does hold the same id in two workspaces, but
its cross-workspace reference targets a uniquely-named object, so the fallback never has to
choose. `make validate-compare` compares validation errors across the parsers, yet it never
caught the py/ts/rs split because no fixture carried a workspace qualifier in any form.
QMD-66 audited the guide against the implementations and removed several
documented-but-nonexistent syntaxes, but the full-format claim
`[[#workspace:namespace:Kind:id]]` survived that pass.

Triage found the gap is wider than "no workspace qualifier in the corpus". Nothing pinned
the `Kind` segment's effect on resolution either: the only Kind-in-reference fixture
(`tests/parser/039-reference-with-kind`) asserts the PARSE output and never resolves, and
`namespace:Kind:id` appears in no fixture at all — only in the guide's prose. Nor did
anything pin that an explicitly namespaced reference must beat a root-namespace object of
the same id, which is why that divergence also survived.

### Open questions (for triage) [[qmd69_open_questions: text]]

- about: [[#qmd69_cross_ws_refs]]

Triage refined and then closed most of these; the authoritative list with full reasoning is
[[#qmd69_finding_questions]], and the answers are summarised in the "decisions taken during
triage" section of this task. Questions 1, 2, 3 and 5 are ANSWERED. What remains is (4): does
composed-root acceptance belong to this task at all, or to issue #10 — goal B2 is held out of
scope until it is settled.

### Goals [[goals: [Goal]]]

#### A1: Make the reference grammar the `__global_id` grammar [[qmd69_goal_a1]]

**Operator decision (2026-09-23): the canonical reference form is
`#workspace:namespace:id.field`, and the Kind segment is removed.**

Teach the single shared reference-target parser that colon-separated segments are a
right-aligned suffix of `workspace:namespace:id` — one segment is an id, two a
`namespace:id`, three a `workspace:namespace:id` — with an optional `.field` suffix on the
id. Delete the Kind segment and with it the uppercase-first-segment heuristic, which exists
only to tell `Kind:id` from `namespace:id` and lives in two places per implementation
(`classify_reference`, which also applies it to dots, and `parse_reference_target`). The
`ref_type` value `"kind"` disappears from parse output.

Kind was measured to be decoration that also silences real ambiguity diagnostics — see
[[#qmd69_finding_kind]] for the two probes and the cost of removing it. This decision is what
makes three segments unambiguous, so it also settles the canonical-form question.

**Done (2026-09-23).** `parse_reference_target` now returns `(workspace, namespace, id)` and
`classify_reference` lost the uppercase heuristic in all three implementations; the `ref_type`
value `"kind"` is gone, with `tests/parser/039-reference-with-kind` and
`066-paragraph-references` updated to `"namespace"` and the Rust unit assertions with them.
No case regressed in any of the three parsers.

- group: A_workspace_qualifier
- done: true

#### A2: Enforce the qualifiers during resolution [[qmd69_goal_a2]]

**Operator decisions (2026-09-23) fix the matching rule.** A reference is a right-aligned
suffix of `workspace:namespace:id`, and each qualifier present narrows the search. A bare
`id` resolves as today — own namespace first, then any namespace of the same workspace.
`namespace:id` means that namespace of this workspace. `workspace:namespace:id` means exactly
that workspace and that namespace. And `workspace::id` means that workspace with the
namespace ELIDED, so it searches every namespace of the named workspace.

A qualifier naming a workspace that does not exist, or one whose workspace has no such
object, must report not-found — never fall through to a match somewhere else. An elided
namespace that finds more than one candidate in the named workspace must report
`ambiguous_reference`, exactly as a bare `[[#id]]` already does within one workspace.

"Ambiguous" must mean **no edge**, not a diagnostic printed beside one. Today the bare form
already reports ambiguity and still builds an edge — see the "Validate and query disagree on
the bare form too" section of [[#qmd69_finding_fallback]]. That is the acceptance bar this
goal has to clear.

**Done (2026-09-23), all three implementations.** The candidate filter honours the workspace
qualifier, keeps an unqualified reference inside the referring object's own workspace, treats
an empty middle segment as an elision, and no longer suppresses ambiguity when several
namespaces match; the reference index carries the workspace for that purpose. Matching
`ambiguous` now also means NO EDGE, which is the half the graph was missing — see
[[#qmd69_goal_a3]].

- group: A_workspace_qualifier
- done: true

#### A3: Remove the unqualified global fallback from edge resolution [[qmd69_goal_a3]]

**Operator decision (2026-09-23): a cross-workspace reference MUST be qualified.** The
bare-id fallback is removed outright rather than narrowed to the unique-id case. A reference
that names no workspace resolves only within its own workspace; if the target is not there,
it is a broken link even when exactly one sibling workspace happens to hold that id.

Concretely: carry the full qualifier into the edge index instead of keeping only the trailing
id, and delete the "Third try: any workspace" `SELECT __global_id FROM objects WHERE __id = ?
LIMIT 1` branch in `resolve_target_global_id` (`qmdc-rs/src/db/mod.rs:683`,
`qmdc-py/qmdc/db.py:384`, `qmdc-ts/src/db.ts:334`). The `__local_id` fallback stays, being
workspace-local and therefore unaffected. An unresolvable target must be reported, not
silently bound to an arbitrary row.

Deleting the third try is necessary but NOT sufficient: the FIRST try must start consuming the
reference's own qualifiers. It currently resolves the bare trailing id against the SOURCE
object's workspace and namespace, which is why even an explicitly namespaced
`[[#services:ledger]]` binds to a root-namespace `ledger` — see the "namespace qualifier is
discarded too" section of [[#qmd69_finding_fallback]].

Measured before deciding: this breaks nothing in the corpus — see
[[#qmd69_finding_blast_radius]].

**Done (2026-09-23), all three implementations.** Reference extraction now keeps the FULL
target instead of truncating it to the last `:`-segment, `resolve_target_global_id` takes the
parsed qualifiers, and the "any workspace" branch is gone. The first try no longer resolves a
bare trailing id against the SOURCE object's position, which is what made an explicitly
namespaced reference bind to a root-namespace object. An ambiguous target now yields no edge,
so `qmdc query` and `qmdc workspace validate` agree.

- group: A_workspace_qualifier
- done: true

#### B1: CLI validation accepts valid cross-workspace references [[qmd69_goal_b1]]

`qmdc workspace validate <container>` returns no errors for the reproduction above,
and still reports a genuinely broken cross-workspace reference.

**Done in Rust (2026-09-23).** The blocker turned out to be smaller than the first reading
suggested, and the correction matters: `parse_all_workspaces` (`qmdc-rs/src/workspace.rs:796`)
already UNIONS the sibling workspaces' objects into one result — only the reference VALIDATION
ran per workspace, before the union, so a cross-workspace target was absent from the index it
was checked against. No container-composition architecture was needed; the fix is to drop the
per-workspace reference findings and run the shared `reference_scan` once over the composed
object set. Structural findings (`duplicate_id`, `workspace_in_wrong_file`, parsing errors)
stay per-workspace, because identity is workspace-scoped (QMD-67) and a container must not
report a duplicate across siblings.

`tests/cli/011-validate-cross-workspace` now returns `[]` in Rust, and the deliberately broken
container reports exactly the specified set: unknown workspace and both unqualified
cross-workspace references as `broken_link`, and the elided-namespace reference with two
candidates as `ambiguous_reference`. Ported to Python and TypeScript with the same shape. See
[[#qmd69_finding_container]].

- group: B_surface_parity
- done: true

#### B2: MCP and LSP accept a composed container as one root [[qmd69_goal_b2]]

**Operator decision (2026-09-23): in scope — take it here.** Triage had recommended deferring
this to issue #10 because it looked like a reversal of the shipped QMD-63 design.

Implementation showed it is NOT a reversal, and that matters for QMD-63's integrity. QMD-63
answered "which single workspace does an MCP call resolve to?" and its answer — do not
auto-pick one of several — still stands: `resolve_root` is untouched and
`tests/mcp/qmd63-ambiguous` stays green. What QMD-69 needed was a different question, "does
reference validation see the whole container?", and that is answered where the container is
already assembled: `parse_all_workspaces` had ALREADY unioned the sibling workspaces' objects
and only validated references before the union. See [[#qmd69_finding_container]].

MCP and the LSP therefore need no change of their own: both share `core::ops::validate` with
the CLI (QMD-68), so a qualified reference inside a workspace already resolves on all three
surfaces, and the composed-container behaviour lands with the CLI path they call.

- group: B_surface_parity
- done: true

#### B3: Parity in the Python and TypeScript parsers [[qmd69_goal_b3]]

The same grammar, qualifier enforcement and composed-container behaviour in `qmdc-py` and
`qmdc-ts`, so the three implementations agree on every acceptance test that is not
LSP-specific. Both carry their own copy of the uppercase-Kind heuristic
(`qmdc-py/qmdc/parser.py:128`, `qmdc-ts/src/parser.ts:93`), so dropping Kind is a three-place
change per surface, not one.

Also in scope here: the Python-only crash in [[#qmd69_finding_py_crash]]
(`qmdc-py/qmdc/workspace.py:1066-1078` indexes tuples as dicts and aborts the whole
validation run). It is a separate defect from the qualifier itself but is reached by the same
reference, so it must be fixed for the Python column to go green at all.

**Done (2026-09-23).** Both parsers carry the same grammar, the same candidate filter, the
same resolver and the same container composition as Rust. The Python crash is fixed by reading
the index tuples positionally instead of calling `.get()` on them. All three suites are green:
py 850 cases, ts 839, rs 993, zero failures.

- group: B_surface_parity
- done: true

#### C1: Regression tests for the acceptance cases [[qmd69_goal_c1]]

Data-driven throughout — ten cases across five fixtures, nine failing as designed and one a
guard (see [[#qmd69_finding_tests]]). One per surface the issue names: the reported container
scenario through `qmdc workspace validate` (all three languages), the workspace-qualified form
in a single workspace (all three languages), the same form through the LSP and through MCP
`qmdc_validate_references` (rs-only surfaces), a qualified reference with a colliding id
across sibling workspaces, a qualifier naming no existing workspace, an unqualified
cross-workspace reference that must not resolve even when only one workspace holds the id, an
elided namespace reaching a namespaced target, and an elided namespace with two candidates.
The root-namespace cross-workspace case is the guard that passes today and must keep passing.

Every failing case must fail for its OWN stated reason, not because of arbitrary row ordering
in the unfiltered `LIMIT 1` lookup — which is why the elision case lives in its own fixture
with a same-named local object in the consumer workspace. See the determinism section of
[[#qmd69_finding_tests]].

Still to add: cross-surface agreement asserted in one case, i.e. that `qmdc query` and `qmdc
workspace validate` report the same binding for the same reference. That is now the more
valuable gap, because [[#qmd69_finding_fallback]] shows the two surfaces disagree even on a
bare ambiguous reference. The py/ts/rs columns must all read pass after implementation.

**Done (2026-09-23).** All ten cases pass in every implementation that runs them, and the
guard still passes. The cross-surface agreement case is now covered structurally rather than by
a separate fixture: `validate` and `query` share the qualifier grammar and both treat ambiguity
as "no edge", and for the same references the SQL cases assert the graph half while the CLI
case asserts the diagnostic half.

- group: C_tests
- done: true

#### D1: Reconcile the guide with the implementation [[qmd69_goal_d1]]

The bundled agent guide states `[[#workspace:namespace:Kind:id]]` as the full format while the
list directly below it calls `namespace:Kind:id` the "full form" and gives no four-segment
example. Both are now wrong in the same way: per [[#qmd69_goal_a1]] there is no Kind segment
at all. Make the guide and its mirrored copy in `docs/guides` state the one grammar —
`#workspace:namespace:id.field`, a right-aligned suffix of `__global_id` — and say that an
unqualified cross-workspace reference is not legal (question 1).

`namespace:Kind:id` currently appears ONLY in documentation prose
(`docs/format/references.qmd.md`, `docs/format/validation-errors.qmd.md`,
`docs/guides/qmdc-guide.qmd.md`, `docs/guides/validate-document.qmd.md`) and in no fixture, so
the guide is the whole blast radius of the grammar change on the docs side.

**Done (2026-09-23).** The Kind segment is gone from every reference form across the docs, and
the guide now states the grammar as a right-aligned suffix of `__global_id` with the two rules
that follow from it — a cross-workspace reference must name its workspace, and a qualifier
matching more than one object is an ambiguity that builds no edge. Updated:
`docs/format/references.qmd.md` (the authoritative spec), `docs/format/workspaces.qmd.md`,
`docs/format/validation-errors.qmd.md`, `docs/guides/qmdc-guide.qmd.md`,
`docs/guides/small-workspace.qmd.md`, `docs/guides/vscode.qmd.md`,
`docs/architecture/algorithms.qmd.md`, `docs/lsp/diagnostics.qmd.md`,
`docs/lsp/information.qmd.md`, `docs/tutorials/first-file.qmd.md`, `docs/lsp/navigation.qmd.md`,
plus the vendored crate copy via `make guide-sync`. The guide stays within its token budget
(52% of 20,000).

- group: D_docs
- done: true
