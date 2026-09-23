-- QMD-69: GUARD (passes today, must keep passing). A cross-workspace reference whose
-- target sits in the other workspace's ROOT namespace resolves to that workspace. `billing`
-- is unique across the container, so today's bare-id fallback happens to land correctly —
-- the point of this case is that removing that fallback (question 1, answered: removed) must
-- not break it.
--
-- Per the operator decision of 2026-09-23 the rule that must keep this green is ELISION:
-- `workspace::id` searches every namespace of the named workspace, and repo_a holds exactly
-- one `billing`. Case 005 is the same rule with a target inside a namespace instead of the
-- root, and case 006 is the same rule when it finds two candidates.
SELECT
  e.edge_type AS edge_type,
  t.__workspace AS target_workspace,
  t.__namespace AS target_namespace,
  t.__id AS target_id
FROM edges e
JOIN objects s ON s.__global_id = e.source_id
JOIN objects t ON t.__global_id = e.target_id
WHERE e.edge_type = 'root_ns'
ORDER BY target_workspace, target_id
