SELECT 
  __id,
  json_extract(data, '$.columns') as columns,
  json_extract(data, '$.primary_key') as primary_key
FROM objects 
WHERE __kind = 'Table'
ORDER BY __id

