SELECT __id, __label
FROM objects 
WHERE __kind = 'Component' 
  AND json_extract(data, '$.size') = 'medium'
ORDER BY __id

