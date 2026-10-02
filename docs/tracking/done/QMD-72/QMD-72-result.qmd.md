# QMD-72: Result

## Task Result [[qmd72_result: Result]]

Explicit multi-root composition landed on every surface, and the composition that already
existed stopped giving three different answers about one reference. A project is now named by
the set of workspaces the caller passes, not by where those workspaces happen to sit on one
developer's disk.

- feature: [[#qmd72]]
- files_changed: [qmdc-rs/src/main.rs, qmdc-rs/src/workspace.rs, qmdc-rs/src/lib.rs, qmdc-rs/src/core/index_seam.rs, qmdc-rs/src/lsp/server.rs, qmdc-rs/src/lsp/workspace.rs, qmdc-py/qmdc/cli.py, qmdc-py/qmdc/workspace.py, qmdc-ts/src/cli.ts, qmdc-ts/src/workspace.ts, docs/format/workspaces.qmd.md, docs/format/objects.qmd.md, docs/format/validation-errors.qmd.md, docs/mcp/readme.qmd.md, docs/mcp/server.qmd.md, docs/parsers/commands.qmd.md, docs/guides/qmdc-guide.qmd.md, README.md, CHANGELOG.md]
- tests_added: [tests/cli/023-compose-with-flag, tests/cli/024-with-and-positional-rejected, tests/cli/025-with-zero-workspaces-rejected, tests/cli/026-with-many-workspaces-rejected, tests/cli/027-with-duplicate-path-rejected, tests/cli/028-with-duplicate-workspace-id-rejected, tests/cli/029-compose-duplicate-local-id, tests/cli/030-with-at-scan-depth-limit, tests/cli/031-with-deeper-than-scan-depth-rejected, tests/cli/032-query-without-path-rejected, tests/cli/033-with-file-follows-workspace-id, tests/cli/034-with-nested-member-not-an-error, tests/cli/035-with-nested-left-out-still-reported, tests/mcp/qmd72-container-validate-references, tests/mcp/qmd72-container-describe-metamodel, tests/lsp/microtests]
- commits: [44c0bc4, e607684]

### What shipped [[qmd72_result_shipped: text]]

- about: [[#qmd72_result]]

A repeatable `-w` / `--with` flag composes workspaces in place on `query`, `workspace parse`,
`workspace validate` and `mcp`, in all three implementations. Composition never copies, syncs or
elects a primary: an object keeps its owning `__workspace`, and the workspace-id-to-local-root
mapping lives in the composed context beside the graph, so two people with different checkout
paths get the identical graph.

The existing container bug went with it. Before, one reference in a container of sibling
workspaces got three verdicts — `query` resolved the edge, `workspace validate` called it
`broken_link`, and MCP refused the container as `ambiguous`. All three now answer from the same
resolver, which is why adding a second way to compose did not double the number of ways to
disagree.

Two measurements changed the plan mid-flight, and both are recorded as findings rather than
quietly absorbed: owner-relative `__file` collides in any composed result (every workspace root
file is `readme.qmd.md`), so the base stays and `-w` mounts at the workspace id; and a nested
workspace member is not an error when it is composed in, but stays an error when it is left out,
because references into it are then genuinely dead.

### Why no second review round [[qmd72_result_review: text]]

- about: [[#qmd72_result]]

The code-review gate ran and its report is `reviews/03-cr-qmd72-multiroot.md`. It found four
`-w` inputs where the three implementations disagreed and zero blockers or criticals, so the
SOP's "re-run until no criticals remain" was already satisfied by the first round.

The four fixes are not resting on that report, though. Each is pinned by a fixture that runs in
all three conformance harnesses — `030-with-at-scan-depth-limit` and
`031-with-deeper-than-scan-depth-rejected` bracket the depth-5 boundary on both sides,
`032-query-without-path-rejected` pins the refusal the task itself had introduced into Rust, and
seven refusal fixtures gained an `expected.stderr` substring so a usage case now pins the reason
for the refusal instead of only "exit 2, empty stdout". That assertion was itself verified by
planting a wrong expectation and confirming all three harnesses fail on it.

Two permission cases from the review are deliberately not fixtures: a mode-000 path cannot be
committed to git, so they live in the finding and in the probe scripts rather than pretending to
be pinned.
