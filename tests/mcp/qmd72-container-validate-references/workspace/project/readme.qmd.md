# Project [[qmd72_project: __Workspace]]

- description: QMD-72 — consumer workspace, sibling of repo_a under a non-workspace container

## Commerce [[commerce: Project]]

`includes` names repo_a's workspace and namespace explicitly, so it is the canonical
`workspace:namespace:id` form and carries no Kind segment. The CLI already resolves it:
`tests/cli/011-validate-cross-workspace` pins `workspace validate` on a container like this
one returning `[]`, and `tests/workspace/cross-workspace-qualifier` pins the query side.

This fixture drives the one surface that does not compose. MCP resolves a path to a SINGLE
workspace root (`core/index_seam.rs::resolve_root`), so a container holding two siblings is
refused with `ambiguous` before any reference is examined — issue #9's fourth acceptance
test, the only one QMD-69 left open.

- includes: [[#qmd72_repo_a:services:payments_api]]
