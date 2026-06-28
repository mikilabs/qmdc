-- Test: group edges by edge_type to verify type distribution
SELECT e.edge_type, COUNT(*) as count
FROM edges e
JOIN objects s ON e.source_id = s.__global_id
WHERE s.__id NOT LIKE 'doc_%' AND s.__id NOT LIKE 'text_%'
GROUP BY e.edge_type
ORDER BY e.edge_type
