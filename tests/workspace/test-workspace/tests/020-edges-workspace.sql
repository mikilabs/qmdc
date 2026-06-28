-- Test: edges table should have __workspace column populated
-- Use JOIN to get __id from __global_id (edges.source_id/target_id contain __global_id)
SELECT s.__id as source_id, t.__id as target_id, e.__workspace
FROM edges e
JOIN objects s ON e.source_id = s.__global_id
JOIN objects t ON e.target_id = t.__global_id
ORDER BY s.__id, t.__id
LIMIT 10

