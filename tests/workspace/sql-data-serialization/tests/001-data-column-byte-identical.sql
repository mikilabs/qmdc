-- Test: the `data` column is serialized byte-for-byte identically across all
-- three parsers. Canonical form: compact JSON (no spaces after : or ,), raw
-- UTF-8 (no \uXXXX escaping), and keys in document insertion order (NOT
-- alphabetical, NOT hash order). The `task` object declares fields in a
-- deliberately non-alphabetical order (zebra, alpha, count, ...) and includes
-- the `[[#file]][]` reference-with-array-suffix syntax.
SELECT __id, data
FROM objects
WHERE __kind = 'Entity'
ORDER BY __id
