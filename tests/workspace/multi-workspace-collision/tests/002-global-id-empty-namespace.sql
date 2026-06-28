-- QMD-34: Test __global_id format with empty namespace (double colon)
-- Objects without namespace should have format workspace::id
SELECT __id, __workspace, __namespace, __global_id
FROM objects
WHERE __id IN ('task_workflow', 'extra_obj')
  AND (__namespace = '' OR __namespace IS NULL)
ORDER BY __workspace, __id

