## Cross Consumer [[cross_consumer: Project]]

QMD-69 matrix, CROSS-workspace half. Every target lives in the provider workspace, so only
the workspace-qualified forms may resolve. The bare and namespace-only forms are the
NEGATIVE half of operator decision 1: a cross-workspace reference must name its workspace,
so they must report `broken_link` and build no edge.

Edge names encode `<idshape>_<qualifier>`, matching the intra-workspace object.

- flat_wsns: [[#qmd69_shapes_provider:arch:flat_store]]
- flat_wselided: [[#qmd69_shapes_provider::flat_store]]
- hier_wsns: [[#qmd69_shapes_provider:arch:system.data.postgres]]
- hier_wselided: [[#qmd69_shapes_provider::system.data.postgres]]
- leaf_wsns: [[#qmd69_shapes_provider:arch:postgres]]
- leaf_wselided: [[#qmd69_shapes_provider::postgres]]
- ffield_wsns: [[#qmd69_shapes_provider:arch:flat_store.engine]]
- hfield_wsns: [[#qmd69_shapes_provider:arch:system.data.postgres.engine]]
