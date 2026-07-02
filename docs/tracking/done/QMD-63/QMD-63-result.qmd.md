# QMD-63: Result

## MCP down-first workspace resolution [[qmd63_result: Result]]

The MCP tools now resolve a workspace from any `path` by searching **down first,
then up**, sharing the discovery primitives with the CLI/LSP instead of the old
upward-only walk. Passing the repo root (or any container) now resolves the
single workspace below it; a container holding several workspaces returns a new
`ambiguous` error listing the candidates. The original repro
(`qmdc_describe_metamodel` with the repo root) now resolves `docs/`.

- feature: [[#qmd63]]
- files_changed: [qmdc-rs/src/core/error.rs, qmdc-rs/src/core/index_seam.rs, qmdc-rs/src/core/mod.rs, qmdc-rs/src/lib.rs, qmdc-rs/src/workspace.rs, qmdc-rs/src/mcp/tools.rs, qmdc-rs/src/mcp/resources.rs, qmdc-rs/src/lsp/server.rs]
- tests_added: [tests/mcp/qmd63-down-single, tests/mcp/qmd63-ambiguous]

### What changed [[qmd63_result_changes: text]]

- `core/index_seam.rs`: new `resolve_root_bidirectional(path)` — self-check →
  down-scan (`find_nested_workspace_roots`: 1 resolves, >1 → `Ambiguous`) → up
  (`find_workspace_root`, `__Workspace`-only, QMD-59 walk-up contract) →
  `NotResolved`. The old upward-only `resolve_root` is retained (still used by
  its existing tests).
- `core/error.rs`: new `ErrorCode::Ambiguous` (`"ambiguous"`) +
  `ErrorEnvelope::error_with_candidates(code, message, candidates)`; the
  candidate workspace paths ride in `error.candidates`.
- `workspace.rs`: added `dir_is_workspace_root` and `owner_root` (shared
  prefix-owner rule); `find_nested_workspace_roots` now sorts shortest-first for
  determinism.
- `mcp/tools.rs`, `mcp/resources.rs`: both entry points call
  `resolve_root_bidirectional` (was `resolve_root`). `force_root` boundary
  unchanged.
- `lsp/server.rs`: `find_workspace_for_file` routes through the shared
  `owner_root`.
- Exports updated in `core/mod.rs` and `lib.rs`.

### Detection decision (regex kept) [[qmd63_result_detection: text]]

Triage's parse-based detection (decision #1) was reverted to the existing regex
`content_has_workspace_marker`. Parse-based (standalone `parse()`) is
inconsistent with the indexer, which recognises bare-anchor readmes and
otherwise falls back to a virtual workspace; going parse-based broke 11
bare-anchor `lsp-microtest` fixtures and changed object composition. Operator
confirmed **option 1: keep regex** (consistent, no fixture churn). True
parse-based unification (migrate ~13 fixtures, reconcile the virtual fallback,
align LSP-vs-CLI detection) remains an optional separate effort. See
[[#qmd63_finding_rootcause]] and the design finding for detail.

### Tests [[qmd63_result_tests: text]]

Data-driven only (no Rust test code), per operator: two MCP fixtures under
`tests/mcp/` driving the real server —

- `qmd63-down-single`: container whose only workspace is one level below →
  resolves it (the repro).
- `qmd63-ambiguous`: container with two sibling workspaces → `ambiguous` error
  with a non-empty `candidates` list.

`make test-fast` is green: 3197 unified cases, 0 failures (was 3195; +2 new).
