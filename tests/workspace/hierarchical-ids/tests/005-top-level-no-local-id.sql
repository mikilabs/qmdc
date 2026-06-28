-- Test: top-level objects have no __parent
SELECT __id, __parent
FROM objects
WHERE __parent IS NULL
  AND __kind NOT IN ('__Workspace')
ORDER BY __id
