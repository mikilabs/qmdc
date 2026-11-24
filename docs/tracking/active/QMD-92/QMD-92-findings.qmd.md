# QMD-92: Findings

## A second publish lands inside the harness's wait [[qmd92_finding_cause: Finding]]

`run_lsp_test` in `qmdc-rs/tests/lsp.rs` copies the workspace into a temp dir, starts the
server and opens every file. For a `diagnostics` test it then waits 200 ms and appended the
`diagnostics` of every `publishDiagnostics` notification for the target file. The server can
publish more than once for a file: the native fs watcher started in `initialized`
(`start_fs_watcher`) can still receive `Created` events for the files the harness just copied.
After its 150 ms debounce, `did_change_watched_files` rescans and re-publishes every open
buffer. The failing run's log shows `did_change_watched_files: 2 changes` (`Created doc.qmd.md`,
`Created readme.qmd.md`) and a rescan for a two-file workspace. Whether that re-publish falls
inside the 200 ms window depends on load, so the test failed only under the full suite;
18 runs of `cargo test --test lsp`, 6 at a time, all passed.

- category: bug
- related_to: [[#qmd92_lsp_diag_flake]]
- affected_files: [qmdc-rs/tests/lsp.rs]
- solution: Read only the latest publish per URI.

## Fix the harness, not the server [[qmd92_finding_fix: Finding]]

In LSP each `publishDiagnostics` for a URI replaces the previous set, so the server's re-publish
after a rescan is correct, and a real client shows one diagnostic. The harness now does the
same: `collect_diagnostics` keeps the last notification per URI and then filters, simplifies
and sorts as before. Lengthening the wait would not help, because a later re-publish would
still be added up. The other LSP integration tests (`lsp_stale_*`, `lsp_shared_id_*`,
`lsp_namespaced_localid`) already read the latest notification. LSP tests are Rust-only, so no
other parser is affected.

- category: design
- related_to: [[#qmd92_lsp_diag_flake]]
- solution: Extract `collect_diagnostics` with latest-per-URI semantics and unit-test it.
