## Negative Consumer [[negative_consumer: Project]]

QMD-69 matrix, NEGATIVE half: every way a qualifier can be wrong, for every id shape. None
of these may build an edge, and each must be reported by the validator.

Three ways to be wrong, applied across the id shapes:

- `bare` / `ns` — unqualified across a workspace boundary (decision 1)
- `bogusws` — names a workspace that does not exist
- `wrongws` — names `qmd69_shapes_third`, which EXISTS and even has a namespace called
  `arch`, but holds none of these objects. This is the case a namespace-only check cannot
  catch.
- `bogusns` — names a namespace that does not exist in the provider

- flat_bare: [[#flat_store]]
- flat_ns: [[#arch:flat_store]]
- flat_bogusws: [[#qmd69_no_such_ws:arch:flat_store]]
- flat_wrongws: [[#qmd69_shapes_third:arch:flat_store]]
- flat_bogusns: [[#qmd69_shapes_provider:no_such_ns:flat_store]]
- hier_bare: [[#system.data.postgres]]
- hier_bogusws: [[#qmd69_no_such_ws:arch:system.data.postgres]]
- hier_wrongws: [[#qmd69_shapes_third:arch:system.data.postgres]]
- hier_bogusns: [[#qmd69_shapes_provider:no_such_ns:system.data.postgres]]
- leaf_bare: [[#postgres]]
- leaf_bogusws: [[#qmd69_no_such_ws:arch:postgres]]
- leaf_wrongws: [[#qmd69_shapes_third:arch:postgres]]
- leaf_bogusns: [[#qmd69_shapes_provider:no_such_ns:postgres]]
- hfield_bogusws: [[#qmd69_no_such_ws:arch:system.data.postgres.engine]]
- hfield_wrongws: [[#qmd69_shapes_third:arch:system.data.postgres.engine]]
