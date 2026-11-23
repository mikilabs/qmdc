-- QMD-87 (GitHub #18): every reference the parser reports for an object is an edge.
-- References written in body text (stored in `__comments`) get source_field and
-- edge_type `__comments`; a field-path reference keeps its target_field; a reference
-- inside inline code is not a reference at all. A reference that appears both in a
-- field and in the body (`users` from `api`) is one edge per place. The workspace
-- root resolves its own child fields inside the workspace it names.
SELECT s.__id AS source, e.source_field, e.edge_type, t.__id AS target, e.target_field
FROM edges e
JOIN objects s ON s.__global_id = e.source_id
JOIN objects t ON t.__global_id = e.target_id
WHERE s.__kind NOT IN ('__Document', '__TextBlock')
ORDER BY source, e.source_field, target, e.target_field
