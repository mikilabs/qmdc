# Visible [[cli73_ign_a: __Workspace]]

`-w ./container` names a directory holding two workspaces, `a/` and `b/`, and the
container's `.qmdcignore` hides `b/`. The `-w` path's own ignore file steers the search for
its workspace, so exactly one workspace is found and composed. Before QMD-73 only Rust read
the file there; Python and TypeScript refused with "contains 2 workspaces".

The assertion is a query, not a validation, so an answer that loads nothing cannot pass: the
row must name this workspace and only this one.

## Loaded workspaces [[cli73_loaded: Query]]

- sql: SELECT __id FROM objects WHERE __kind = '__Workspace' ORDER BY __id
