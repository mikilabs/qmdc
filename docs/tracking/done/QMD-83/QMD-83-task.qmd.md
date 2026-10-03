# QMD-83: the LSP did not reload .qmdcignore

## a .qmdcignore change never reached the running server [[qmd83_ignore_reload: Bug]]

GitHub [#6](https://github.com/mikilabs/qmdc/issues/6). Creating or editing `.qmdcignore` left the
ignored workspaces in the explorer until a server restart. Both event paths dropped it: the
extension watched only `**/*.qmd.md`, and the server's own fs watcher and its
`didChangeWatchedFiles` handler skipped every path not ending in `.qmd.md`.

- status: done
- priority: medium
- category: lsp
