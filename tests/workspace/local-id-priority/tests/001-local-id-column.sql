-- Test: __local_id column exists and is populated for hierarchical children.
-- Top-level objects have NULL __local_id; a child (service.config) carries its
-- short local id ("config"). All three parsers must expose this column.
SELECT __id, __local_id
FROM objects
WHERE __kind = '__Object'
ORDER BY __id
