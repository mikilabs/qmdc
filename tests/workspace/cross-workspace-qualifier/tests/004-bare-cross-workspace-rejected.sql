-- QMD-69 / operator decision 2026-09-23: a cross-workspace reference MUST be
-- qualified. A reference that names no workspace resolves only inside its own
-- workspace, so neither of the two bare references from qmd69_project may produce
-- an edge:
--   bare_unique     -> [[#billing]]       target held by exactly ONE sibling workspace
--   bare_duplicated -> [[#payments_api]]  target held by TWO sibling workspaces
-- The unique case is the sharp edge of the decision: the fallback is removed
-- outright rather than kept for unambiguous ids, so "only one workspace has it" is
-- explicitly NOT a reason to resolve. Today the third-try fallback binds both.
SELECT
  s.__workspace AS source_workspace,
  e.edge_type AS edge_type,
  t.__workspace AS target_workspace,
  t.__id AS target_id
FROM edges e
JOIN objects s ON s.__global_id = e.source_id
JOIN objects t ON t.__global_id = e.target_id
WHERE e.edge_type IN ('bare_unique', 'bare_duplicated')
ORDER BY edge_type, target_workspace, target_id
