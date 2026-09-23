# Project [[cli_project: __Workspace]]

## Commerce [[commerce: Project]]

`includes` names repo_a's namespace explicitly. `root_ns` and `elided_ns` both use the
`workspace::id` form, where the empty middle segment ELIDES the namespace rather than
asserting an empty one (operator decision, 2026-09-23): `root_ns` reaches an object in
repo_a's workspace ROOT, `elided_ns` reaches one inside repo_a's `services` NAMESPACE, and
all three must be clean. Under the rejected "assertion" reading `elided_ns` would be a
broken link, so it is what makes this fixture discriminate between the two readings.

No reference carries a Kind segment: the canonical form is `workspace:namespace:id` with an
optional `.field` suffix, matching `__global_id`.

- includes: [[#cli_repo_a:services:payments_api]]
- root_ns: [[#cli_repo_a::billing]]
- elided_ns: [[#cli_repo_a::payments_api]]
