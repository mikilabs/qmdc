SELECT 
  __id,
  json_extract(data, '$.deliverables') as deliverables
FROM objects 
WHERE __kind = 'Capability' AND __id = 'cap_test'

