# QMD-82: explorer items in a second editor folder opened a nonexistent file

## getWorkspaceTree lost projectRoot for a duplicated workspace id [[qmd82_project_root: Bug]]

GitHub [#7](https://github.com/mikilabs/qmdc/issues/7). In a multi-root window, clicking an object of
a workspace from the second folder opened nothing. The LSP enriched each tree entry with
`projectRoot` through `WorkspaceIndex::get_by_id`, which returns nothing when the id is declared in
several places — the reporting project held copies of one experiment under `.tmp/`. The extension
then fell back to the first editor folder and resolved the relative `file` against it.

- status: done
- priority: medium
- category: lsp
