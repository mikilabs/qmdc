-- Test: child objects have hierarchical dot-IDs (Endpoint kind only)
SELECT __id, __kind
FROM objects
WHERE __kind NOT IN ('__Workspace', '__Object')
ORDER BY __id
