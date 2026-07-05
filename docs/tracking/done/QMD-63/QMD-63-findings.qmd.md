# QMD-63: Findings

Triage of MCP workspace resolution when the caller passes a path that is not
itself inside a workspace (e.g. the repo root).

## Root cause: three divergent resolvers; MCP's is upward-only [[qmd63_finding_rootcause: Finding]]

The MCP path uses a different resolver than the LSP and CLI, and it only walks
upward. The deeper problem is that "find the workspace" is implemented three
times with different logic.

- category: parser
- related_to: [[#qmd63]]
- solution: consolidate one shared core discovery primitive; route MCP (and ideally LSP) through it

### Three implementations today [[qmd63_rc_three: text]]

- **LSP** (`qmdc-rs/src/lsp/server.rs` ~256) scans **downward** from the
  editor's workspace folder with `WalkDir`, `.qmdcignore`-aware, detecting
  `__Workspace` via `parse_and_index` (`__kind == "__Workspace"`), sorts roots
  shortest-first, and excludes nested files. Per request it maps a path to its
  owner with `find_workspace_for_file` (server.rs:635) by prefix match —
  comment: **"nested workspaces are not allowed, so there should be only one
  match"**. This is the behaviour we want, and it already works.
- **CLI** (`main.rs`) uses `workspace::resolve_workspace` (`workspace.rs:1568`,
  QMD-59): walk-UP via `find_workspace_root`, else walk-DOWN via
  `parse_all_workspaces` → `find_nested_workspace_roots` (regex marker match).
- **MCP** (`mcp/tools.rs::resolve_and_index` ~97, `mcp/resources.rs:157`) uses
  `core/index_seam.rs::resolve_root` — a bounded **upward-only** walk that
  stops at the first `__Workspace`/`__Namespace` readme, `.git`, fs root, or
  `MAX_UPWARD_WALK` (64).

### Why the repo root fails [[qmd63_rc_why: text]]

From the repo root the MCP upward walk finds no marker (only `README.md`), hits
`.git`, and returns `not-resolved` — it never looks *down* into `docs/`
(`[[docs:__Workspace]]`). The git-worktree detail (`.git` is a gitlink *file*)
is **not** the cause: `current.join(".git").exists()` is true for the file too.
Confirmed: `./qmdc query ./docs` works (294 objects); the LSP resolves this
repo fine.

## Workspaces vs namespaces; nesting is illegal [[qmd63_finding_model: Finding]]

Clarifies the data model that bounds the ambiguity logic.

- category: parser
- related_to: [[#qmd63]]
- solution: down-search targets `__Workspace` markers only; multiple disjoint hits = ambiguous error

### Model [[qmd63_model_rules: text]]

- A **workspace** is a dir whose `readme.qmd.md` declares `[[id:__Workspace]]`.
  A **namespace** (`[[id:__Namespace]]`) is a subdivision *inside* a workspace,
  not a workspace itself.
- **A workspace cannot be nested inside another workspace** — that is an
  invalid configuration, not a tie-break case. So the "innermost wins" question
  for down-search is moot: workspace roots found below a path are disjoint
  sibling subtrees.
- Under this repo root there is exactly **one** workspace: `docs/`.
  `docs/mcp/readme.qmd.md` is `[[mcp:__Namespace]]` (a namespace, correct).
  (An earlier `grep __Workspace` false-matched body prose in that file, not the
  `[[…:__Workspace]]` marker — `find_nested_workspace_roots` uses the marker
  regex, so it would NOT treat `docs/mcp` as a workspace.)
- `find_nested_workspace_roots` (`workspace.rs:86`) matches the `__Workspace`
  marker only and honours `.qmdcignore` — the right primitive for down-search.

## Proposed resolution: one shared discovery primitive [[qmd63_finding_design: Finding]]

A single core primitive shared by LSP and MCP (and the CLI), not an MCP-only
resolver.

- category: parser
- related_to: [[#qmd63]]
- solution: extract the LSP's down-scan + prefix-owner logic into a transport-agnostic core primitive; MCP and LSP both call it

### Shared primitive (core, transport-agnostic) [[qmd63_design_shared: text]]

Promote the workspace-discovery logic the LSP already uses into one core
function (the docs already claim MCP "reuses the transport-agnostic core layer
shared with the LSP"):

- `discover_workspace_roots(base) -> Vec<PathBuf>` — downward `WalkDir`,
  `.qmdcignore`-aware, detects `__Workspace`, sorted shortest-first. This
  replaces the LSP inline scan (server.rs ~256) and `find_nested_workspace_roots`.
- `owner_root(roots, path) -> Option<PathBuf>` — the single root that is a
  prefix of `path` (nesting disallowed ⇒ at most one), i.e. the core version of
  `find_workspace_for_file` (server.rs:635).

LSP keeps its multi-workspace registry but builds it from
`discover_workspace_roots`; per-request lookups use `owner_root`. No behaviour
change for the LSP — just deduplication onto the shared primitive.

### MCP resolution on top of the shared primitive [[qmd63_design_mcp: text]]

MCP has a single-root contract (one `path` in → one root to index), so it
composes the shared primitive as **down-first, then up**:

1. **Down**: `roots = discover_workspace_roots(path)`.
   - If `path` is itself / inside exactly one root (`owner_root` or a single
     discovered root at/below `path`) → resolve it.
   - If `>1` disjoint root is found below `path` (container case, e.g. repo
     root over several workspaces) → **ambiguous error** with candidates (see
     [[#qmd63_finding_error]]).
2. **Up (fallback)**: if nothing is found downward, walk upward (existing
   `resolve_root` logic) — the "caller already deep inside a workspace whose
   root is above `path`" case (e.g. `path = docs/tracking`, a namespace subdir;
   owner `docs/` is above).
3. None → actionable `not-resolved`.

The repo-root repro: `discover_workspace_roots(repo_root)` returns exactly
`[docs/]` (mcp is a namespace) → single → resolves to `docs/`.

### Placement & reuse [[qmd63_design_reuse: text]]

- `core::index_seam` already depends on `crate::workspace`, so the primitive can
  live in `core`/`workspace` and be called from both `lsp` and `mcp` with no new
  dependency direction.
- `force_root` fail-closed boundary still wraps the MCP result, and every
  ambiguous candidate must lie within `force_root` when set.
- `get_index` is unchanged (still takes a single resolved root); only the
  resolution step in front of it changes.
- CLI `resolve_workspace` can also be re-expressed on the primitive, but that is
  optional cleanup — its parse/union contract differs from MCP's single-root.

### Decisions (resolved during triage) + implementation note [[qmd63_design_oq: text]]

1. **`__Workspace` detection — parse-based was reverted to the existing regex.**
   Triage chose parse-based, but during implementation it proved **inconsistent
   with how the codebase actually recognises workspaces**, and breaks legacy
   fixtures:
   - Standalone `parse()` of a *bare-anchor* readme (`# Title` then a separate
     `[[id:__Workspace]]` line — the form used by 11 lsp-microtest fixtures and
     the `create_workspace` test helper) yields **no** `__Workspace` object
     (`__Document`/`__TextBlock` instead), so parse-based detection says "not a
     workspace".
   - But the indexer (`parse_workspace`) treats those dirs as workspaces anyway
     via its **virtual-workspace fallback** (`workspace_id = None` →
     folder-named `__Workspace`). The long-standing resolver used the regex,
     which matches the bare anchor and stays consistent with that.
   - Going parse-based therefore (a) made resolution fail for those dirs and (b)
     when fixtures were migrated to heading form, changed object composition
     (dropped the virtual ws + readme `__Document`/`__TextBlock`), breaking 3
     `mcp-expected.json`.
   - Detection was historically already split: LSP scan = `parse_and_index`
     (parse-based), CLI/resolve = regex. Unifying on parse-based is a larger,
     separate effort (migrate ~13 fixtures, reconcile the virtual fallback,
     update affected expectations).
   - **Implemented:** kept the regex `content_has_workspace_marker` as the
     shared detector (used by `dir_is_workspace_root`, `find_nested_workspace_roots`,
     `find_workspace_root`, `parse_all_workspaces`). The LSP scan is left on
     `parse_and_index` (unchanged). This is consistent with the resolver/indexer
     and needs no fixture churn. **Awaiting operator confirmation** on whether to
     pursue true parse-based unification separately (optionally fence-aware regex
     to address the code-block false-match concern).
2. **LSP dedup (scope) — reverted.** An earlier change routed
   `find_workspace_for_file` through a shared `owner_root` helper. It was
   behaviour-equivalent (nesting is illegal ⇒ at most one prefix match) and not
   needed for the MCP fix, so it was reverted: the LSP is left untouched and
   `owner_root` was removed. MCP reuses `find_workspace_root` and a depth-bounded
   `find_nested_workspace_roots_bounded`.
3. **Down-scan is bounded (MCP only).** `find_nested_workspace_roots` stays
   unbounded (CLI/`parse_workspace` need complete nested-workspace detection);
   the MCP resolver uses `find_nested_workspace_roots_bounded(_, WORKSPACE_SCAN_MAX_DEPTH=5)`.
   `.qmdcignore` now prunes descent (`filter_entry`) in both.

## Ambiguous case → new error code + standard envelope [[qmd63_finding_error: Finding]]

Shape of the multiple-workspaces error, per the agreed "it's an error" decision.

- category: parser
- related_to: [[#qmd63]]
- solution: add `ErrorCode::Ambiguous` ("ambiguous"), carry candidates in the error envelope; no new success format

### Shape [[qmd63_error_shape: text]]

- Keep the single `{success:false, error:{code, message, ...}}` envelope. No
  second success-shaped response.
- **Add a new `ErrorCode::Ambiguous`** (string `"ambiguous"`) — operator
  decision. The enum in `core/error.rs` is deliberately exhaustive/fail-closed,
  so adding a variant means updating all match sites; that is intended and keeps
  the ambiguous case semantically distinct from `not-resolved` (nothing found).
- Carry the candidate workspace paths in an added optional `error.candidates`
  field (precedent: `WorkspaceError.candidates` in `workspace.rs`). Message:
  list the found workspaces and instruct the caller to re-call with one as
  `path`.
- `ErrorEnvelope::error` currently takes `(code, message)`; add
  `error_with_candidates(code, message, Vec<String>)` (or make candidates an
  optional param) so the envelope can include them.

### Open questions [[qmd63_error_oq: text]]

1. Candidate paths absolute vs workspace-relative in the envelope (lean
   absolute, since the caller re-calls with one as `path`).

## Test plan [[qmd63_finding_tests: Finding]]

How to verify, reusing the existing fixture corpus where possible.

- category: testing
- related_to: [[#qmd63]]
- solution: reuse existing `tests/workspace/*` fixtures for the core primitive; add Rust integration tests for the MCP single-root + ambiguous contract

### Existing landscape [[qmd63_tests_landscape: text]]

- **CLI resolution is already pinned cross-parser** by QMD-59
  (`qmdc-py/tests/test_workspace.py`): `container-root-single-workspace` →
  walk-down resolves the single sub-workspace; `walkup-subdir-of-workspace/sub`
  → walk-up resolves the parent. These run the `qmdc` binary in Python/TS/Rust.
- **LSP** discovery/nesting is exercised by `qmdc-rs/tests/lsp.rs` plus the
  `nested-workspace*` / `multi-workspace*` fixtures.
- **MCP resolution has no such coverage** — it uses `resolve_root` directly and
  is the gap this task closes.
- Ready-made fixtures to reuse: `container-root-single-workspace` (down→single),
  `multi-workspace` / `multiple-workspaces` (down→ambiguous),
  `nested-workspace-error` (nesting is an error), `walkup-subdir-of-workspace`
  (up fallback).

### Why some tests are Rust [[qmd63_tests_why: text]]

The data-driven `.sql`+`.expected.json` harness covers parse/query over a
resolved workspace; it cannot express **filesystem resolution** (down/up
walking, `.git` boundary, multiple roots, the new `ambiguous` error). Those are
Rust integration tests in `qmdc-rs/tests/core_index_seam.rs`, consistent with
the existing `resolve_root` tests and their `TempDir` helpers. The core
primitive itself is unit-tested by **pointing it at the existing
`tests/workspace/*` fixtures** (no new fixtures needed).

### New cases [[qmd63_tests_cases: text]]

Core primitive (against existing fixtures):

- **discover-roots / single**: `container-root-single-workspace` → exactly one
  root (`docs/`).
- **discover-roots / ambiguous**: `multi-workspace` (or `multiple-workspaces`)
  → >1 disjoint root.
- **owner-root / up**: `walkup-subdir-of-workspace/sub` → owner is `walkup_ws`.
- **nesting**: `nested-workspace-error` → treated as the existing error, not a
  silent pick.

MCP resolve contract (Rust integration, `TempDir` + the `.git` helper):

- **down-resolves-single**: `.git` + no marker at root, one workspace in
  `root/docs` → resolves `root/docs` (the QMD-63 repro; currently fails).
- **down-ambiguous-errors**: `.git` at root, two sibling workspaces → new
  `ambiguous` error with both in `candidates`.
- **inside-workspace-still-up**: path inside a namespace subdir → up-walk wins.
- **self-is-workspace**: `path` == root resolves to itself.
- **regression**: existing `resolve_root` and `lsp.rs` suites stay green after
  the LSP is moved onto the shared primitive.

### How to verify [[qmd63_tests_verify: text]]

`make test-fast` (parallel, Python/TS/Rust) must stay green. Manually re-run the
original MCP repro (`qmdc_describe_metamodel` with the repo root) and confirm it
resolves to `docs/`; re-run with a two-workspace container and confirm the
`ambiguous` error with candidates.
