-- QMD-69 GUARD: a reference to the workspace ROOT object must still resolve.
--
-- A `__Workspace` object carries no own `__workspace` field -- it IS the workspace -- so its
-- `__global_id` has an empty workspace segment (`::docs_ws`). Removing the unqualified
-- "any workspace" fallback made this reference unresolvable, because the same-workspace
-- lookup filters on `__workspace = <ws>` and the root object does not match it.
--
-- Nothing pinned this before: the fixture carries `about: [[#docs_ws]]` but no case asserted
-- the resulting edge, so the regression was silent. The resolver now admits an
-- empty-workspace object only when the looked-up id IS the workspace's own name, which keeps
-- this working without letting a bare id reach a sibling workspace's root object.
SELECT
  s.__id AS source_id,
  e.edge_type AS edge_type,
  t.__global_id AS target_global_id,
  t.__kind AS target_kind
FROM edges e
JOIN objects s ON s.__global_id = e.source_id
JOIN objects t ON t.__global_id = e.target_id
WHERE e.edge_type = 'about'
ORDER BY source_id
