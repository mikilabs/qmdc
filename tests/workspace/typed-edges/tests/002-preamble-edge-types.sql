-- Test: text field preambles create typed edges where edge_type != source_field
SELECT s.__id as source_id, e.source_field, t.__id as target_id, e.edge_type
FROM edges e
JOIN objects s ON e.source_id = s.__global_id
JOIN objects t ON e.target_id = t.__global_id
WHERE e.edge_type != e.source_field
ORDER BY s.__id, e.source_field, t.__id, e.edge_type
