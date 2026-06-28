SELECT __workspace, __kind, count(*) as cnt 
FROM objects 
WHERE __kind NOT IN ('__Document', '__TextBlock')
GROUP BY __workspace, __kind 
ORDER BY cnt DESC, __workspace, __kind

