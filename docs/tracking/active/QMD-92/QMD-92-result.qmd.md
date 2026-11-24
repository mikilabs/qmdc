# QMD-92: Result

## LSP diagnostics microtests read the latest publish [[qmd92_result: Result]]

The diagnostics collection in the Rust LSP microtest harness moves into `collect_diagnostics`,
which keeps only the latest `publishDiagnostics` per URI, as an LSP client does. A second
publish after an fs-watcher rescan no longer duplicates diagnostics. No server change.

- feature: [[#qmd92_lsp_diag_flake]]
- completed: 2026-11-24
- files_changed: [qmdc-rs/tests/lsp.rs]
- tests_changed: [qmdc-rs/tests/lsp.rs]

### Verification [[qmd92_result_verification: text]]

- about: [[#qmd92_result]]

- `collect_diagnostics_keeps_latest_publish_per_uri` fails on the old concatenating
  collection (two diagnostics instead of one) and passes after the fix.
- `cargo test --test lsp`: both tests pass, all LSP microtests included.
- `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` exit 0.
- `make test-fast`, `make validate-docs` and `make md-lint` exit 0.
- A timing reproduction of the race was not reliable (0 failures in 18 parallel runs), so the
  regression test checks the collection logic directly.
- Code review: self-review only.
