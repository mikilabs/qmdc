-- Test: dot-ID object finds parent in same file
SELECT __id, __parent
FROM objects
WHERE __id LIKE 'svc.%'
ORDER BY __id
