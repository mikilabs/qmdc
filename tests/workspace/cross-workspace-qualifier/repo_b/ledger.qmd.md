## Ledger [[ledger: Service]]

QMD-69 / operator decision 2026-09-23. This object's id ALSO exists in repo_b's
`services` namespace, which is legal: ids are unique per namespace, and the workspace
root and `services` are different namespaces.

Because the empty middle segment of `[[#qmd69_repo_b::ledger]]` ELIDES the namespace
rather than asserting an empty one, that reference has two candidates in repo_b and must
report ambiguity — exactly as a bare `[[#ledger]]` already does within a single workspace.

- runtime: Python
- owner: repo_b
- where: repo_b workspace root
