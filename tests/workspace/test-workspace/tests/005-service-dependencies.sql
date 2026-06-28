SELECT o.__id as service, t.__id as depends_on
FROM objects o
JOIN edges e ON o.__global_id = e.source_id
JOIN objects t ON e.target_id = t.__global_id
WHERE o.__kind = 'Service'
ORDER BY o.__id, t.__id
