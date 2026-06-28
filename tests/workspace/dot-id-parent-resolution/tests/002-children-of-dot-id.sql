-- Test: children of dot-ID object get hierarchical IDs
SELECT __id, __kind, __parent
FROM objects
WHERE __id LIKE 'my_service.operations.%'
ORDER BY __id
