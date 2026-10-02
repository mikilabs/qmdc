-- QMD-69 REPRODUCTION (fails today): a qualified reference to an object's `__local_id`
-- builds no edge, while `qmdc workspace validate` reports no problem with it.
--
-- `[[#qmd69_hier_provider:arch:postgres]]` names the workspace and namespace that hold
-- `system.data.postgres`, and `postgres` is that object's `__local_id`. The short-form
-- `__local_id` fallback is what makes `[[#postgres]]` work inside its own namespace, so the
-- same reference with the target's workspace and namespace spelled out must resolve too.
--
-- Today it does not, and the two surfaces disagree about it — the exact split this task
-- exists to remove:
--
--   * the GRAPH builds no edge, because the `__local_id` fallback in the edge resolver is
--     scoped to the SOURCE object's workspace and namespace and never looks at the
--     reference's own qualifiers;
--   * the VALIDATOR stays silent, because its `__local_id` index carries no workspace at all,
--     so it matches the leaf in the provider while ignoring which workspace was named.
--
-- This case pins the graph half. The validator half is pinned by
-- `tests/cli/013-validate-qualified-local-id-unknown-workspace`, which shows the same index
-- gap accepting a workspace that does not exist.
SELECT
  e.edge_type AS edge_type,
  t.__workspace AS target_workspace,
  t.__namespace AS target_namespace,
  t.__id AS target_id
FROM edges e
JOIN objects s ON s.__global_id = e.source_id
JOIN objects t ON t.__global_id = e.target_id
WHERE e.edge_type = 'hier_leaf'
ORDER BY target_workspace, target_namespace, target_id
