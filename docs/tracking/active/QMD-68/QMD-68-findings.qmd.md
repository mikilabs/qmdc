# QMD-68: Findings

## Open questions — need operator decisions before implementation [[qmd68_finding_open_questions: Finding]]

Design forks that materially change scope/approach. Triage stops here for answers.

- category: lsp
- related_to: [[#qmd68_converge]]

### Questions [[qmd68_oq: text]]

- about: [[#qmd68_converge]]

1. **MCP duplicate_id surface (A3).** (a) extend `core::ops::validate` to also emit
   structural diagnostics, or (b) separate `diagnostics` op keeping
   `validate_references` reference-only?
2. **Scope of this single task.** All of A+B+C in QMD-68, or split B2/B3 + C into a
   follow-up?
3. **Cross-parser SSOT reality.** Per-language SSOT (Rust → core; py/ts parallel copies
   pinned by consistency fixtures) — acceptable?
4. **Backtick / inline-code suppression parity.** Port suppression into core (LSP/MCP
   stop flagging inline-code refs) or drop it from CLI?
5. **LSP duplicate_id: replace or augment** the per-document `seen_ids` check?

### Operator decisions [[qmd68_oq_decisions: text]]

- about: [[#qmd68_converge]]

1. **(a)** — extend `core::ops::validate` to emit structural diagnostics too. The
   existing `validate_references` output contract is NOT a constraint right now.
2. **All of A+B+C in QMD-68.** Further follow-ups can happen later as needed.
3. **Per-language SSOT is correct** — Rust consolidates into `core`; py/ts remain
   parallel copies pinned by `validation-parser-consistency`.
4. **Do NOT resolve references inside single (inline) backticks.** Port the
   suppression into the shared core path so CLI/LSP/MCP all skip inline-code refs
   (correct behavior = no broken_link for a ref inside `` `...` ``).
5. **Augment, do not remove.** Same-file duplicate detection MUST keep working at
   single-document scope (no workspace needed), so the per-document check stays; add
   cross-file/namespace-scoped detection on top, without double-reporting a same-file
   duplicate.

## Part A — namespace-scoped duplicate_id shared by LSP + MCP [[qmd68_finding_duplicate: Finding]]

Make cross-file/namespace-scoped duplicate detection reusable and surface it in LSP+MCP.

- category: lsp
- related_to: [[#qmd68_converge]]

### Affected [[qmd68_dup_affected: text]]

- about: [[#qmd68_converge]]

- `qmdc-rs/src/workspace.rs:667-770` — the QMD-67 `(namespace, full __id)` grouping to
  extract into a shared `core` function (e.g. `core::ops::duplicates` or part of a
  structural-diagnostics module).
- `qmdc-rs/src/lsp/server.rs:1198-1253` — per-document `seen_ids` duplicate check; add
  workspace-level results from the shared fn (see open question 5).
- `qmdc-rs/src/core/ops/validate.rs:170-172` — the "QMDC003 reserved" comment; MCP
  wiring per open question 1.
- `qmdc-rs/src/mcp/{tools.rs:215,resources.rs:86}` — MCP entry points.

### Solution [[qmd68_dup_solution: text]]

- about: [[#qmd68_converge]]

Add a shared core function `duplicate_id_issues(objects) -> Vec<Issue>` keyed by
`(namespace, full __id)`, preserving current exclusions (`__ParsingError`,
`__Document`/`__TextBlock`) and the same-file/different-scope branches. CLI calls it
(replacing its inline block); LSP consumes it for cross-file duplicates; MCP exposes it
per open question 1. Parser same-file detection (`parser.rs:4372`) stays as the source
for same-file/same-kind.

### Test plan [[qmd68_dup_tests: text]]

- about: [[#qmd68_converge]]

Data-driven fixtures asserting duplicate_id parity across CLI/LSP/MCP: reuse
`duplicate-id-namespace-scoping` + `duplicate-id-nested-namespace-scoping` (QMD-67) and
`validation-parser-consistency`. Add an LSP microtest (there is precedent under the LSP
test suite) asserting cross-file duplicate_id is published, and an MCP fixture asserting
the tool/resource now returns it.

## Part B — reconcile CLI/LSP/MCP divergences [[qmd68_finding_divergence: Finding]]

Route the CLI through the shared resolver and surface structural diagnostics everywhere.

- category: lsp
- related_to: [[#qmd68_converge]]

### Divergences to close [[qmd68_div_list: text]]

- about: [[#qmd68_converge]]

- CLI reference scan is a second engine (`workspace.rs:772-1150`) vs shared
  `collect_reference_issues` (`validate.rs:125`). Divergences: error label
  (`broken_link` vs QMDC001), a CLI-only kind/namespace multiplicity ambiguity branch
  (`workspace.rs:1090`), and CLI-only backtick suppression (open question 4).
- Structural diagnostics CLI-only: `broken_parent` (`workspace.rs` Phase 3),
  `nested_workspace`, all `__ParsingError` kinds (dangling_field, mixed_field_keys,
  multiple_definitions, structured_in_textblock, same-file duplicate_id).
- `workspace_in_wrong_file`: CLI (workspace-level) + LSP (`server.rs:1270`, single-doc);
  MCP none.

### Solution [[qmd68_div_solution: text]]

- about: [[#qmd68_converge]]

B1: write the diagnostic×surface matrix into `docs/lsp/diagnostics.qmd.md`. B2: port
backtick suppression + the extra ambiguity branch into `core::resolve`, then delete the
CLI inline scan and map `RefIssue`→`WorkspaceError`. B3: extract `__ParsingError` +
`broken_parent`/`nested_workspace` into the shared layer for LSP/MCP. Keep rs/py/ts
byte-compatible via consistency fixtures.

### Test plan [[qmd68_div_tests: text]]

- about: [[#qmd68_converge]]

Extend `validation-parser-consistency` with inline-code-ref and kind/namespace-ambiguity
cases. Add fixtures where a workspace has structural errors and assert LSP + MCP now
report them. Run the three shared workspace conformance suites + LSP microtests.

## Part C — de-duplicate resolution/index logic [[qmd68_finding_dedup: Finding]]

Collapse the copied helpers to one source of truth per language.

- category: parser
- related_to: [[#qmd68_converge]]

### Duplication inventory [[qmd68_dedup_list: text]]

- about: [[#qmd68_converge]]

- Reference-target parsing: `workspace.rs:1225` (3-part) vs `core::resolve`
  `extract_id_from_target`+`parse_ref_namespace` (drops kind) vs py
  `workspace.py:632` + `db.py:405` vs ts `workspace.ts:581`.
- `__local_id` fallback: `workspace.rs:884` vs `core::resolve::resolve` vs
  `db/mod.rs:703` (SQL) — three namespace-filter semantics.
- Did-you-mean hint: `workspace.rs:965` vs `core::resolve` vs py/ts.
- Object index build: `workspace.rs:587/624` vs `lsp/workspace.rs:178` vs
  `core::resolve::ObjectIndex::build` — differing system-kind inclusion.
- Namespace-for-dir: `workspace.rs:447` (parsed-readme map) vs `lsp/workspace.rs:205`
  (filesystem walk).
- DB edge resolution: `db/mod.rs:648` — 4th resolution chain, no field/hierarchical/kind
  handling.
- System-kind skip lists hardcoded and divergent (`workspace.rs:626` vs `:751`; core
  skips none).

### Solution [[qmd68_dedup_solution: text]]

- about: [[#qmd68_converge]]

Rust: one target parser in `core::resolve` returning `(ns, kind, id)`; one
`__local_id`+hint path; build `ObjectIndex` once and let LSP `WorkspaceInfo` hold/borrow
it; route DB edges through `ObjectIndex::resolve_object`; one `is_system_kind()`. py/ts:
mirror into their own single helpers. Sequencing per open question 2 — most of C
collapses for free once B2 lands.

### Test plan [[qmd68_dedup_tests: text]]

- about: [[#qmd68_converge]]

Rely on existing parser/workspace/sql conformance suites as the safety net (they already
pin cross-parser behavior); add targeted fixtures for edge-resolution and namespace-for-dir
parity. C4 adds explicit cross-surface consistency fixtures. No behavior change expected —
these are refactors validated by unchanged expected outputs.
