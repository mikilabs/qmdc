-- Test: Check that valid workspaces are parsed
SELECT __id, __kind, __label, __file
FROM objects 
WHERE __kind = '__Workspace'
ORDER BY __id;

