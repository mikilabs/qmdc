-- Test: Virtual workspace should load all objects
SELECT COUNT(*) as count FROM objects 
WHERE __kind NOT IN ('__Document', '__TextBlock', '__Workspace')

