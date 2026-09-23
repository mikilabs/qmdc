-- QMD-69 matrix, CROSS-workspace half: every id shape reached from ANOTHER workspace, in the
-- only two qualifier forms that are legal across a boundary — `ws:ns:` and `ws::`.
--
-- The bare and namespace-only forms are illegal here by operator decision 1 and live in the
-- negative case (003), so this case is purely about the qualified forms carrying each id
-- shape correctly over the boundary.
--
-- 8 combinations, 8 edges. Two fail today, the same `__local_id` pair as in the intra case:
-- `leaf_wsns` and `leaf_wselided`.
SELECT
  e.edge_type AS edge_type,
  t.__workspace AS target_workspace,
  t.__id AS target_id,
  COALESCE(e.target_field, '') AS target_field
FROM edges e
JOIN objects s ON s.__global_id = e.source_id
JOIN objects t ON t.__global_id = e.target_id
WHERE s.__id = 'cross_consumer'
ORDER BY edge_type
