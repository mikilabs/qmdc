-- QMD-34: Test cross-workspace edges
-- Edge from ws1:cross_ref to ws2:target_obj should exist
SELECT s.__workspace, s.__id, e.source_field, t.__workspace, t.__id
FROM edges e
JOIN objects s ON e.source_id = s.__global_id
JOIN objects t ON e.target_id = t.__global_id
WHERE s.__id = 'cross_ref' AND t.__id = 'target_obj'
ORDER BY s.__workspace, t.__workspace






