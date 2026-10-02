# Project [[cli72_project: __Workspace]]

## Commerce [[commerce: Project]]

Two workspaces composed by repeated `-w`, neither of them the case directory, so the only
thing that can bind this reference is the composed set. Both paths are peers: the reference
lives in the second one and its target in the first, so an implementation that treated the
first `-w` as primary and the rest as satellites would still have to resolve across them.

The case directory is deliberately not a workspace. That keeps this fixture distinct from
`011-validate-cross-workspace`, which composes the same two-workspace shape through the
implicit positional `.` and already passes.

The assertion is a QUERY, not a validation. `workspace validate` returning `[]` is satisfied
by loading NOTHING — measured: sabotaging composition to keep only the first `-w` still left
the diagnostics empty, because the referring object was simply absent. Rows from BOTH
workspaces cannot be produced that way, so this is what actually pins C1.

- includes: [[#cli72_repo_a:services:payments_api]]

## Query [[cli72_edges: Query]]

- sql: SELECT source_id, target_id FROM edges WHERE edge_type = 'includes' ORDER BY source_id
