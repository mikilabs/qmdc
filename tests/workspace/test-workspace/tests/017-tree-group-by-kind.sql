-- Test: Group objects by Kind
SELECT DISTINCT __kind FROM objects 
WHERE __kind NOT IN ('__TextBlock', '__Workspace', '__Document', '__Namespace')
ORDER BY __kind
