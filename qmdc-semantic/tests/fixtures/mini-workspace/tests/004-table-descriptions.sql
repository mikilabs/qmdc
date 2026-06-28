SELECT __id, json_extract(data, '$.description') as description 
FROM objects 
WHERE __kind = 'Table' 
ORDER BY __id
