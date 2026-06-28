-- Test: objects with both inline fields and text field preambles
-- Storage objects have inline database refs AND text field preambles
SELECT s.__id as source_id, e.source_field, t.__id as target_id, e.edge_type
FROM edges e
JOIN objects s ON e.source_id = s.__global_id
JOIN objects t ON e.target_id = t.__global_id
WHERE s.__kind IN ('Database', 'Table')
ORDER BY s.__id, e.source_field, t.__id, e.edge_type
