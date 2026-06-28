-- Test: can query children by ID prefix (find all objects under auth_svc)
SELECT __id, __kind
FROM objects
WHERE __id LIKE 'auth_svc.%'
ORDER BY __id
