SELECT __id, __kind, __level 
FROM objects 
WHERE __kind IN ('Table', 'Column', '__Namespace') 
  AND __file = 'data.qmd.md'
ORDER BY __level, __id

