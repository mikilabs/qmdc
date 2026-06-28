-- Test: full `SELECT *` from the edges table is byte-identical across all three
-- parsers. Notably the `contains` edge comes from the `[[#file]][]` reference,
-- which must be parsed as a single reference (not a mangled array) so the edge
-- is created — this previously diverged (Rust dropped it).
SELECT *
FROM edges
ORDER BY source_id, source_field, target_id, edge_type, target_field
