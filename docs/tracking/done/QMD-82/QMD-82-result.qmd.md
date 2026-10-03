# QMD-82: Result

## projectRoot matched by id and file; no folder-0 fallback [[qmd82_result: Result]]

`WorkspaceIndex::get_for_entry(id, file)` picks the workspace with that id holding a file whose
`project_root`-relative, slash-separated path is the entry's `file`, falling back to `get_by_id`; an
entry still without a root is logged by the server. The extension skips such an entry instead of
guessing the first folder, as the issue asked.

`lsp_tree_project_root.rs`: two editor folders with nested workspaces, all four grouping modes, plus
the duplicate-id case. Only the duplicate-id test was red on 2.0.0 (`entry without projectRoot`);
singly-declared ids already got a root, so the other test is a guard. The client change is compiled
and linted, not run in a VS Code host. SOP review `reviews/10-cr-gh6-7-10-11.md`: APPROVE.

- feature: [[#qmd82_project_root]]
- completed: 2026-10-03
- files_changed: [qmdc-rs/src/lsp/workspace.rs, qmdc-rs/src/lsp/server.rs, qmdc-vscode/src/qmdcTreeProvider.ts, CHANGELOG.md]
- tests_changed: [qmdc-rs/tests/lsp_tree_project_root.rs]
