# QMD-68: Converge validation across CLI / LSP / MCP and de-duplicate the logic

## Unify diagnostics across surfaces and remove duplicated validation logic [[qmd68_converge: Feature]]

Validation/diagnostic behavior is exposed by three surfaces — CLI (`qmdc workspace
validate`), LSP (editor diagnostics), MCP (`qmdc_validate_references`, `qmdc://
diagnostics`) — and they do NOT share one validator. Reference resolution is already
consolidated in `core::resolve` + `core::ops::validate::collect_reference_issues`
(used by LSP and MCP), but the CLI/indexer path in `qmdc-rs/src/workspace.rs`
reimplements it inline, and `qmdc-py`/`qmdc-ts` are full parallel copies. Structural
diagnostics (cross-file `duplicate_id`, `broken_parent`, `workspace_in_wrong_file`,
and all parser `__ParsingError` kinds) are CLI-only, so the editor and MCP silently
miss them. This task closes the surface gaps AND consolidates the duplicated logic to
one source of truth so the surfaces can't drift again.

- status: done
- priority: high
- category: lsp
- related_task: [[#qmd67_ns_dup]]

### Context [[qmd68_context: text]]

- about: [[#qmd68_converge]]

Evidence gathered during QMD-67 review (all read-only):

- Reference resolution SSOT exists: `core::resolve::ObjectIndex` +
  `core::ops::validate::collect_reference_issues` (`validate.rs:125`), consumed by LSP
  `compute_diagnostics` (`lsp/server.rs:1390`) and MCP `validate` (`tools.rs:223`,
  `resources.rs:86`).
- CLI does NOT use it: `workspace.rs::parse_workspace` has an inline reference scan
  (`workspace.rs:772-1150`) with its own `parse_reference_target` (`workspace.rs:1225`)
  and its own backtick/inline-code suppression.
- `duplicate_id` has THREE implementations: `parser.rs:4372` (same-file/same-kind),
  `workspace.rs:667-770` (cross-file, namespace-scoped per QMD-67), and
  `lsp/server.rs:1198` `seen_ids` (per-document, bare-id, NOT namespace-scoped).
- MCP emits NO `duplicate_id` (reserved comment `validate.rs:170-172`).
- CLI-only diagnostics never reach LSP/MCP: `broken_parent`, `nested_workspace`,
  `__ParsingError` kinds (`workspace.rs` parsing-error pass), and LSP/MCP each cover
  only a subset of `workspace_in_wrong_file`.
- Duplicated helpers: reference-target parsing (`workspace.rs:1225` vs
  `core::resolve` vs py `workspace.py:632` + `db.py:405` vs ts `workspace.ts:581`),
  `__local_id` fallback (workspace.rs vs core::resolve vs `db/mod.rs:703` SQL),
  did-you-mean hint (workspace.rs vs core::resolve vs py/ts), namespace-for-dir
  (`workspace.rs:447` vs `lsp/workspace.rs:205`), index building (3+ places),
  DB edge resolution (`db/mod.rs:648`).

### Goals [[goals: [Goal]]]

#### A1: Cross-file/namespace-scoped duplicate_id in a shared layer [[qmd68_goal_a1]]

Move the namespace-scoped `(namespace, full __id)` duplicate detection (workspace.rs
QMD-67 logic) into a shared core function so it is not tied to the CLI path. Keep the
parser's same-file detection as-is; the shared function owns cross-file + same-file
different-scope detection.

- group: A_lsp_mcp_duplicate
- done: true

#### A2: LSP surfaces cross-file duplicate_id (QMDC003) [[qmd68_goal_a2]]

Have the LSP publish workspace-level `duplicate_id` from the shared layer (A1), not
just the per-document `seen_ids` check (`lsp/server.rs:1198`). The per-doc check is
namespace-blind; the shared one is namespace-correct. Ensure no double-reporting for
same-file duplicates.

- group: A_lsp_mcp_duplicate
- done: true

#### A3: MCP surfaces duplicate_id [[qmd68_goal_a3]]

Decide and implement how MCP exposes `duplicate_id` (either extend
`core::ops::validate` to include structural diagnostics, or add a dedicated
diagnostics op). Update the `validate.rs:170-172` "reserved" comment accordingly.

- group: A_lsp_mcp_duplicate
- done: true

#### B1: Audit + document the full CLI/LSP/MCP diagnostic matrix [[qmd68_goal_b1]]

Produce a diagnostic-by-surface matrix (duplicate_id, broken_link, ambiguous_reference,
ambiguous_field_reference, workspace_in_wrong_file, broken_parent, nested_workspace,
and every `__ParsingError` kind), documenting for each which surface emits it and the
intended converged behavior. Land it in `docs/lsp/diagnostics.qmd.md` (or a new doc).

- group: B_divergence_audit
- done: true

#### B2: Reconcile reference-diagnostic divergences [[qmd68_goal_b2]]

Route the CLI reference scan through `core::resolve`/`collect_reference_issues`,
porting the two CLI-only behaviors first: (a) inline-code/backtick suppression,
(b) the kind/namespace multiplicity ambiguity branch (`workspace.rs:1090`) absent
from core. Align error identity (CLI `broken_link` string ↔ `QMDC001`, etc.). Data-
driven parser-consistency tests must stay green across rs/py/ts.

- group: B_divergence_audit
- done: true

#### B3: Surface structural diagnostics (__ParsingError, broken_parent) in LSP + MCP [[qmd68_goal_b3]]

Extract `__ParsingError` objects and `broken_parent`/`nested_workspace` into the
shared layer so LSP and MCP report them too (dangling_field, mixed_field_keys,
multiple_definitions, structured_in_textblock, same-file duplicate_id). Scope/limits
per B1's agreed matrix.

- group: B_divergence_audit
- done: true

#### C1: Single reference-target parser [[qmd68_goal_c1]]

Collapse the reference-target parsers into one SSOT in `core::resolve` returning
`(namespace, kind, id)`; remove `workspace.rs::parse_reference_target` and the second
Python `db.py::_parse_reference`. Mirror in py/ts. Resolve the kind-handling
divergence (`core::resolve` currently drops kind).

- group: C_dedup
- done: true

#### C2: Single __local_id fallback + did-you-mean hint [[qmd68_goal_c2]]

De-duplicate the `__local_id` namespace-filtered fallback and the cross-namespace
did-you-mean hint (workspace.rs vs core::resolve vs the `db/mod.rs` SQL variant vs
py/ts) to one documented precedence, consumed everywhere.

- group: C_dedup
- done: true

#### C3: Single object-index + namespace-for-dir + edge resolution [[qmd68_goal_c3]]

Build the objects-by-id / by-local-id index once (`ObjectIndex`) and share it with
LSP `WorkspaceInfo`; unify namespace-for-directory (`workspace.rs:447` vs
`lsp/workspace.rs:205`); route DB edge resolution (`db/mod.rs:648`) through the shared
resolver. Centralize the system-kind skip lists into one `is_system_kind()` helper.

- group: C_dedup
- done: true

#### C4: Test-pin cross-surface + cross-parser consistency [[qmd68_goal_c4]]

Add data-driven fixtures asserting identical diagnostics across CLI/LSP/MCP (and
rs/py/ts) for the converged behaviors, so future drift is caught. Prefer
`.sql`/`_expected.json` fixtures over per-language test code.

- group: C_dedup
- done: true
