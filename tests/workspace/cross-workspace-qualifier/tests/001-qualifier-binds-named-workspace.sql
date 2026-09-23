-- QMD-69: a workspace-qualified reference must bind inside the named workspace.
-- `includes` names qmd69_repo_a explicitly, and an identically-named object exists in
-- qmd69_repo_b, so the qualifier is the only thing that can disambiguate the two.
-- Today the qualifier is discarded and the edge lands on whichever workspace the
-- unqualified `WHERE __id = ?1 LIMIT 1` lookup happens to return.
SELECT
  s.__workspace AS source_workspace,
  e.edge_type AS edge_type,
  t.__workspace AS target_workspace,
  t.__namespace AS target_namespace,
  t.__id AS target_id
FROM edges e
JOIN objects s ON s.__global_id = e.source_id
JOIN objects t ON t.__global_id = e.target_id
WHERE e.edge_type = 'includes'
ORDER BY target_workspace, target_id
