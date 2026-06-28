-- Test that virtual workspace creates __Workspace object
-- Virtual workspace should have __Workspace object with ID = folder name
SELECT __id, __kind, __file, __line 
FROM objects 
WHERE __kind = '__Workspace'
ORDER BY __id

