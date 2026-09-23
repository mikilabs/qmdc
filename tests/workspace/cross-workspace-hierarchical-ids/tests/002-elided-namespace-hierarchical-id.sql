-- QMD-69 GUARD (passes today, must keep passing): the ELIDED form combined with a
-- HIERARCHICAL id.
--
-- `[[#qmd69_hier_provider::system.data.postgres]]` — the empty middle segment elides the
-- namespace, so the target is matched in any namespace of the named workspace, and the
-- dotted id is still taken whole. This is the intersection of two rules that were each
-- pinned separately but never together.
SELECT
  e.edge_type AS edge_type,
  t.__workspace AS target_workspace,
  t.__namespace AS target_namespace,
  t.__id AS target_id
FROM edges e
JOIN objects s ON s.__global_id = e.source_id
JOIN objects t ON t.__global_id = e.target_id
WHERE e.edge_type = 'hier_elided'
ORDER BY target_workspace, target_namespace, target_id
