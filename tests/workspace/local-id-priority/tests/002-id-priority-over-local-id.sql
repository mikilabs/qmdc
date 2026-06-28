-- Test: a reference [[#config]] resolves to the top-level object whose __id is
-- "config" (id-match priority), NOT to the child "service.config" that merely
-- has __local_id = "config". Reference resolution must try __id before the
-- __local_id fallback.
SELECT s.__id AS source_id, e.source_field, t.__id AS target_id
FROM edges e
JOIN objects s ON e.source_id = s.__global_id
JOIN objects t ON e.target_id = t.__global_id
WHERE s.__id = 'consumer'
