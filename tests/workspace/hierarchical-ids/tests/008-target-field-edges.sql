-- Test: field-level references populate target_field column
SELECT s.__id as source_id, e.source_field, t.__id as target_id, e.target_field
FROM edges e
JOIN objects s ON e.source_id = s.__global_id
JOIN objects t ON e.target_id = t.__global_id
WHERE e.target_field != ''
ORDER BY s.__id, e.source_field
