-- QMD-90 (GitHub #23): a file with CRLF line endings gives the same edges as with LF.
-- `- about: [[#api]]` followed by a blank line and prose is a preamble, so its edge type
-- is `about`, not the field name `overview`.
SELECT s.__id AS source, e.source_field, e.edge_type, t.__id AS target
FROM edges e
JOIN objects s ON s.__global_id = e.source_id
JOIN objects t ON t.__global_id = e.target_id
WHERE s.__kind NOT IN ('__Document', '__TextBlock', '__Workspace')
ORDER BY source, e.source_field, target
