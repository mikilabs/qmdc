-- QMD-69 GUARD (passes today, must keep passing): a workspace-qualified reference whose id
-- is HIERARCHICAL resolves to that exact object.
--
-- `[[#qmd69_hier_provider:arch:system.data.postgres]]` — the colons separate the
-- workspace/namespace/id, the DOTS belong to the id and must not be split off. The parser
-- joins everything after the second colon back into the id, so a dotted id survives
-- qualification.
--
-- Nothing pinned this before: the corpus contained no reference carrying a qualifier and a
-- dotted id together, in any form.
SELECT
  e.edge_type AS edge_type,
  t.__workspace AS target_workspace,
  t.__namespace AS target_namespace,
  t.__id AS target_id
FROM edges e
JOIN objects s ON s.__global_id = e.source_id
JOIN objects t ON t.__global_id = e.target_id
WHERE e.edge_type = 'hier_full'
ORDER BY target_workspace, target_namespace, target_id
