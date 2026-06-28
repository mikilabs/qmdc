-- Test that all objects have __workspace field pointing to virtual workspace
SELECT __id, __kind, __workspace
FROM objects 
WHERE __kind NOT IN ('__Workspace', '__Document', '__TextBlock')
ORDER BY __id

