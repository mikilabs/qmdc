# QMD-84: Result

## one composed index behind every MCP call [[qmd84_result: Result]]

`qmdc mcp -w A -w B` checks the set at startup with the CLI's rules plus the force-root (each `-w`
path and each workspace it resolves to) and exits 2 on refusal. Tools and resources now share one
entry, `index_for_path`: with `-w` it builds the composed index and requires the call's `path` to lie
inside one of its workspaces (`out-of-root` otherwise). A composed index has a virtual base, so the
three ops that read `index.root` changed: rename reads lines through `ResolvedIndex::disk_path` (the
`WorkspaceEntry` longest-prefix rule), a validate `file` scope must be one of the composed `__file`
values, and the dump reports `root: null`.

`mcp_compose_with.rs`: validate, query from either member, locate, rename (the definition edit carries
`[[y: Thing]]`, which only a disk read yields), scoped validate, dump, a stray path refused, and four
startup refusals plus a clean start. Each of the three op changes, reverted alone, turns its own
assertion red. Composed roots are force-root-checked at startup only (review note F8): the set is
fixed for the process, consistent with the stdio threat model. SOP review
`reviews/10-cr-gh6-7-10-11.md`: APPROVE.

- feature: [[#qmd84_mcp_with]]
- completed: 2026-10-03
- files_changed: [qmdc-rs/src/core/index_seam.rs, qmdc-rs/src/core/resolved_index.rs, qmdc-rs/src/core/ops/validate.rs, qmdc-rs/src/core/ops/rename_plan.rs, qmdc-rs/src/core/ops/dump.rs, qmdc-rs/src/mcp/tools.rs, qmdc-rs/src/mcp/resources.rs, qmdc-rs/src/mcp/server.rs, qmdc-rs/src/main.rs, docs/mcp/server.qmd.md, CHANGELOG.md]
- tests_changed: [qmdc-rs/tests/mcp_compose_with.rs]
