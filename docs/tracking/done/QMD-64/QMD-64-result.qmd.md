# QMD-64: Result

## Server-owned native fs watcher fixes stale QMDC001 [[qmd64_result: Result]]

Fixed the false-QMDC001 stale-index bug by giving the LSP its own OS-level
filesystem watcher, so the workspace index stays a faithful projection of
on-disk state even when the client never delivers
`workspace/didChangeWatchedFiles` (external tool writes, folder `mv`). Watcher
events are funnelled through the existing `did_change_watched_files` handler, so
all QMD-58 refresh machinery is reused; no reactive resolution-time hack.

- feature: [[#qmd64]]
- files_changed: [qmdc-rs/Cargo.toml, qmdc-rs/src/lsp/server.rs, qmdc-rs/tests/lsp_stale_external_ref.rs]
- tests_added: [qmdc-rs/tests/lsp_stale_external_ref.rs]

### What changed [[qmd64_result_changes: text]]

- `qmdc-rs/Cargo.toml`: added `notify = "8.2"`.
- `qmdc-rs/src/lsp/server.rs`:
  - `Backend` is now `#[derive(Clone)]` (all fields already `Arc`/`Client`), plus
    a new `fs_watcher: Arc<Mutex<Option<RecommendedWatcher>>>` field.
  - `start_fs_watcher` (called from `initialized`) watches each workspace folder
    root recursively; a debounced background task coalesces events and calls
    `did_change_watched_files`.
  - `fs_events_to_changes` maps notify events → `*.qmd.md` `FileEvent`s
    (Create→CREATED, Remove→DELETED, Modify(Name)→CREATED, Modify(other)→CHANGED),
    de-duplicated.
- `qmdc-rs/tests/lsp_stale_external_ref.rs`: new RED→GREEN regression test that
  creates the target on disk with NO watched-file event and asserts the open
  referencing doc reaches 0 QMDC001.

### Verification [[qmd64_result_verify: text]]

- `make test-fast`: 3201 cases, 0 failures (was 3200; +1 new test).
- `cargo clippy --all-targets`, `cargo fmt --check`, `scripts/lint-file-size.sh`:
  all clean.
- The new test is confirmed RED against pre-watcher code (`got Some(1)`) and
  GREEN after the fix.

### Follow-up (non-blocking) [[qmd64_result_followup: text]]

Recursive watch covers the whole folder root; non-`*.qmd.md` events are filtered
early so they trigger no work, but on Linux `inotify` a very large tree could
approach `max_user_watches`. Narrowing the watched set (per-workspace roots,
`.qmdcignore`-aware) can be layered later if it becomes a problem.
