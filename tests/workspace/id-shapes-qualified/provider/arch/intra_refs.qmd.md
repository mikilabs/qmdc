## Intra Consumer [[intra_consumer: Project]]

QMD-69 matrix, INTRA-workspace half. This object sits in the same workspace and namespace as
every target, so all four qualifier forms are legal here — including the bare and
namespace-only ones, which are illegal across a workspace boundary.

Edge names encode `<idshape>_<qualifier>`:

- `flat` — a top-level id (`flat_store`)
- `hier` — a hierarchical dotted id (`system.data.postgres`)
- `leaf` — that object's `__local_id` (`postgres`)
- `ffield` / `hfield` — a `.field` path on the flat and on the hierarchical object

- flat_bare: [[#flat_store]]
- flat_ns: [[#arch:flat_store]]
- flat_wsns: [[#qmd69_shapes_provider:arch:flat_store]]
- flat_wselided: [[#qmd69_shapes_provider::flat_store]]
- hier_bare: [[#system.data.postgres]]
- hier_ns: [[#arch:system.data.postgres]]
- hier_wsns: [[#qmd69_shapes_provider:arch:system.data.postgres]]
- hier_wselided: [[#qmd69_shapes_provider::system.data.postgres]]
- leaf_bare: [[#postgres]]
- leaf_ns: [[#arch:postgres]]
- leaf_wsns: [[#qmd69_shapes_provider:arch:postgres]]
- leaf_wselided: [[#qmd69_shapes_provider::postgres]]
- ffield_bare: [[#flat_store.engine]]
- ffield_ns: [[#arch:flat_store.engine]]
- ffield_wsns: [[#qmd69_shapes_provider:arch:flat_store.engine]]
- ffield_wselided: [[#qmd69_shapes_provider::flat_store.engine]]
- hfield_bare: [[#system.data.postgres.engine]]
- hfield_ns: [[#arch:system.data.postgres.engine]]
- hfield_wsns: [[#qmd69_shapes_provider:arch:system.data.postgres.engine]]
- hfield_wselided: [[#qmd69_shapes_provider::system.data.postgres.engine]]
