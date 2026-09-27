# Alpha [[cli72_b2_alpha: __Workspace]]

## Paths [[cli72_b2_files: Query]]

Two workspaces composed by repeated `-w`, checked out at different depths and under
directory names that are not their ids. Under `-w` the base is virtual: each workspace sits
at its own id, so `__file` depends on the content alone and is the same wherever the
repositories are checked out.

Before QMD-72 fixed it, `__file` was relative to the common ancestor of the `-w` paths.
Here that would have been the case directory, giving `deep/checkout/alpha_dir/...`; for two
checkouts in unrelated places it degenerated to `/` and put the host's own directory names
into the graph.

- sql: SELECT __id, __file FROM objects ORDER BY __file, __id
