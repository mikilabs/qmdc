## Commerce [[commerce: Project]]

QMD-69 REPRODUCTION: a qualifier naming a workspace that does not exist must be reported as
a broken link — including on the `__local_id` path.

`postgres` is the `__local_id` of `cli_unknown_provider:arch:system.data.postgres`, and
`no_such_ws` is not a workspace in this container. The validator accepts it today anyway,
because its `__local_id` index carries no workspace and the filter only looks at the
namespace, so the leaf is matched in the provider while the named workspace is ignored. That
is the one place where the workspace qualifier is still unenforced.

- leaf_bogus_ws: [[#no_such_ws:arch:postgres]]
