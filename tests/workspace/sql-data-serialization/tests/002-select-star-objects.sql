-- Test: full `SELECT *` from the objects table is byte-identical across all
-- three parsers (Rust, Python, TypeScript). This locks the complete object
-- schema AND the `data` column serialization (compact JSON, raw UTF-8, document
-- insertion order, preserved float literals like 1.0, preserved [[#ref]][]
-- syntax, and verbatim markdown tables in text fields).
SELECT *
FROM objects
ORDER BY __global_id, __kind
