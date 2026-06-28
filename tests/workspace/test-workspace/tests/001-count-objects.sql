SELECT COUNT(*) as count FROM objects 
WHERE __kind NOT IN ('__Document', '__TextBlock', '__Namespace', '__Workspace')