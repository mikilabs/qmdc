# Consumer [[cli72_c3_consumer: __Workspace]]

## App [[app: Service]]

Issue 9 acceptance test 3: the same local id in two composed workspaces. `config` exists in
both `repo_a` and `repo_b`, and identity is workspace-scoped (QMD-67), so they are two
objects and never one.

Each qualified reference reaches its own workspace's object. The bare reference reaches
NEITHER: a reference without a qualifier is workspace-local, so from a third workspace it
cannot cross into either — `broken_link`, not `ambiguous_reference`. That is the whole
invariant this case pins, and it is why a duplicate local id across workspaces needs no
ambiguity rule.

- uses_a: [[#cli72_c3_repo_a::config]]
- uses_b: [[#cli72_c3_repo_b::config]]
- uses_bare: [[#config]]
