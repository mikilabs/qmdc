-- QMD-69 GUARD (passes today, must keep passing): a `.field` path on a qualified
-- HIERARCHICAL id.
--
-- `[[#qmd69_hier_provider:arch:system.data.postgres.engine]]` has four dot-separated
-- segments after the qualifiers. Only the LAST one is the field; the rest are the object's
-- hierarchical id. The edge must therefore land on `system.data.postgres` and carry
-- `target_field = engine`, not on `system.data` with a field of `postgres`.
--
-- This is the case most at risk from a naive fix: splitting the id on the FIRST dot, or
-- splitting the whole target before the qualifiers are removed, both produce a wrong edge
-- here rather than no edge.
SELECT
  e.edge_type AS edge_type,
  t.__id AS target_id,
  e.target_field AS target_field
FROM edges e
JOIN objects s ON s.__global_id = e.source_id
JOIN objects t ON t.__global_id = e.target_id
WHERE e.edge_type = 'hier_field'
ORDER BY target_id
