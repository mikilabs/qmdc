-- QMD-34: Test __global_id format with namespace (single colon)
-- Objects with namespace should have format workspace:namespace:id
SELECT __id, __workspace, __namespace, __global_id
FROM objects
WHERE __namespace = 'ns1'
ORDER BY __workspace, __id






