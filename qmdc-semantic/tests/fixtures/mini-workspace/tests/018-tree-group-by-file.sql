-- Test: Group objects by File
SELECT DISTINCT __file, COUNT(*) as object_count FROM objects 
WHERE __kind NOT IN ('__TextBlock', '__Workspace', '__Document', '__Namespace')
GROUP BY __file
ORDER BY __file
