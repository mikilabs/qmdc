-- QMD-69 / operator decision 2026-09-23: because `workspace::id` ELIDES the namespace,
-- it is ambiguous when the named workspace holds that id in more than one namespace.
--
-- `elided_ambiguous` is [[#qmd69_repo_b::ledger]], and repo_b holds BOTH
-- qmd69_repo_b::ledger (workspace root) and qmd69_repo_b:services:ledger. Two candidates,
-- so the reference must report ambiguity and bind NOTHING — the same rule a bare
-- [[#ledger]] already follows inside a single workspace.
--
-- This query pins the GRAPH half (no edge). The diagnostic half — an
-- `ambiguous_reference` rather than silence — cannot be asserted from this fixture:
-- the workspace-conformance harness takes one workspace_id per fixture and so does not
-- run over a multi-workspace container. See QMD-69-findings.
--
-- Today the qualifier is discarded and the source object's own namespace is used instead,
-- so a single arbitrary edge is created with no diagnostic at all.
SELECT
  e.edge_type AS edge_type,
  t.__workspace AS target_workspace,
  t.__namespace AS target_namespace,
  t.__id AS target_id
FROM edges e
JOIN objects s ON s.__global_id = e.source_id
JOIN objects t ON t.__global_id = e.target_id
WHERE e.edge_type = 'elided_ambiguous'
ORDER BY target_workspace, target_namespace, target_id
