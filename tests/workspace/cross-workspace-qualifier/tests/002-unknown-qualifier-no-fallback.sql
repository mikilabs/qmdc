-- QMD-69: a qualifier naming a workspace that does not exist must not resolve.
-- `bogus` names qmd69_no_such_ws. No workspace by that name is present, so the
-- reference is unresolvable and must produce no edge. Today the qualifier is
-- discarded and the bare id still matches an object in another workspace, so a
-- silently wrong edge is created.
SELECT
  s.__workspace AS source_workspace,
  e.edge_type AS edge_type,
  t.__workspace AS target_workspace,
  t.__id AS target_id
FROM edges e
JOIN objects s ON s.__global_id = e.source_id
JOIN objects t ON t.__global_id = e.target_id
WHERE e.edge_type = 'bogus'
ORDER BY target_workspace, target_id
