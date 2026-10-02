-- QMD-69 / operator decision 2026-09-23: in `workspace::id` the empty middle segment ELIDES
-- the namespace, it does not assert an empty one. So the reference matches the id in ANY
-- namespace of the named workspace.
--
-- `elided_ns` is [[#qmd69_elision_provider::gateway]] and its target sits in the provider's
-- `services` namespace, nothing named `gateway` being at the provider's root. Under the
-- elision reading the reference must reach it; under the rejected "assertion" reading it would
-- be a broken link. That is what makes this case the sharp test of the decision.
--
-- This fixture exists SEPARATELY from cross-workspace-qualifier for one reason: the failure
-- must be deterministic, not lucky. The consumer workspace declares its OWN root-level
-- `gateway`, and edge resolution starts from the source object's own workspace and namespace,
-- so today every implementation matches that local object on the first try and never consults
-- the qualifier. No arbitrary `LIMIT 1` ordering is involved. Keeping the shadow out of
-- cross-workspace-qualifier matters because the Python validator CRASHES on this shape
-- (see QMD-69-findings, the Python crash Finding), and a crash fails every case in whatever
-- fixture it lives in — including the root-namespace GUARD, which would then guard nothing.
SELECT
  e.edge_type AS edge_type,
  t.__workspace AS target_workspace,
  t.__namespace AS target_namespace,
  t.__id AS target_id
FROM edges e
JOIN objects s ON s.__global_id = e.source_id
JOIN objects t ON t.__global_id = e.target_id
WHERE e.edge_type = 'elided_ns'
ORDER BY target_workspace, target_namespace, target_id
