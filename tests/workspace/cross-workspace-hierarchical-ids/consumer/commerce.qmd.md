## Commerce [[commerce: Project]]

QMD-69: a workspace qualifier combined with a HIERARCHICAL (dotted) id. The corpus had no
reference that carried a qualifier and a dotted id together, in any form, so none of these
paths were pinned before.

Four shapes, all naming the same target `system.data.postgres` in the provider's `arch`
namespace, or a field on it:

- `hier_full` — the fully qualified hierarchical id.
- `hier_elided` — same target with the namespace elided (`ws::id`).
- `hier_field` — a `.field` path on that hierarchical object; the LAST dot separates the
  field, the earlier ones belong to the id.
- `hier_leaf` — the object's `__local_id` (`postgres`) rather than its full dotted id,
  qualified with the workspace and namespace that hold it.

- hier_full: [[#qmd69_hier_provider:arch:system.data.postgres]]
- hier_elided: [[#qmd69_hier_provider::system.data.postgres]]
- hier_field: [[#qmd69_hier_provider:arch:system.data.postgres.engine]]
- hier_leaf: [[#qmd69_hier_provider:arch:postgres]]
