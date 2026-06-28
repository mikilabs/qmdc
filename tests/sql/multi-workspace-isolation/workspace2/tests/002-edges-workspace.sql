-- Test: edges table should have __workspace column populated
SELECT source_id, target_id, __workspace
FROM edges
ORDER BY source_id, target_id

