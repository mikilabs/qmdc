# QMD-69: Findings

Triage of [[#qmd69_cross_ws_refs]] — what the defect actually is, where it lives, what the
mandatory regression tests prove, and which part of the reported issue is not a defect at
all.

## Workspace qualifiers are parsed away, not enforced [[qmd69_finding_parse: Finding]]

No reference form carries a workspace qualifier anywhere in the three parsers. Nothing
carries one, so nothing can enforce one — and each implementation discards it differently,
which is why the surfaces disagree. The issue reported this against the four-segment
`workspace:namespace:Kind:id` form the agent guide documents; the operator has since removed
the Kind segment ([[#qmd69_finding_kind]]), so the canonical form is the three-segment
`workspace:namespace:id`, but the cause is unchanged — the workspace slot is read as a
namespace.

- category: parser
- related_to: [[#qmd69_cross_ws_refs]]
- affected_files: [qmdc-rs/src/core/reference_scan.rs, qmdc-py/qmdc/workspace.py, qmdc-ts/src/workspace.ts]
- affected_functions: [parse_reference_target, workspace._parse_reference, workspace._extract_id_from_reference, parseReference]
- solution: Return an explicit workspace qualifier from the shared reference-target parser, then enforce it during resolution — an unknown or non-matching workspace must resolve to not-found rather than to an object elsewhere.

### What each implementation does today [[qmd69_finding_parse_detail: text]]

- about: [[#qmd69_finding_parse]]

`parse_reference_target` (`qmdc-rs/src/core/reference_scan.rs:72`) reads segments as
`namespace:Kind:id`. Given the canonical `ws:ns:id` it returns namespace `ws`, Kind `ns`,
id `id` — so the workspace is taken for a namespace, nothing matches, and the reference is
a broken link. Given the originally-reported `ws:ns:Kind:id` it returned the id
`Kind:id`, the two trailing segments glued together, which is what the reported
`Object 'Service:payments_api' not found` message printed.

Python `_extract_id_from_reference` (`qmdc-py/qmdc/workspace.py:708`) and the TypeScript
equivalent take `parts[-1]`. On the four-segment form that landed on the correct bare id by
accident and reported no error, which is the **validation parity split** the issue exposed:
Rust said broken link, Python and TypeScript said clean. `make validate-compare` never caught
it because no fixture in the corpus carried a workspace qualifier in any form. On the
canonical three-segment form all three now fail — and Python fails hardest, by crashing
([[#qmd69_finding_py_crash]]).

## The edge index resolves by bare id across all workspaces [[qmd69_finding_fallback: Finding]]

Even with a qualifier present, the SQLite edge builder keeps only the last colon-segment
of the target, so every qualifier is gone before resolution starts. Resolution then ends
in an unfiltered "any workspace" lookup that takes the first row it finds. This is the
silent half of the bug: the graph gets an edge, it just may point at the wrong object.

- category: parser
- related_to: [[#qmd69_cross_ws_refs]]
- affected_files: [qmdc-rs/src/db/mod.rs, qmdc-py/qmdc/db.py, qmdc-ts/src/db.ts]
- affected_functions: [resolve_target_global_id, _resolve_target_global_id, resolveTargetGlobalId]
- solution: Carry the parsed qualifier into edge resolution instead of only the trailing id, and replace the unfiltered third-try lookup with a qualifier-respecting one. An unresolvable or ambiguous target must be reported rather than bound to an arbitrary row.

### The arbitrary pick differs per implementation [[qmd69_finding_fallback_detail: text]]

- about: [[#qmd69_finding_fallback]]

The offending lookup is `SELECT __global_id FROM objects WHERE __id = ?1 LIMIT 1` with no
workspace, namespace or Kind predicate — `qmdc-rs/src/db/mod.rs:683` ("Third try: any
workspace"), `qmdc-py/qmdc/db.py:384`, `qmdc-ts/src/db.ts:334`. The target id itself is
truncated to the last segment one level earlier (`db/mod.rs:628`, `db.py:353`).

`LIMIT 1` with no ORDER BY means the winner is whatever the storage layer returns first,
and that differs by implementation. On the new fixture, with the same id present in two
sibling workspaces:

| implementation | `includes` (names repo_a) | `bogus` (names a workspace that does not exist) |
| --- | --- | --- |
| Python | binds `qmd69_repo_b` — wrong | binds `qmd69_repo_b` — should not resolve |
| TypeScript | binds `qmd69_repo_a` — right by luck | binds `qmd69_repo_a` — should not resolve |
| Rust | binds `qmd69_repo_b` — wrong | binds `qmd69_repo_b` — should not resolve |

So the same workspace produces a **different graph depending on which parser reads it**,
with no diagnostic in any of them. Two objects that share an id across sibling workspaces
are legal — QMD-67 established that identity is scoped, and `multi-workspace-collision`
already ships a fixture where `task_workflow` exists in both `ws1` and `ws2`.

### The namespace qualifier is discarded too, not just the workspace [[qmd69_finding_fallback_ns: text]]

- about: [[#qmd69_finding_fallback]]

Found while probing the elision decision, and it widens this finding well past cross-workspace
references. The edge builder does not merely lose the WORKSPACE qualifier — it loses the
NAMESPACE qualifier as well, and resolves relative to the SOURCE object's position instead.

One workspace, `ledger` present both at the root and in `services`, referenced from a
root-level consumer:

| reference | edge built |
| --- | --- |
| `[[#ledger]]` | `elision_ws::ledger` |
| `[[#services:ledger]]` | `elision_ws::ledger` ✘ |

An explicitly namespaced reference binds to the ROOT object. The mechanism is the first try:
the trailing id `ledger` is resolved with `compute_global_id(workspace, <source's namespace>,
id)`, which hits the root object and returns before any qualifier is consulted. The
reference's own namespace never enters resolution at all.

This matters for scope: goal A3 cannot be "delete the third try". The edge builder has to
consume the parsed qualifiers from the reference in the FIRST try, or an explicitly
namespaced reference keeps binding to the wrong object even after the cross-workspace
fallback is gone.

### Validate and query disagree on the bare form too [[qmd69_finding_fallback_bare: text]]

- about: [[#qmd69_finding_fallback]]

Same probe, and a pre-existing defect independent of workspace qualifiers: for
`[[#ledger]]` with two candidates in one workspace, `workspace validate` correctly reports
`ambiguous_reference` — and the graph builds an edge to the root `ledger` anyway.

So the validate-versus-query split the issue reported is not specific to cross-workspace
references; it is the general shape of the defect, and the bare ambiguous reference is its
smallest instance. It also fixes the acceptance bar for [[#qmd69_goal_a2]]: "ambiguous" must
mean no edge, not merely a diagnostic printed beside one.

Note on the doc itself: this section and the one above are text sub-sections of this Finding,
not objects, so they cannot be referenced with `[[#...]]` — they are named in prose instead.

## The MCP ambiguity refusal is a decision, not a defect [[qmd69_finding_mcp: Finding]]

The third symptom in the report — MCP `qmdc_validate_references` refusing a container of
sibling workspaces with `ambiguous` — is the **designed** behaviour of QMD-63, which is
`done` and ships a test asserting exactly that envelope. Treating it as a bug would
silently revert a shipped decision, so it must not be folded into this fix.

- category: parser
- related_to: [[#qmd69_cross_ws_refs]], [[#qmd63]]
- affected_files: [qmdc-rs/src/core/index_seam.rs, tests/mcp/qmd63-ambiguous/mcp-expected.json]
- affected_functions: [resolve_root_bidirectional, resolve_root]
- solution: Do NOT change it under this task. Keep goal B2 out of scope until the operator decides what a composed root means for MCP; that decision belongs with issue #10 (explicit --with composition).

### Evidence [[qmd69_finding_mcp_detail: text]]

- about: [[#qmd69_finding_mcp]]

QMD-63 added `resolve_root_bidirectional` with self-check, then down-scan, then up-walk,
and defined `>1 workspace below the path` as `ErrorCode::Ambiguous` carrying the candidate
paths, deliberately, as recorded in its own result document. Its test
`tests/mcp/qmd63-ambiguous` pins `{"code": "ambiguous", "candidates": "__array_nonempty__"}`
for a container with two sibling workspaces.

A regression test asserting the opposite would contradict a passing test rather than
reproduce a bug, so none was written for this symptom. This is why goal B2 in the task is
now marked out of scope pending the operator's answer to open question 4.

## Mandatory regression tests (data-driven, failing as designed) [[qmd69_finding_tests: Finding]]

Five fixtures carrying ten data-driven cases were added during triage per the Bug triage
exception, one per surface the issue names: the CLI validator, the composed-container graph,
the LSP, and MCP. Nine reproduce a defect and one is a guard. No existing fixture covered a
workspace-qualified reference in any form, a qualified reference with a colliding id, a
qualifier naming no workspace, an unqualified cross-workspace reference, or an elided
namespace, so none of these duplicate existing coverage. All are fixtures plus expected JSON
— no new test code in any language.

The set grew as the operator settled the open questions, and each answer was pinned before
moving on: case 004 pins question 1 (a cross-workspace reference must be qualified), the
references were rewritten to the Kind-less form for question 2, and
`cross-workspace-elision/001` plus case 006 pin question 5 (an empty middle segment elides the
namespace, and is ambiguous on multiple candidates).

- category: testing
- related_to: [[#qmd69_cross_ws_refs]]
- affected_files: [tests/cli/011-validate-cross-workspace, tests/workspace/workspace-qualified-reference, tests/workspace/cross-workspace-qualifier, tests/workspace/cross-workspace-elision, tests/lsp/microtests/diagnostics/033-workspace-qualified-ref]
- solution: Keep all five fixtures as the acceptance gate; the nine failing cases turn green only when every qualifier is parsed and enforced, and the guard holds the form that already works.

### Every failing case must fail for its own reason [[qmd69_finding_tests_determinism: text]]

- about: [[#qmd69_finding_tests]]

A case that fails or passes because of arbitrary row ordering pins nothing, and the corpus
already contains that trap: the unfiltered `SELECT ... WHERE __id = ? LIMIT 1` has no
`ORDER BY`, so when a target id exists in two sibling workspaces the winner differs per
implementation. An earlier draft of the elision case relied on it and passed in Python and
Rust by luck while failing in TypeScript.

The fix was to stop involving that lookup at all. Edge resolution begins from the SOURCE
object's own workspace and namespace, so a same-named object in the CONSUMER workspace is
matched on the first try, before any qualifier is consulted — deterministically, in all three
implementations. `cross-workspace-elision` is built that way: the consumer declares its own
root-level `gateway`, so today every implementation binds the local object and the case fails
everywhere for one stated reason. It also strengthens the assertion, because the case now pins
two things at once — the qualifier must outrank a same-named local object, and the elided
namespace must still find the target.

Cost, and why the fixture is separate: that same shape detonates the Python crash
([[#qmd69_finding_py_crash]]), and a crash fails every case in its fixture. Inside
`cross-workspace-qualifier` it took down all six, guard 003 included — a guard that cannot
pass guards nothing. Splitting the shadow into its own container confines the crash to the one
case that is meant to fail anyway.

### The fixtures [[qmd69_finding_tests_fixtures: text]]

- about: [[#qmd69_finding_tests]]

`tests/cli/011-validate-cross-workspace` is the reported scenario verbatim: a non-workspace
container holding `project/` and `repo_a/`, validated with `qmdc workspace validate .`
expecting `[]`. It carries all three valid cross-workspace forms — the fully qualified
`[[#cli_repo_a:services:payments_api]]`, the elided-to-root `[[#cli_repo_a::billing]]`, and
the elided-to-namespace `[[#cli_repo_a::payments_api]]` — and runs in all three languages
through the CLI conformance harness. This is the only case that reproduces the issue exactly
as filed, and the elided-to-namespace reference is what makes it discriminate between the two
readings of `::` (question 5).

`tests/workspace/workspace-qualified-reference` is a single workspace, picked up by the
existing five-aspect `_expected.json` harness so it runs in all three languages. One object
holds the same target written three ways; the bare and `namespace:id` forms already resolve,
and the `workspace:namespace:id` one is the only difference. It isolates the grammar cause
with no composed container involved, it pins the py/ts/rs parity split, and it is the case
that crashes the Python CLI outright ([[#qmd69_finding_py_crash]]).

`tests/workspace/cross-workspace-qualifier` is three sibling workspaces under a
non-workspace container: `qmd69_repo_a` and `qmd69_repo_b` each define
`services:payments_api`, `repo_b` additionally holds `ledger` at its root AND in `services`,
and `qmd69_project` references them six ways. It is driven by the SQL harness, which loads
through `parse_all_workspaces` — the composed-container path the reported issue exercises.
Its five cases cover: the qualifier binding the named workspace (001), an unknown qualifier
(002), the root-namespace target as a GUARD that passes today (003), two unqualified
cross-workspace references that must not resolve (004), and an elided namespace with two
candidates (006).

One asymmetry is deliberate: `repo_b` carries the ambiguous `ledger` pair while `repo_a`
stays clean, so case 003's guard and case 004's `bare_unique: billing` keep their meaning —
`billing` must remain held by exactly one workspace for 004 to be the sharp test of
question 1.

`tests/workspace/cross-workspace-elision` is a second, smaller container — one consumer
workspace and one provider — carrying the positive half of the elision rule. It is separate
for a measured reason, recorded in the fixture's own SQL comment: the case needs a
source-workspace shadow to fail deterministically, and that shape detonates the Python crash
([[#qmd69_finding_py_crash]]), which fails EVERY case in whatever fixture it lives in. Keeping
it out of `cross-workspace-qualifier` is what lets the guard 003 stay green there and go on
guarding.

The diagnostic half of case 006 — an `ambiguous_reference` rather than silence — is not
asserted here. The workspace-conformance harness takes one `workspace_id` per fixture and so
never runs over a multi-workspace container, which is why `multi-workspace-collision` ships
SQL-only too. Case 006 therefore pins the graph half (no edge); the diagnostic half of the
same rule is reachable from a single workspace and is what the "validate and query disagree on
the bare form too" section below records as already broken for the bare form.

`tests/lsp/microtests/diagnostics/033-workspace-qualified-ref` carries two expected
envelopes over one workspace — `expected.json` for the LSP `textDocument/diagnostic`
response and `mcp-expected.json` for `qmdc_validate_references` — so a single fixture pins
both surfaces. Both are rs-only, because the LSP and MCP servers exist only in the Rust
implementation.

### Expected failures before implementation [[qmd69_finding_tests_failures: text]]

- about: [[#qmd69_finding_tests]]

Recorded per the Bug triage exception. Every failure below is caused by a newly added
regression test and matches the documented bug; no pre-existing or unrelated test fails.
Columns re-measured after the references were rewritten to the canonical Kind-less form.

| case | surface | py | ts | rs | reason for the failure |
| --- | --- | --- | --- | --- | --- |
| `cli/011-validate-cross-workspace` | CLI validate | **fail** | **fail** | **fail** | the reported scenario: `workspace validate .` on the container reports `broken_link` for all three valid cross-workspace forms instead of `[]` |
| `workspace-qualified-reference/errors` | parser | **CRASH** | **fail** | **fail** | all three read the workspace slot as a namespace and emit a false `broken_link`; py does not report it but aborts with `'tuple' object has no attribute 'get'` ([[#qmd69_finding_py_crash]]) |
| `cross-workspace-qualifier/001-qualifier-binds-named-workspace` | graph/SQL | **fail** | pass | **fail** | the qualifier is discarded, so the edge binds to whichever workspace the unfiltered lookup returns first; ts happens to return repo_a and passes by luck |
| `cross-workspace-qualifier/002-unknown-qualifier-no-fallback` | graph/SQL | **fail** | **fail** | **fail** | a qualifier naming no existing workspace still resolves via the bare-id fallback instead of reporting not-found |
| `cross-workspace-qualifier/004-bare-cross-workspace-rejected` | graph/SQL | **fail** | **fail** | **fail** | pins question 1: an UNQUALIFIED cross-workspace reference must not resolve, not even when exactly one workspace holds the id; today all three bind both bare references |
| `cross-workspace-elision/001-elided-namespace-matches-any` | graph/SQL | **CRASH** | **fail** | **fail** | pins question 5: `workspace::id` must reach a target inside a NAMESPACE of that workspace, and must outrank a same-named LOCAL object. rs/ts bind the local shadow; py hits the crash |
| `cross-workspace-qualifier/006-elided-namespace-ambiguous` | graph/SQL | **fail** | **fail** | **fail** | pins the other half of question 5: with root `ledger` and `services:ledger` both in repo_b the elided form is ambiguous and must bind nothing; today all three bind the root object silently |
| `diagnostics/033-workspace-qualified-ref` (`expected.json`) | LSP | — | — | **fail** | the editor shows a false `QMDC001` on the canonical qualified form |
| `diagnostics/033-workspace-qualified-ref` (`mcp-expected.json`) | MCP | — | — | **fail** | `qmdc_validate_references` returns one false `QMDC001` where the expected envelope is `{"diagnostics": [], "count": 0}` |

One further case in the same fixture set is a GUARD and passes in all three today —
`cross-workspace-qualifier/003-root-namespace-target`. Together with the elision case and 006
it pins the elision rule across all three outcomes: one candidate in the root (003), one
candidate in a namespace (elision/001), two candidates (006).

The uneven py/ts/rs columns are themselves a finding, not test flakiness: a case that
passes in one implementation and fails in another means the corpus was not pinning this
behaviour at all. After implementation all three columns must read pass.

The Python CRASH row costs five reported failures rather than one, and that spread is itself
the finding: all four `_expected.json` aspects of the fixture fail (not just `errors`,
because the exception happens inside `parse_workspace` before any aspect can be checked), and
`test_parse_all_workspaces` fails too. The last one matters most — it parses the WHOLE
`tests/workspace/` tree, so a single workspace-qualified reference anywhere in a container
aborts the parse of every workspace in it.

On the originally-reported four-segment form the three implementations did not even agree on
which id they were looking for: given `[[#cli_repo_a:services:Service:payments_api]]`, rs
reported `Object 'Service:payments_api' not found` (keeping the tail segments glued) while ts
reported `Object 'cli_repo_a' not found` (keeping the head). Both messages were wrong about
what was missing, and wrong differently. On the canonical form they converge on
`Object 'payments_api' not found`, which is at least an honest message — the disagreement
moved rather than vanished, and it is still visible in where a bare reference lands: on
`bare_duplicated` in case 004, py and rs bind `qmd69_repo_b` while ts binds `qmd69_repo_a`.

### LSP and MCP coverage [[qmd69_finding_tests_mcp: text]]

- about: [[#qmd69_finding_tests]]

The LSP and MCP share one fixture and one workspace, and neither expected envelope touches
the composed-container question. `tests/lsp/microtests/diagnostics/033-workspace-qualified-ref`
is a SINGLE workspace, so `resolve_root` resolves it via the self-check and the QMD-63
ambiguity path is never entered; the defect is then visible purely as reference resolution.
Both surfaces return one `QMDC001 — Object 'Service:payments_api' not found` where the
expected result is no diagnostics at all.

One fixture is enough for both because MCP validation shares `core::ops::validate` with the
LSP after QMD-68, and the microtest harness reads the two envelopes from the same directory:
`expected.json` is driven by `tests/lsp` and `mcp-expected.json` by `tests/mcp`, so each
surface fails independently and is reported as its own case. An earlier standalone MCP
fixture was removed once this one covered both.

This is the distinction that matters for [[#qmd69_finding_mcp]]: what must NOT be tested is
MCP accepting a multi-workspace container, because QMD-63 deliberately refuses that. What
must be tested is MCP resolving a qualified reference, which is the real defect and has
nothing to do with containers. Both `tests/mcp/qmd63-ambiguous` and
`tests/mcp/qmd63-down-single` stay green with the new fixture present, confirming the two
concerns are independent.

Both cases are rs-only because the LSP and MCP servers exist only in the Rust
implementation — the same reason the unified report shows `lsp` and `mcp` as `rs-only` rows.
The CLI path is covered separately and in all three languages by
`tests/cli/011-validate-cross-workspace` and
`tests/workspace/workspace-qualified-reference`.

### Full suite record [[qmd69_finding_tests_suite: text]]

- about: [[#qmd69_finding_tests]]

Measured per suite on this tree, reading the JUnit reports rather than the console. Final
figures:

| suite | cases | failed | which |
| --- | --- | --- | --- |
| py | 862 | 17 | cli/011, sql qualifier 001+002+004+006, sql elision/001, plus 5 from the Python crash on the single-workspace fixture, plus 6 belonging to QMD-70 |
| ts | 851 | 12 | cli/011, workspace-qualified-reference/errors, sql qualifier 002+004+006, sql elision/001, plus 6 belonging to QMD-70 |
| rs | 1005 | 21 | cli/011, workspace-qualified-reference/errors, sql qualifier 001+002+004+006, sql elision/001, lsp 033, mcp 033, plus 12 belonging to QMD-70 |

Every failure is one of the nine reproduction cases of this task or one of QMD-70's four
fixtures; nothing else fails. Per-file breakdown for the rs side of THIS task:
`rs-cli.xml` 11/1, `rs-workspace.xml` 150/1, `rs-sql.xml` 65/5, `rs-lsp.xml` 125/1,
`rs-mcp-fixtures.xml` 35/1.

Two harness facts make a naive `make test` run under-report, and both must be worked around
while this Bug is open:

1. `cargo nextest` cancels the remaining test binaries after the first failure, so
   `test-reports/rs.xml` is never written and whole rs suites vanish from the aggregate. Use
   `cargo nextest run --no-fail-fast`: **157 test binaries, 151 passed, 6 failed**
   (`cli_conformance`, `lsp`, `mcp`, `sql`, `workspace_conformance`, `workspace_unit`).
2. `make py-test ts-test` stops at the first failing target and `npm test` is an `&&` chain,
   so a py failure hides the whole ts side and a ts CLI failure hides `test-workspace.ts`
   and `test-sql.ts`. Run the suites independently, or with `make -k`.

Because of the above, the aggregate case total shifts between runs (3126 and 2977 both
seen). That number is not evidence of anything; the set of FAILING cases is stable, only the
count of reported green cases moves. Use the per-suite figures above.

One rs failure exists outside the fixture harnesses and is the same defect seen through a
different assertion: the impl-specific unit test
`qmdc-rs/tests/workspace_unit.rs::test_workspace_validation_no_errors` walks every fixture
whose `_expected.json` declares no errors and asserts the parse produced none. It fails on
`workspace-qualified-reference` with the identical
`broken_link — Object 'Service:payments_api' not found`, so it is reproduction evidence
rather than a separate problem.

## Blast radius of removing the fallback [[qmd69_finding_blast_radius: Finding]]

Measured before the operator answered question 1, so the decision was informed rather than
guessed. **No existing fixture relies on an unqualified cross-workspace reference**, so
removing the bare-id fallback is breaking in principle but not in practice on this corpus.
The fallback is meanwhile demonstrably harmful: it converts a correct diagnostic into a
confident wrong edge as soon as a sibling workspace exists.

- category: parser
- related_to: [[#qmd69_cross_ws_refs]]
- affected_files: [qmdc-rs/src/db/mod.rs, qmdc-py/qmdc/db.py, qmdc-ts/src/db.ts, tests/workspace/errors/ambiguous_refs.qmd.md]
- solution: Remove the third-try fallback as decided; no existing expected-JSON needs migrating, and the one existing cross-workspace reference in the corpus is already qualified.

### What actually depends on the fallback [[qmd69_finding_blast_radius_measure: text]]

- about: [[#qmd69_finding_blast_radius]]

Method: every fixture directory under `tests/workspace/` was parsed ALONE — the same scoping
the conformance and SQL harnesses use, since `load_workspace_objects` composes only the
fixture handed to it — and queried for edges whose source and target workspaces differ. Those
are exactly the edges that go through the third try today, because the qualifier is discarded
before resolution.

Only two fixtures produce any:

| fixture | reference | qualified? | survives the decision |
| --- | --- | --- | --- |
| `multi-workspace-collision` | `[[#ws2::target_obj]]` | yes, double-colon form | yes |
| `cross-workspace-qualifier` | the three QMD-69 cases | yes | yes (that is their point) |

So the only pre-existing cross-workspace reference in the whole corpus already names its
workspace, and `multi-workspace-collision/tests/001-cross-workspace-edges` keeps passing.
No `_expected.json` and no SQL expectation encodes a resolution that the decision takes away.

Caveat on measurement: querying the WHOLE `tests/workspace/` tree as one container instead
shows ~20 cross-workspace edges, but those are cross-FIXTURE artifacts of composing unrelated
fixtures into one graph and are asserted by nothing. Do not read that number as blast radius.

### Evidence that the fallback hides real errors [[qmd69_finding_blast_radius_harm: text]]

- about: [[#qmd69_finding_blast_radius]]

The clearest case is a pre-existing shipped fixture, not one written for this task.
`tests/workspace/errors/ambiguous_refs.qmd.md` holds two references to `users`, and the
fixture's own prose declares one ambiguous and one unambiguous.

Parsed alone, both behave as the fixture intends: zero edges, and the bare reference is
reported as `broken_link — Object 'users' not found`, with a did-you-mean suggestion naming
the hierarchical object `alpha_objects.users` in namespace `alpha`.

Placed in a container beside sibling workspaces, both silently acquire an edge to
`test_ws:storage:users` — an object in an unrelated workspace belonging to a different
fixture. The diagnostic disappears and is replaced by a wrong edge.

Note which reference is affected: `[[#alpha:users]]` is NAMESPACE-qualified and still leaks,
because no object in `error_cases` has the bare `__id` `users` (the real object is the
hierarchical `alpha_objects.users`), so the second try misses and the third try matches on
bare id across workspaces. The defect therefore is not limited to bare `[[#id]]` references —
any reference whose qualifiers fail to match locally is liable to be re-pointed into a
neighbouring workspace.

## The Kind segment is decoration, and it silences real ambiguity [[qmd69_finding_kind: Finding]]

Measured on the operator's challenge ("Kind has to go — it isn't real, is it?"). It is not
real: `Kind` never participates in identity, is ignored by the graph, and in the
three-segment form it suppresses a diagnostic that would otherwise fire. Removing it from the
reference grammar is therefore a simplification with no expressive loss — and it collapses
the canonical-form question, because without Kind three colon-separated segments can only
mean `workspace:namespace:id`.

- category: parser
- related_to: [[#qmd69_cross_ws_refs]]
- affected_files: [qmdc-rs/src/core/reference_scan.rs, qmdc-rs/src/parser_modules/references.rs, qmdc-py/qmdc/parser.py, qmdc-ts/src/parser.ts, tests/parser/039-reference-with-kind.qmd.md]
- solution: Drop Kind from the reference grammar in all three parsers; the canonical form becomes workspace:namespace:id with an optional .field suffix, identical to __global_id.

### Identity never includes Kind [[qmd69_finding_kind_identity: text]]

- about: [[#qmd69_finding_kind]]

`compute_global_id` builds `__global_id` from `workspace`, `namespace` and `id` only. Kind is
not part of an object's identity, so a Kind segment in a reference can never be needed to
name a target — at most it is a redundant assertion about the target it already named.

Stronger: ids are unique within a namespace, enforced by `duplicate_id`. For a Kind to
disambiguate two objects it would have to distinguish two same-id objects in one namespace —
which is already an error. **Kind can only ever be load-bearing in an already-invalid
workspace.**

### Probe 1: with a namespace, Kind is ignored AND silences ambiguity [[qmd69_finding_kind_probe1: text]]

- about: [[#qmd69_finding_kind]]

One workspace, one namespace `storage`, two objects both with id `users` differing only by
Kind (`Table` and `Entity`) — which is itself a `duplicate_id`, per the paragraph above. Four
references from the same consumer:

| reference | diagnostic | edge actually built |
| --- | --- | --- |
| `[[#storage:users]]` | `ambiguous_reference` ✔ | — |
| `[[#storage:Table:users]]` | **none** | → `Entity` ✘ |
| `[[#storage:Entity:users]]` | **none** | → `Entity` |
| `[[#storage:NoSuchKind:users]]` | **none** | → `Entity` ✘ |

Two separate defects visible at once. The graph ignores the Kind entirely — the reference
that explicitly asks for the `Table` gets an edge to the `Entity`. And adding any Kind
segment, including one naming a Kind that does not exist, turns a correct
`ambiguous_reference` into silence: `reference_scan` short-circuits with
`is_ambiguous = false` whenever both a kind and a namespace are present, while the candidate
filter returns early on the namespace match and never looks at the kind at all.

### Probe 2: without a namespace, validator and graph disagree [[qmd69_finding_kind_probe2: text]]

- about: [[#qmd69_finding_kind]]

The two-segment `Kind:id` form is the one place the validator does honour Kind — and there
the graph still does not, so the two surfaces disagree exactly as in the reported issue. One
workspace, `alpha:users` a `Table` and `beta:users` an `Entity`:

| reference | validator | edge actually built |
| --- | --- | --- |
| `[[#users]]` | `ambiguous_reference` ✔ | → `alpha:users` |
| `[[#Table:users]]` | clean | → `alpha:users` ✔ |
| `[[#Entity:users]]` | **clean** | → `alpha:users` ✘ |

So `[[#Entity:users]]` passes validation and then builds an edge to the `Table` in the other
namespace. This is the same validate-versus-query split as [[#qmd69_finding_fallback]], on a
second axis.

### What removing Kind costs [[qmd69_finding_kind_cost: text]]

- about: [[#qmd69_finding_kind]]

Small, and all of it visible in the corpus rather than guessed:

- The uppercase-first-segment heuristic disappears. It exists only to tell `Kind:id` from
  `namespace:id`, and it lives in TWO places per implementation — `classify_reference`
  (which sets the `type` field in `__references`) and `parse_reference_target` (resolution).
  The first applies it to dots as well as colons, so `[[#Foo.bar]]` is currently typed
  `kind` too.
- The `ref_type` value `"kind"` vanishes from parse output. Asserted by exactly two
  fixtures: `tests/parser/039-reference-with-kind.expected.full.json` and
  `tests/parser/066-paragraph-references.expected.full.json`. Both need updating to
  `namespace`, in all three languages.
- `[[#Kind:id]]` written by a user becomes `namespace:id` and will usually report
  not-found. This is the one genuinely breaking consequence. No fixture other than 039
  exercises it, and `namespace:Kind:id` appears in no fixture at all — only in the guide's
  own prose, which goal D1 already has to rewrite.

## The Python validator crashes on a workspace-qualified reference [[qmd69_finding_py_crash: Finding]]

Found while re-running the fixtures after Kind was dropped. This is a third failure mode,
and the most severe: not a wrong diagnostic but an unhandled exception in a shipped code
path, reachable from the public CLI.

- category: parser
- related_to: [[#qmd69_cross_ws_refs]]
- affected_files: [qmdc-py/qmdc/workspace.py]
- solution: The hint block at workspace.py:1066-1078 indexes tuples as if they were dicts. Read the tuple positionally — objects_by_id holds (file, kind, ns_id, line), so the namespace is element 2 and the id is ref_id itself.

### Reproduction and cause [[qmd69_finding_py_crash_detail: text]]

- about: [[#qmd69_finding_py_crash]]

`qmdc-py workspace validate tests/workspace/workspace-qualified-reference` exits with
`Error: 'tuple' object has no attribute 'get'`. No diagnostics are produced at all — the
whole validation run is lost, not just the one reference.

`objects_by_id` is declared `dict[str, list[tuple[str, str, str, int]]]` and populated with
`(obj_file, obj_kind, ns_id, obj_line)` tuples. The "did you mean" hint block for a broken
link then calls `.get("__namespace")` and `.get("__id")` on those elements.

The branch needs three conditions at once, which is why it survived until now: the reference
must be a broken link, the `by_local_id` hint branch must have produced nothing, and
`objects_by_id` must nevertheless contain the bare id. A workspace-qualified reference hits
all three — the id exists, just not in the namespace the reference names. The reported
container case does NOT crash, because there the `by_local_id` branch fills the hint first;
that is why the issue reported a wrong answer rather than a crash.

## A separate bug was split off: QMD-70 [[qmd69_finding_spinoff: Finding]]

Triage surfaced a defect that is NOT part of this task and was filed on its own as **QMD-70**,
on its own branch `qmd-70` off `public/main`. It is recorded here because it was found while
authoring this task's own documents and because it shaped how they are written. There is no
`[[#...]]` reference to it on purpose: the two tasks are on separate branches, so a reference
would be a broken link on either one until both land.

- category: parser
- related_to: [[#qmd69_cross_ws_refs]]
- affected_files: [docs/tracking/planned/QMD-70]
- solution: Nothing to do here. QMD-70 carries its own goals, findings and four failing fixtures on branch qmd-70; the operator has parked it, and it does not block any goal of QMD-69.

### What it is and why it is out of scope [[qmd69_finding_spinoff_detail: text]]

- about: [[#qmd69_finding_spinoff]]

A Markdown table written as ordinary content inside an object-array ELEMENT is captured by
the parent array and its rows become sibling elements. A comparison table inside goal A2 of
this task's own file destroyed that goal and replaced it with four anonymous ones labelled
after the table's first column.

It shares nothing with the qualifier defect but the file it was found in: different code
path (the table branch of the block walker, not reference resolution), different symptom
(object loss, not mis-binding), different fix. Keeping it here would have widened a
parser-resolution task into a parser-structure one, so it was filed separately and the
operator parked it.

Two consequences visible in THIS task's documents: every comparison table in the task file
is written as prose instead of a table, and the tables that remain live inside `text`
sub-sections of Findings, which is the one position where the corpus already proves a table
survives (`tests/parser/065-text-table-in-array`). A tracking document that puts a table
inside a Goal is currently corrupting itself, and in Rust it loses the Goal.

## B1 and B2 are one change, and question 4 decides both [[qmd69_finding_container: Finding]]

Found during implementation, not triage, and it revises the recommendation this task carried.
Triage recommended dropping goal B2 (composed root on MCP/LSP) and letting issue #10 own it,
while keeping B1 (CLI validation of a container) inside QMD-69. That split does not survive
contact with the code: B1 cannot pass without the same composed index B2 asks for.

- category: parser
- related_to: [[#qmd69_cross_ws_refs]]
- affected_files: [qmdc-rs/src/workspace.rs, qmdc-rs/src/core/index_seam.rs]
- affected_functions: [parse_all_workspaces, resolve_root]
- solution: Operator decision needed (question 4). Either QMD-69 owns container composition for all surfaces (B1 + B2 together), or both are deferred to issue #10 and QMD-69 ships qualifier semantics WITHIN a workspace only — in which case `tests/cli/011-validate-cross-workspace` and the composed-container SQL cases move with them.

### The measurement [[qmd69_finding_container_detail: text]]

- about: [[#qmd69_finding_container]]

With the qualifier filter in place, a qualified reference inside ONE workspace resolves
correctly — `tests/workspace/workspace-qualified-reference` validates clean, and the LSP and
MCP microtests both went green. The container case did not: it still reports
`broken_link — Object 'payments_api' not found` for all three of its cross-workspace forms.

The cause is upstream of resolution. `parse_all_workspaces` (`qmdc-rs/src/workspace.rs:796`)
finds the sibling workspace directories and then loops `for ws_dir in &workspace_dirs`,
parsing and validating each one on its own and concatenating the errors. The reference
resolver is handed one workspace's objects at a time, so a cross-workspace target is not
merely mis-filtered — it is absent from the index. No change to the filter can fix that.

This is the same thing QMD-63 decided against on the MCP surface, where `resolve_root`
returns `ambiguous` for a container of several workspaces rather than composing them. So the
three surfaces are consistent today: none of them composes a container. Composing it is a
single architectural change that lands on all three at once.

### Why this matters for the SOP gate [[qmd69_finding_container_gate: text]]

- about: [[#qmd69_finding_container]]

The workflow SOP requires every Goal to reach `done: true` before a task may move to
`done_review` — partial completion is explicitly not done. With B1 blocked and B2 held out of
scope, QMD-69 cannot reach that gate as currently scoped, regardless of how much of the
qualifier work lands. The scope has to be settled first:

- **Own it here:** B1 and B2 both stay, QMD-69 grows a container-composition change, and it
  reverses a shipped QMD-63 decision — which needs its own justification and probably its own
  test review, since `tests/mcp/qmd63-ambiguous` currently pins the opposite.
- **Defer both:** B1 and B2 move to issue #10 along with the container fixtures, and QMD-69
  ships the qualifier grammar and its enforcement within a workspace — A1, A2, A3, B3, C1, D1.
  Six of ten test cases then belong to QMD-69 and four move with the deferred goals.

The second option is what the measurement recommends: it keeps QMD-69 to one coherent defect
and leaves the architectural question where it was already being discussed.

## What implementation found that triage did not [[qmd69_finding_impl: Finding]]

Three things surfaced only while writing the fix, and each one changes what the task claims.
Recorded here because two of them were wrong in the triage documents and one is a defect this
task nearly shipped.

- category: parser
- related_to: [[#qmd69_cross_ws_refs]]
- affected_files: [qmdc-rs/src/db/mod.rs, qmdc-py/qmdc/db.py, qmdc-ts/src/db.ts, qmdc-mkdocs/qmdc_mkdocs/references.py, tests/workspace/container-root-single-workspace]
- solution: All three are fixed; the guard test and the mkdocs port are part of the change. Nothing outstanding.

### QMD-63 was not reversed, and did not need to be [[qmd69_finding_impl_qmd63: text]]

- about: [[#qmd69_finding_impl]]

Goal B2 was taken into scope expecting to overturn the shipped QMD-63 decision. It did not
come to that, and the distinction is worth keeping straight because QMD-63's reasoning is
still sound.

QMD-63 answered "which SINGLE workspace does an MCP call resolve to when handed a container?"
and decided: do not auto-pick one of several, return `ambiguous` with the candidates. That is
about ROOT SELECTION. `resolve_root` is untouched by QMD-69 and `tests/mcp/qmd63-ambiguous`
stays green.

QMD-69 needed a different question answered: "does reference validation see the whole
container?" And that is settled where the container is already assembled. QMD-63's own findings
noted in passing that the CLI's "parse/union contract differs from MCP's single-root" — the
union was already there. `parse_all_workspaces` combined the sibling workspaces' objects into
one result and only ran reference validation BEFORE the union, per workspace. So no
container-composition architecture was needed: drop the stale per-workspace reference findings,
run the shared engine once over the composed set, and keep structural findings per-workspace
because identity is workspace-scoped (QMD-67).

### Removing the fallback broke references to a workspace ROOT object [[qmd69_finding_impl_root: text]]

- about: [[#qmd69_finding_impl]]

A real regression, caught by reading the graph rather than by a failing test — and that is the
point worth recording.

A `__Workspace` object carries no own `__workspace` field, because it IS the workspace; its
`__global_id` is `::docs_ws`. The same-workspace lookup filters on `__workspace = <ws>`, which
that object cannot satisfy, so once the unqualified "any workspace" fallback was gone,
`about: [[#docs_ws]]` stopped producing an edge.

Nothing pinned it. `tests/workspace/container-root-single-workspace` carries exactly that
reference but had no `tests/` directory at all, so the whole suite stayed green while the edge
silently disappeared. Fixed by admitting an empty-workspace object only when the looked-up id
IS the workspace's own name — which keeps the root object reachable without letting a bare id
reach a sibling workspace's root, so the operator's decision 1 still holds. Pinned now by
`container-root-single-workspace/001-reference-to-workspace-root`.

### The grammar had a fourth consumer outside the three parsers [[qmd69_finding_impl_mkdocs: text]]

- about: [[#qmd69_finding_impl]]

The blast-radius measurement looked at fixtures and at the three parsers. It missed that
`qmdc-mkdocs` reaches into `QmdcDatabase._resolve_target_global_id` — a private method — and
additionally reimplements the old grammar inline, including its own `parts[0][0].isupper()`
Kind branch and a `namespace:Kind:id` query.

So "the uppercase heuristic lives in two places per implementation" was an undercount: there
was a third copy in a sibling package. Its 24 failing tests were the signal. The port replaces
the inline grammar with the new call shape, and six of its tests asserted the Kind forms and
were rewritten to the workspace-qualified ones.

Lesson for the next grammar change: `grep` for the private resolver name across ALL packages,
not just for the syntax inside `tests/`.

## Three resolution paths bypassed the qualifier filter [[qmd69_finding_bypass: Finding]]

Found by the operator asking whether hierarchical dotted ids still worked. They did — but the
corpus contained **not one reference carrying a qualifier and a dotted id together, in any
form**, so nothing was pinning it. Building that matrix out exposed three paths that resolve a
reference AFTER the main candidate filter misses, and none of them consulted the qualifiers.

- category: parser
- related_to: [[#qmd69_cross_ws_refs]]
- affected_files: [qmdc-rs/src/core/reference_scan.rs, qmdc-rs/src/db/mod.rs, qmdc-py/qmdc/workspace.py, qmdc-py/qmdc/db.py, qmdc-ts/src/workspace.ts, qmdc-ts/src/db.ts]
- affected_functions: [qualifiers_match, workspace_matches, resolve_target_global_id]
- solution: All three fixed in all three implementations, behind one shared predicate pair so they cannot drift apart again. Pinned by the id-shape matrix.

### The three paths [[qmd69_finding_bypass_paths: text]]

- about: [[#qmd69_finding_bypass]]

**1. The `__local_id` fallback in the validator.** Its index carried no workspace column at
all, so a workspace qualifier was not merely mishandled — there was nothing to compare against.
It filtered on the namespace only, which produced a precise and damning asymmetry: with
`postgres` held solely by the provider workspace, `[[#provider:no_such_ns:postgres]]` was
reported while `[[#no_such_ws:arch:postgres]]` and `[[#third_ws:arch:postgres]]` were accepted
silently. The second of those names a workspace that EXISTS and does not hold the object, which
is the case a namespace check cannot catch — the fixture gives the third workspace a namespace
called `arch` on purpose to prove it.

**2. The field-reference escape in the validator.** When the full id misses, the validator
splits on the last dot and accepts the reference if the prefix is an object with that field.
It did so without looking at the qualifiers, so a mis-qualified reference to a hierarchical id
was swallowed: `[[#no_such_ws:arch:system.data.postgres]]` was read as "field `postgres` of
object `system.data`" and never reported. The same applied to the `ambiguous_field_reference`
(QMDC009) check beside it.

**3. The `__local_id` fallback in the edge builder.** Hard-wired to the SOURCE object's
workspace and namespace, so naming the workspace that actually holds the target made the
reference stop resolving. Spelling out more made it worse, which is the shape of the defect.

Paths 1 and 3 together produced the validate-versus-query split this task exists to remove, on
the one combination it had not covered: the validator accepted a qualified leaf reference while
the graph built no edge for it.

### Why it survived [[qmd69_finding_bypass_survival: text]]

- about: [[#qmd69_finding_bypass]]

Reaching these paths needs a reference whose full id does NOT exist — a short-form leaf or a
field path — AND a qualifier on it. The corpus pins the namespace half
(`local-id-cross-namespace-explicit` uses `[[#services:gateway]]`), but the workspace half was
unpinnable: before this task there was no workspace qualifier in the grammar. The holes were
not overlooked; they were not expressible.

Confirmed by the diff, not by argument: the `__local_id` block was untouched by the earlier
part of this change, so these are pre-existing defects that the new tests finally reached.

### The fix [[qmd69_finding_bypass_fix: text]]

- about: [[#qmd69_finding_bypass]]

Two shared predicates, so no path decides on its own again:

- `workspace_matches` — the workspace rule alone.
- `qualifiers_match` — workspace plus the namespace rule where `Some("")` is an elision.

The main candidate filter, the field-reference escape and the QMDC009 check all use
`qualifiers_match`. The `__local_id` fallback uses `workspace_matches` plus its OWN namespace
rule, and that difference is deliberate: there an unqualified reference is scoped to the
referring object's own namespace EXACTLY. Conflating the two is the mistake this fix made
first, and two shipped fixtures caught it —
`local-id-cross-namespace-no-fallback` (a bare `[[#config]]` at the workspace root must not
reach `gateway.config` in `services`) and `errors` (a root-level `[[#users]]` must stay a
broken link rather than becoming ambiguous). Both are documented at the call sites so the
distinction is not lost again.

Two supporting changes fell out of it. The field lookup index became a MULTIMAP: it mapped
`id -> first object seen`, which in a composed container silently inspected an object from the
wrong workspace. And the edge resolver was restructured into one flow — it used to return early
whenever a workspace was named, so the `__local_id` fallback was unreachable for exactly the
references that needed it.

### The id-shape matrix [[qmd69_finding_bypass_matrix: text]]

- about: [[#qmd69_finding_bypass]]

Five id shapes — flat, hierarchical dotted, the `__local_id` leaf of a hierarchical object, a
field path on a flat object, a field path on a hierarchical one — crossed with every qualifier
form, on every surface.

| fixture | surface | what it covers |
| --- | --- | --- |
| `tests/workspace/id-shapes-qualified/001` | graph, py+ts+rs | 20 intra-workspace combinations |
| `tests/workspace/id-shapes-qualified/002` | graph, py+ts+rs | 8 cross-workspace combinations |
| `tests/workspace/id-shapes-qualified/003` | graph, py+ts+rs | 15 negatives build no edge — GUARD, the graph was already right |
| `tests/cli/014-validate-id-shapes` | validator, py+ts+rs | all 15 negatives reported AND every valid form clean |
| `tests/cli/012-validate-qualified-hierarchical` | validator, py+ts+rs | GUARD: valid qualifier+dot combinations stay clean |
| `tests/cli/013-validate-qualified-local-id-unknown-workspace` | validator, py+ts+rs | unknown workspace on the leaf path |
| `tests/lsp/microtests/diagnostics/034-qualified-local-id-and-field-path` | LSP + MCP | the leaf and field-path forms on the rs-only surfaces |
| `tests/workspace/cross-workspace-hierarchical-ids/001-004` | graph, py+ts+rs | dotted id and field path with a qualifier; three guards plus one reproduction |

The field-path case is the one most at risk from a careless fix: in
`[[#ws:arch:system.data.postgres.engine]]` only the LAST dot separates the field, so splitting
on the first dot, or splitting before the qualifiers are removed, yields a WRONG edge rather
than no edge — which passes unnoticed unless asserted. `003-qualified-hierarchical-field-path`
asserts the target id and the target field separately for exactly that reason.

Whole-suite result after the fix: **3325 cases, 0 failures**, and `make validate-compare`
reports 0 errors from each of the three parsers.

## Open questions for the operator [[qmd69_finding_questions: Finding]]

Five decisions were needed before implementation. **Questions 1, 2, 3 and 5 are now
answered** (2026-09-23); only question 4 is still with the operator, and it is a scoping call
rather than a blocker.

- category: parser
- related_to: [[#qmd69_cross_ws_refs]]
- solution: Questions 1, 2, 3 and 5 answered — a cross-workspace reference must be qualified, the canonical form is #workspace:namespace:id.field with no Kind segment, self-qualification is legal, and an empty middle segment elides the namespace (ambiguous on multiple candidates). Goals A1-A3, B1, B3, C1 and D1 are ready to implement; goal B2 stays out of scope until question 4 is settled.

### The questions [[qmd69_finding_questions_detail: text]]

- about: [[#qmd69_finding_questions]]

1. **Is an unqualified cross-workspace reference still legal?** — **ANSWERED (operator,
   2026-09-23): no.** A cross-workspace reference must name its workspace. The bare-id
   fallback is removed outright rather than narrowed to the unique-id case, so a reference
   that names no workspace resolves only inside its own workspace and is otherwise a broken
   link. The fix is therefore breaking in principle, but measurably not in practice — see
   [[#qmd69_finding_blast_radius]]. Recorded as the decision in [[#qmd69_goal_a3]] and pinned
   by `cross-workspace-qualifier/004-bare-cross-workspace-rejected`.
2. **What is the canonical qualified form?** — **ANSWERED (operator, 2026-09-23):**
   `#workspace:namespace:id.field`, single colons, no Kind segment. The question turned out
   to rest on a false premise: single versus double colon was never a choice between two
   conventions. Once Kind is dropped ([[#qmd69_finding_kind]]) three segments can only mean
   `workspace:namespace:id`, and `[[#ws2::target_obj]]` is simply that form with an empty
   middle segment — an object in a workspace's root namespace. So the shipped
   `multi-workspace-collision` fixture and the guide's "full format" describe the SAME
   grammar; only the Kind segment was spurious. The canonical form is therefore literally
   `__global_id` plus an optional `.field` suffix, which means a reference and an identity are
   written the same way.
3. **Is a self-qualified reference valid?** — **ANSWERED implicitly by question 2: yes.**
   Naming your own workspace is just writing the full identity of a target that happens to be
   local; nothing in the grammar makes it a special case, and it is how the smallest parsing
   test is written. `workspace-qualified-reference` keeps its expectation.
4. **Does composed-root acceptance belong to this task at all?** Goal B2 contradicts the
   shipped QMD-63 decision (see [[#qmd69_finding_mcp]]) and overlaps issue #10. Recommended:
   drop B2 from QMD-69 and let #10 own it.
5. **What exactly may be elided, and from which end?** — **ANSWERED (operator, 2026-09-23):
   the empty middle segment is an ELISION, and it is ambiguous when it finds more than one
   candidate.** A reference is a right-aligned suffix of `workspace:namespace:id`, and
   `workspace::id` means "this workspace, any namespace, this id" — so it reaches a target in
   the workspace root AND one inside a namespace. If the named workspace holds that id in two
   namespaces (say root `ledger` and `services:ledger`), the reference is ambiguous and binds
   nothing.

   The decisive argument is consistency rather than taste: this makes `workspace::id` behave
   exactly like a bare `[[#id]]` already behaves inside one workspace — `reference_scan`'s
   "second try" is any-namespace and reports `ambiguous_reference` on multiple matches. So the
   rule is not new semantics, it is the existing semantics finally reachable through an
   explicit workspace qualifier. Pinned by `cross-workspace-qualifier/003` (root target),
   `cross-workspace-elision/001` (namespaced target, with a local shadow so the failure is
   deterministic) and `cross-workspace-qualifier/006` (two candidates); recorded in
   [[#qmd69_goal_a2]].

   Measured while deciding: the elided form reaches NOTHING today. `[[#ws::only_here]]` where
   the target sits in `ws:services` reports `Object 'only_here' not found` with a did-you-mean
   pointing at the object it just refused to match.
