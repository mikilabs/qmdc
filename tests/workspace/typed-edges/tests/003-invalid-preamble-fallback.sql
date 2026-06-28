-- Test: invalid preamble falls back to edge_type = source_field
-- Both no_preamble and invalid_preamble should have edge_type = source_field for text field refs
SELECT s.__id as source_id, e.source_field, t.__id as target_id, e.edge_type
FROM edges e
JOIN objects s ON e.source_id = s.__global_id
JOIN objects t ON e.target_id = t.__global_id
WHERE s.__kind = 'Case'
ORDER BY s.__id, e.source_field, t.__id
