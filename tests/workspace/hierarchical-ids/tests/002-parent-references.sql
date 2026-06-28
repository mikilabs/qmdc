-- Test: __parent column uses the parent's hierarchical ID
SELECT __id, __parent
FROM objects
WHERE __parent IS NOT NULL
ORDER BY __id
