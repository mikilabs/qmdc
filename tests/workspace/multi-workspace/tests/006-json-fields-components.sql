SELECT 
  __id,
  json_extract(data, '$.color') as color,
  json_extract(data, '$.size') as size
FROM objects 
WHERE __kind = 'Component'
ORDER BY __id

