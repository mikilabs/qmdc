# QMD-64: Findings

Triage of the false-QMDC001 stale-index bug: cross-file references in an
already-open document keep a "not found" squiggle after the target is
created/moved on disk outside the editor, even though hover/card/CLI resolve it.

## Root cause: push diagnostics go stale; refresh depends on watched-file events [[qmd64_finding_rootcause: Finding]]

The precise asymmetry is **push vs pull**, layered on top of the cached
workspace index. Cross-file resolution for BOTH diagnostics and hover uses the
same cached `ws.objects` index — so "hover works, diagnostics don't" is not
about different resolvers, it is about *when each runs*.

- category: lsp
- related_to: [[#qmd64]]
- solution: make cross-file diagnostics self-heal at resolution time instead of relying solely on watched-file-driven re-publish

### Pull (hover/card/definition) vs push (diagnostics) [[qmd64_rc_pushpull: text]]

- Hover / go-to-definition / the card are **pull**: `find_object_in_workspace_with_namespace`
  (`server.rs:862`) runs fresh on every user request against the current index,
  so once the index is healthy they resolve immediately.
- Diagnostics are **push**: `compute_diagnostics` (`server.rs:1031`) runs and is
  `publish_diagnostics`'d only when *something re-publishes the referencing doc*.
  For an already-open referencing file that itself did not change, that only
  happens via QMD-58's `rescan_workspaces` / `refresh_open_documents` paths, which
  are triggered by `did_change_watched_files`.
- So if the index is refreshed by a later save/rescan but the referencing open
  doc is never re-diagnosed, its previously-published QMDC001 lingers until
  `Restart Language Server`. And if the index is never refreshed at all (no
  event), it lingers for the same reason.

### Both cross-file paths read the same cached index [[qmd64_rc_index: text]]

`compute_diagnostics` resolves the open doc's refs via
`core::ops::validate::collect_reference_issues(&index_objects, &iter_objects)`
where `index_objects` = whole-workspace `ws.objects` (minus the open file's stale
copy) ∪ the freshly-parsed open doc. `ws.objects` comes from
`find_workspace_for_file` and is only mutated on the editor's `did_open`/`did_save`,
the initial scan, and `did_change_watched_files`. Files created/moved on disk
that never produce a delivered watched-file event are absent from `ws.objects`
→ false QMDC001 for cross-file refs to their objects.

### Why the QMD-58 machinery does not close it [[qmd64_rc_gap: text]]

QMD-58 already added: server-side file-watch registration (`server.rs` ~1809,
glob `**/*.qmd.md`), a client-side watcher (`qmdc-vscode/src/extension.ts:227`
`synchronize.fileEvents`), rescan-on-CREATE/DELETE, incremental re-index on
CHANGE, and `refresh_open_documents` / `refresh_other_open_documents`. The
remaining gap is that **every one of those refresh paths is gated on a delivered
`workspace/didChangeWatchedFiles` notification**, which is unreliable for the
exact triggers in this bug:

1. Programmatic/tool writes and batch **folder moves** (`planned/ → active/`, as
   happened during QMD-63) frequently do not produce per-file `**/*.qmd.md`
   events the client forwards.
2. The CHANGE branch's cross-file re-publish is further guarded by
   `ids_changed` (`server.rs` ~2205): if the delivered event is CHANGED and the
   id-set looks unchanged vs the cached copy, other open docs are not refreshed.

CLI `validate` never sees any of this because it always parses from disk (why
`make test-fast` is green while the editor shows red).

## Chosen fix: server owns a native filesystem watcher [[qmd64_finding_design: Finding]]

Make the index an authoritative projection of on-disk state by having the LSP
own an OS-level watcher, instead of a reactive self-heal that only *guesses* the
index is stale after a QMDC001 already fired. The server no longer depends on
the client volunteering `didChangeWatchedFiles`.

- category: lsp
- related_to: [[#qmd64]]
- solution: add a native `notify` watcher over workspace folder roots; funnel its events through the existing `did_change_watched_files` handler so all QMD-58 refresh machinery is reused

### Why not the reactive self-heal (rejected) [[qmd64_design_rejected: text]]

An earlier triage draft proposed a resolution-time fallback: on a QMDC001 whose
id is missing from the index, re-scan disk and recompute. Rejected as an ugly
hack — it encodes distrust of our own index and only reacts *after* a false
error would be emitted, guaranteeing nothing about index freshness for other
features. The right fix is to keep the index correct in the first place.

### Mechanism [[qmd64_design_mechanism: text]]

- `Backend` becomes `#[derive(Clone)]` — every field is already `Arc`/`Client`,
  so a clone is cheap and shares the same state. This lets a background task hold
  a Backend handle.
- On `initialized`, `start_fs_watcher` creates a `notify::recommended_watcher`
  and watches each workspace folder root recursively. The watcher handle is
  stored in a new `fs_watcher: Arc<Mutex<Option<RecommendedWatcher>>>` field to
  keep it alive for the life of the server.
- notify's sync event callback forwards raw events into an unbounded channel. A
  spawned task drains it, debounces (~150 ms, so a folder move = one batch),
  coalesces via `fs_events_to_changes`, and calls the SAME
  `did_change_watched_files(params)` used for the client notification.
- `fs_events_to_changes` maps notify kinds → LSP `FileEvent`s for `*.qmd.md`
  only (Create→CREATED, Remove→DELETED, Modify(Name)→CREATED so a rename forces a
  rescan, Modify(other)→CHANGED), de-duplicated per (path, type).

Because everything routes through `did_change_watched_files`, all existing
QMD-58 behaviour is reused unchanged (rescan on create/delete, incremental
re-index on change, re-publish of open docs). Client + native double-delivery is
harmless — the handler is idempotent.

### Scope / cost (NFR-2) [[qmd64_design_bound: text]]

- Correctness-first: structural events reuse full `rescan_workspaces`; the
  ~150 ms debounce collapses bursts (folder moves) into a single rescan.
- Known limitation (follow-up, not blocking): the recursive watch covers the
  whole folder root, so on Linux `inotify` a very large tree could approach
  `max_user_watches`. Events for non-`*.qmd.md` files are filtered out early so
  they never trigger work; narrowing the watched set (per-workspace roots,
  `.qmdcignore`-aware) can be layered later if needed.

## Open questions [[qmd64_finding_oq: Finding]]

Triage answers to the task's open questions.

- category: lsp
- related_to: [[#qmd64]]
- solution: OQ1 = server owns the watcher (native), OQ2/OQ3 resolved by choosing approach A, OQ4 confirmed independent

### Resolved [[qmd64_oq_resolved: text]]

- **OQ1 (register file watchers / who watches?)** — the server now owns a native
  watcher; it no longer relies solely on the client's `register_capability` +
  `synchronize.fileEvents`, which drop events for programmatic writes / folder
  moves. Client events remain as a redundant fast path (idempotent).
- **OQ2 (live re-scan fallback?)** — rejected in favour of A (see the
  rejected-alternative note in [[#qmd64_finding_design]]): keep the index
  authoritative rather than papering over staleness at resolution time.
- **OQ3 (rescan scope)** — reuse full `rescan_workspaces` (correctness-first);
  debounce bounds cost. Narrowing is a documented follow-up, not required.
- **OQ4 (independent of QMD-63?)** — confirmed independent: single-workspace
  `find_workspace_for_file` mapping is unaffected by QMD-63, and CLI validate is
  clean.

## Test plan [[qmd64_finding_tests: Finding]]

The bug is LSP cache-invalidation behaviour, which the data-driven
`.sql`+`.expected.json` harness cannot express (it has no editor session / cache
/ delivered-vs-missed notifications). Cover it with a Rust LSP integration test
mirroring the existing style, and explicitly WITHOUT sending the watched-file
event — that omission is the whole point vs QMD-58.

- category: testing
- related_to: [[#qmd64]]
- solution: built `qmdc-rs/tests/lsp_stale_external_ref.rs` — creates a target on disk with NO didChangeWatchedFiles and asserts the open referencing doc reaches 0 QMDC001 (RED before the watcher, GREEN after)

### Existing coverage and the gap [[qmd64_tests_landscape: text]]

- `qmdc-rs/tests/lsp_stale_diagnostics.rs::qmd58_forward_reference_clears_after_target_created`
  proves the CREATE case clears QMDC001 **when the `didChangeWatchedFiles`
  CREATED event is sent**.
- `qmdc-rs/tests/lsp_did_save_sync.rs::test_did_change_watched_files_*` prove the
  index/SQLite update **when the event is sent**.
- None of them cover the case where the event is **not delivered** (the QMD-64
  repro) — that is the gap this task closes.

### New test (LSP integration, Rust) — built [[qmd64_tests_new: text]]

`qmdc-rs/tests/lsp_stale_external_ref.rs::qmd64_external_target_resolves_without_watched_file_event`,
driving the real `Backend` over `tower::Service` and capturing `publishDiagnostics`
(helpers copied from `lsp_stale_diagnostics.rs`):

1. Workspace on disk: `readme.qmd.md` (`__Workspace`) + `a.qmd.md` with
   `- depends: [[#b_obj]]`. `b_obj` does not exist yet.
2. `initialize` + `initialized`, then `didOpen` `a.qmd.md`.
   Precondition assert: `a.qmd.md` has exactly 1 QMDC001.
3. Create `b.qmd.md` on disk defining `b_obj` — and send **NO**
   `didChangeWatchedFiles` (simulating the missed event / external move).
4. A natural `didChange` of `a.qmd.md` follows; assert `a.qmd.md` reaches **0**
   QMDC001. The native watcher observes the disk write and rescans, so the index
   is fresh with no client watch event and no restart.

Confirmed RED against pre-watcher code (`got Some(1)`), GREEN after the fix.

### How to verify [[qmd64_tests_verify: text]]

`make test-fast` (Python/TS/Rust) green — 3201 cases, 0 failures, including the
new test and the existing QMD-58 event-delivered test (no regression). `cargo
clippy --all-targets`, `cargo fmt --check`, and `scripts/lint-file-size.sh` all
clean. Manually: in the editor, with a file open referencing an object,
create/move the target file via a tool (no manual save) and confirm the squiggle
clears without `Restart Language Server`.
