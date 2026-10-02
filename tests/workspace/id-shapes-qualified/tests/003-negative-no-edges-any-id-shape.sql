-- QMD-69 matrix, NEGATIVE half (GUARD — passes today, must keep passing): not one of the
-- fifteen wrong references may build an edge, whatever the id shape.
--
-- Four ways to be wrong, crossed with the id shapes:
--   * bare / `ns:`  -- unqualified across a workspace boundary (operator decision 1)
--   * `bogusws`     -- names a workspace that does not exist
--   * `wrongws`     -- names `qmd69_shapes_third`, which EXISTS and even has a namespace
--                      called `arch`, but holds none of these objects. A namespace-only
--                      check cannot catch this one, which is why the fixture gives the third
--                      workspace the same namespace NAME on purpose.
--   * `bogusns`     -- names a namespace the provider does not have
--
-- The graph is already correct on all fifteen. The VALIDATOR is not: eight of them are
-- accepted silently today, which is asserted by `tests/cli/014-validate-id-shapes`. This case
-- is what keeps the graph half from regressing while that is fixed.
SELECT
  e.edge_type AS edge_type,
  t.__global_id AS target_global_id
FROM edges e
JOIN objects s ON s.__global_id = e.source_id
JOIN objects t ON t.__global_id = e.target_id
WHERE s.__id = 'negative_consumer'
ORDER BY edge_type
