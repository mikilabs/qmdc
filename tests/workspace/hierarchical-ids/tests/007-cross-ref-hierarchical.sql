-- Test: cross-reference to hierarchical ID creates proper edge
SELECT s.__id as source_id, e.source_field, t.__id as target_id
FROM edges e
JOIN objects s ON e.source_id = s.__global_id
JOIN objects t ON e.target_id = t.__global_id
WHERE s.__id = 'payment_svc'
  AND e.source_field = 'auth'
