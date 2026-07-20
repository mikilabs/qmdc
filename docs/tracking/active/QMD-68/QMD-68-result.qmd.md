# QMD-68: Result

## Converged validation across CLI / LSP / MCP [[qmd68_result: Result]]

The three surfaces now share ONE reference-validation engine and ONE duplicate detector,
and LSP + MCP surface the workspace-level diagnostics they previously missed.

- feature: [[#qmd68_converge]]

### Files Changed [[files_changed: array]]

- qmdc-rs/src/core/reference_scan.rs
- qmdc-rs/src/core/mod.rs
- qmdc-rs/src/core/ops/validate.rs
- qmdc-rs/src/core/resolve.rs
- qmdc-rs/src/core/fields.rs
- qmdc-rs/src/workspace.rs
- qmdc-rs/src/lsp/server.rs
- qmdc-py/qmdc/workspace.py
- qmdc-ts/src/workspace.ts
- docs/lsp/diagnostics.qmd.md
- tests/lsp/microtests/diagnostics/030-crossfile-duplicate-id/
- tests/lsp/microtests/diagnostics/031-structural-multiple-definitions/
- tests/lsp/microtests/diagnostics/032-ambiguous-crossns-ref/
- tests/lsp/microtests/diagnostics/028-ambiguous-field-ref/ (enriched message on all surfaces)

### Summary [[summary: text]]

Part A — duplicate_id: added `core::ops::validate::collect_duplicate_issues` /
`cross_file_duplicate_keys` (namespace-scoped, QMD-67 semantics). MCP `validate` emits
duplicate_id; LSP publishes cross-file duplicate_id (per-file, no double-report with the
same-file `seen_ids`). Python/TS `__ParsingError` skip aligned with Rust.

Part B — divergence: added the shared reference engine `core::reference_scan::reference_scan`
(the CLI's exact algorithm, lifted into core). The CLI, LSP, and MCP all call it, so
broken_link / ambiguous_reference / ambiguous_field_reference can never drift. Structural
diagnostics (`broken_parent`, `nested_workspace`, `workspace_in_wrong_file`, and every
parser `__ParsingError` kind) now surface in MCP (via the index's WorkspaceResult) and in
the LSP (`__ParsingError` for the open doc). Surface-coverage matrix documented in
`docs/lsp/diagnostics.qmd.md`.

Part C — dedup: removed the duplicate reference resolver — deleted
`core::resolve::{Resolution, ObjectIndex::resolve, field_ref_resolves}`,
`core::ops::validate::{RefIssue, collect_reference_issues}`, and the CLI's inline reference
loop, `parse_reference_target`, `is_inside_backticks`, `ObjectLocation`, and `by_local_id`
build. One `parse_reference_target` lives in `core::reference_scan`. Added a single
`core::fields::is_system_kind` used by the duplicate detector.

Verification: `make test-fast` green (rs 157 nextest incl. 6 new duplicate unit tests;
py/ts workspace conformance; LSP + MCP fixtures). New characterization fixtures 030
(cross-file duplicate_id), 031 (structural), 032 (cross-namespace ambiguity) pin LSP + MCP.
Independent subagent code review: no BLOCKING/MAJOR issues.

### Scope Delivered vs Deferred [[qmd68_scope: text]]

- about: [[#qmd68_converge]]

**Delivered (Rust):** one reference engine + one duplicate detector shared by CLI/LSP/MCP;
one target parser; one `is_system_kind`; structural diagnostics on all surfaces.

**Intended convergence side-effects (now covered by tests):** LSP + MCP now detect the
kind/namespace-multiplicity `ambiguous_reference` (fixture 032, previously under-reported).
The `ambiguous_field_reference` message (QMDC009) now carries the two conflicting
candidate interpretations on ALL three surfaces (fixture 028) — CR #02 Q2 converged the
CLI/LSP/MCP *up* to the enriched form rather than down to the bare sentence.

**Post-review fixes (CR #02, `reviews/02-cr-qmd68-validation-converge.md`):**

- The CLI now routes `duplicate_id` through the shared
  `core::ops::validate::collect_duplicate_issues` (its inline detector was removed), so
  CLI/LSP/MCP genuinely share ONE duplicate detector — closing the last drift gap
  (blocker B1 + divergence H1: per-object `is_system_kind` skip, deterministic order).
- QMDC009 message enriched with candidate detail on all surfaces (Q2).
- LSP cross-file `duplicate_id` anchors on the first open-file occurrence per key, so it
  can no longer double-report with the same-file `seen_ids` pass (M1).
- Guarded `__line == 0` underflow in the LSP `seen_ids` and workspace-in-wrong-file
  passes (M7).

**Deferred (deliberately, lower value / higher risk — recommend follow-up tasks):**

- Python/TS keep their own parallel validators (per the operator's per-language-SSOT
  decision), pinned by `validation-parser-consistency`. The intra-Python second target
  parser (`qmdc-py/qmdc/db.py::_parse_reference`) was left in place.
- The DB graph-edge resolver (`qmdc-rs/src/db/mod.rs::resolve_target_global_id`) is a
  separate SQL-based chain used only by `traverse`/`find_path`; not unified with the
  in-memory resolver.
- `ObjectIndex::resolve_object`/`resolve_id` (used by locate/describe/find_references/
  rename) retain their own local-id logic; not merged into `reference_scan`.
- MCP does not apply double-backtick inline-code suppression (relies on the parser's
  single-backtick stripping); no test covers double-backtick on the MCP path.
