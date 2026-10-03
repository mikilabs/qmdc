# QMD-83: Result

## any .qmdcignore change rescans [[qmd83_result: Result]]

A change to a `.qmdcignore` at any depth is structural: both the native watcher and the client
notification now trigger the full rescan, which re-reads the ignore rules and sends
`qmdc/workspaceUpdated`, on which the explorer already drops its cached tree. The extension also
watches `**/.qmdcignore`.

`lsp_qmdcignore_reload.rs`: the client-notification path (create, then edit back) and the native
watcher with no client event; both red on 2.0.0 (`fixture_ws` still listed). A unit test pins the
matcher: nested and Windows paths match, `foo.qmdcignore` and `.qmdcignore.bak` do not (review note
F7). SOP review `reviews/10-cr-gh6-7-10-11.md`: APPROVE.

- feature: [[#qmd83_ignore_reload]]
- completed: 2026-10-03
- files_changed: [qmdc-rs/src/lsp/server.rs, qmdc-vscode/src/extension.ts, CHANGELOG.md]
- tests_changed: [qmdc-rs/tests/lsp_qmdcignore_reload.rs]
