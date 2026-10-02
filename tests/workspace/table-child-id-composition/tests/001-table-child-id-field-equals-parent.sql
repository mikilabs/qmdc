-- QMD-70 goal B2: how a table child's hierarchical id is composed.
--
-- `## Items [[items: [Item]]]` is a TOP-LEVEL object array: the heading owns the array, so the
-- array field's name is also the parent object's own id. The field-name segment must not be
-- repeated — `items.items_0`, not `items.items.items_0`.
--
-- Rust composed `{parent}.{field}.{local}` unconditionally and produced the doubled form, while
-- Python's `resolve_child_id` documents and implements the special case and TypeScript matches
-- it. Nothing pinned the disagreement, because no fixture used an array whose field name equals
-- its parent's id.
--
-- Asserted through the SQL harness rather than a parser microtest on purpose: the microtest
-- harness also checks `parse -> rebuild`, and this shape is not round-trip stable — rebuild
-- emits a wrapper heading plus a nested array heading. That is a separate pre-existing rebuild
-- limitation, unrelated to id composition, and pinning it here keeps the two concerns apart.
SELECT
  __id AS id,
  __local_id AS local_id
FROM objects
WHERE __kind = 'Item'
ORDER BY id
