-- QMD-69 matrix, INTRA-workspace half: every id shape x every qualifier form, resolved from
-- an object in the SAME workspace and namespace as its targets.
--
-- Id shapes: a flat top-level id (`flat_store`), a hierarchical dotted id
-- (`system.data.postgres`), that object's `__local_id` (`postgres`), and a `.field` path on
-- each of the flat and the hierarchical object.
--
-- Qualifier forms, all four legal here because nothing crosses a workspace boundary:
-- bare, `ns:`, `ws:ns:`, and `ws::` (namespace elided).
--
-- 20 combinations, 20 edges. Two fail today, both on the `__local_id` path:
-- `leaf_wsns` and `leaf_wselided` build no edge, because the `__local_id` fallback in the
-- edge resolver is scoped to the SOURCE object's workspace and namespace and never consults
-- the reference's own qualifiers. Naming the workspace that actually holds the target makes
-- the reference WORSE than leaving it out, which is the shape of the defect.
SELECT
  e.edge_type AS edge_type,
  t.__id AS target_id,
  COALESCE(e.target_field, '') AS target_field
FROM edges e
JOIN objects s ON s.__global_id = e.source_id
JOIN objects t ON t.__global_id = e.target_id
WHERE s.__id = 'intra_consumer'
ORDER BY edge_type
