-- QMD-70 goal B2, the system-container branch of the same rule.
--
-- When the array's parent is a system container (`__Workspace` / `__Namespace`) the child takes
-- the BARE local id, with no parent prefix and no `__local_id`. Python's `resolve_child_id`
-- documents and implements this, TypeScript matches it, and Rust's shared `resolve_child_id`
-- helper does too — but Rust's table-child path used to re-implement the rule inline and its
-- copy prefixed the parent id, producing `rows_ns_rows_0` where the other two produced `rows_0`.
--
-- Nothing pinned it: no fixture put a table-fed array directly under a namespace root, and the
-- cross-parser comparison harness only compares validation errors, not ids. Rust's table-child
-- path now calls the shared helper, so there is a single rule for both branches.
SELECT
  __id AS id,
  __kind AS kind,
  __local_id AS local_id
FROM objects
WHERE __kind = 'Row'
ORDER BY id
