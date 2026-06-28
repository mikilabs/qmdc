SELECT __namespace, __kind, count(*) as cnt
FROM objects 
WHERE __kind NOT IN ('__Document', '__TextBlock', '__Workspace', '__Namespace')
GROUP BY __namespace, __kind
ORDER BY __namespace, cnt DESC

