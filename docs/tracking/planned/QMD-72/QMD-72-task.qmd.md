# QMD-72: Multi-root workspace composition, and one resolver behind every surface

## Compose explicit workspaces, and make every surface agree on the result [[qmd72: Feature]]

`qmdc` already composes sibling workspaces when they happen to sit under one common
non-workspace parent. A real multi-repository project does not keep every checkout under
one directory, so today the only way to compose such a project is to copy files or build
an artificial tree — and both put filesystem layout into the graph's identity.

This task adds an explicit form: one or more `-w` / `--with` paths, each pointing at a
workspace anywhere on disk, composed in place. It applies to `query`, `workspace parse`,
`workspace validate` and `mcp`, in all three implementations.

It also fixes the existing composition, because the two are the same problem seen twice.
Today a container of sibling workspaces gives three different answers about one reference:
`query` resolves the cross-workspace edge, `workspace validate` calls that same reference
broken, and MCP refuses the container outright as ambiguous. Adding a second way to compose
without fixing that would just double the number of ways to disagree.

- status: planned
- priority: high
- category: parser
- upstream_issues: [https://github.com/mikilabs/qmdc/issues/10, https://github.com/mikilabs/qmdc/issues/9]
- requires_changes: [qmdc-rs/src/main.rs, qmdc-rs/src/workspace.rs, qmdc-py/qmdc/cli.py, qmdc-py/qmdc/workspace.py, qmdc-ts/src/cli.ts, qmdc-ts/src/workspace.ts]
- findings: [[[#qmd72_finding_issue9_stale]], [[#qmd72_finding_seam_split]], [[#qmd72_finding_surface_scope]], [[#qmd72_finding_file_relative]], [[#qmd72_finding_usage_exit]], [[#qmd72_finding_regressions]], [[#qmd72_finding_coverage_map]], [[#qmd72_finding_c3_mechanism]], [[#qmd72_finding_lsp_split]], [[#qmd72_finding_ambiguous_narrowed]], [[#qmd72_finding_wrapped_ref]], [[#qmd72_finding_review_conformance]], [[#qmd72_finding_file_identity]], [[#qmd72_finding_nested_member]]]
- result: null

### Why an explicit form rather than more discovery [[qmd72_why: text]]

- about: [[#qmd72]]

Discovery answers "what is near this path". Composition answers "which workspaces make up
this project", and those are different questions: the second one's answer is a property of
the project, the first one's is a property of one developer's disk.

`qmdc-wiki` is the concrete consumer. A logical Project there may span several repositories
whose checkout paths differ between users, so the set of workspaces has to be supplied by
the caller and the graph must not record where they happened to live.

### The invariant that makes this safe [[qmd72_invariant: text]]

- about: [[#qmd72]]

Composition reads workspaces in place. It never copies, syncs, moves, or elects an
authoritative workspace, and no filesystem path enters the graph: an object keeps its owning
`__workspace`, `__file` stays relative to that owner, and the workspace-id-to-local-root
mapping lives in the composed context beside the graph rather than inside it.

That is what lets the same two repositories, checked out at different paths by two people,
produce the identical graph.

### The existing bug, now in scope [[qmd72_related_bug: text]]

- about: [[#qmd72]]

Upstream issue 9 reports it on 1.0.7: two sibling workspaces under a non-workspace container, one
holding a reference qualified with the other's workspace id. `query` returns the edge with the
expected target, `workspace validate` on the same container reports `broken_link`, and MCP reference
validation refuses the container with `ambiguous` — "path contains 2 workspaces".

Triage re-measured all of it on HEAD, and only the MCP half survives. The reference as written in the
issue carries four segments including a `Kind`, which the qualified-reference grammar does not
define; 1.0.7's `query` was the permissive one, emitting an edge for a malformed reference that
validation correctly rejected. On HEAD all three agree it is broken, and on the conformant
three-segment form all three resolve it and validate clean.

What remains is the seam: MCP and LSP resolve a path to ONE workspace root while the CLI composes
every workspace it finds, so a container is refused rather than answered — and neither candidate
alone can see the other's target. Details in [[#qmd72_finding_issue9_stale]] and
[[#qmd72_finding_seam_split]].

Its own stated risk still matters: query may be finding a target by a globally unique object id
without enforcing every supplied qualifier, which would make the resolution correct by luck. That is
what the qualifier and duplicate-id goals below pin.

Because this half is a Bug, the SOP's bug-triage exception applies to it: triage adds the
smallest data-driven regression reproducing the three-way disagreement, and that regression
is expected to be red before implementation.

### Goals [[goals: [Goal]]]

#### A1: Repeatable `-w` / `--with` on every workspace-aware command [[qmd72_goal_a1]]

`query`, `workspace parse` and `workspace validate` accept one or more `-w` / `--with` paths,
in all three implementations; the Rust `mcp` subcommand accepts them too. Every path is a
peer — the first is not primary — and a single `-w` is valid. The existing positional form
keeps today's behaviour, and `.` stays the implicit workspace wherever a command already
supports one.

Triage measured the surface rather than assuming it: Python and TypeScript have no `mcp` or
`lsp` subcommand at all, so three-way conformance can only cover the three workspace commands
(see [[#qmd72_finding_surface_scope]]).

Done when: the positional form's output is unchanged on the existing corpus, and two
workspaces at unrelated paths compose through repeated `-w` in Rust, Python and TypeScript.

- group: A_cli_surface
- done: true

#### A2: Every rejected input fails as a usage error, not as a wrong answer [[qmd72_goal_a2]]

Five shapes must be refused: a positional path together with `-w`; a `-w` path resolving to
zero workspaces; a `-w` path resolving to more than one; the same path twice; two paths
carrying the same workspace id.

Done when: each shape exits non-zero naming the offending path, identically in all three,
and a CLI conformance fixture pins each one — including its exit code, which the harness
already compares from a per-case `exit` file, so no harness work is needed. The starting
point is itself a divergence: an unknown `-w` exits 2 in Rust and Python and 1 in TypeScript
(see [[#qmd72_finding_usage_exit]]).

- group: A_cli_surface
- done: true

#### A3: `--force-root` bounds every composed path [[qmd72_goal_a3]]

`--force-root` exists today only on the Rust `mcp` subcommand, where it fences the server to
one directory. A composed set has to honour it.

The two flags never meet on one surface: `-w` is a CLI option and `--force-root` is an `mcp`
one, and `mcp` takes its paths per request rather than at startup. What the seam change did
alter is WHAT reaches the boundary check — previously a single resolved workspace root, now the
container itself — and the boundary holds by construction, because every composed member lives
under the container that was checked.

Done when: the existing force-root test also covers a composed container inside the boundary,
and an outside path still fails closed with `out-of-root`.

- group: A_cli_surface
- done: true

#### B1: One composition primitive per implementation [[qmd72_goal_b1]]

A single function — `compose_workspaces(paths)` or the local equivalent — discovers the
workspaces and builds the composed index. Query, `workspace parse`, `workspace validate`,
MCP and LSP consume it rather than each doing its own discovery and resolution, and the
existing common-parent discovery is expressed in terms of it rather than beside it.

Done when: every one of those surfaces reaches composition through that function, verified by
reading each call site, so a surface cannot diverge without editing it.

- group: B_shared_primitive
- done: true

#### B2: Make identity survive composition [[qmd72_goal_b2]]

Under `-w`, `__file` starts with the workspace id: `shop/storage/tables.qmd.md`. The envelope
always carries `workspaces`, one `{id, root, path}` entry per workspace, replacing the QMD-59
`workspace` / `workspaces` / `workspace: null` trio; `root` is null for the `-w` form, whose
base is virtual. No host path is stored in any object.

This deviates from the plan approved in chat ("`__file` relative to its owner, in every
form"). Measured during implementation: owner-relative `__file` is not unique in a composed
result, since every workspace's root file is `readme.qmd.md`, and the `files` list, an error's
`file`, the py/ts `index.by_file` and the reference scanner's source-line cache all key on it
(see [[#qmd72_finding_file_identity]]). The id prefix keeps the value unique and still
portable. The container form keeps `__file` relative to the container: the MCP rename tool
joins the index root with `__file` to find each file to edit, and the container's own
subdirectory names are part of the input the caller gave.

Done: over two checkouts at different depths and names, `__file` is `<id>/...` identically in
all three (`tests/cli/033-with-file-follows-workspace-id`, caught a sabotage that mounted by
directory name); the version decision is still open.

- group: B_shared_primitive
- done: true

#### C1: Every supplied workspace contributes to the answer [[qmd72_goal_c1]]

Done when: a query over a composed set returns objects and edges from all supplied
workspaces, and dropping one path drops exactly its rows.

- group: C_resolution
- done: true

#### C2: A workspace qualifier is enforced, never a hint [[qmd72_goal_c2]]

A reference naming a workspace resolves in that workspace only. A wrong qualifier is a
broken reference — never a fallback to a same-named object in another workspace.

Already covered by QMD-69: `tests/cli/012-validate-qualified-hierarchical` pins the
correct-qualifier side and `013-validate-qualified-local-id-unknown-workspace` the
wrong-qualifier side, both three-way (see [[#qmd72_finding_coverage_map]]).

Done when: fixtures pin both the correct-qualifier resolution and the wrong-qualifier
failure, and `query` and `workspace validate` agree on both.

- group: C_resolution
- done: true

#### C3: The same local id in two workspaces stays two objects [[qmd72_goal_c3]]

This is issue 9's acceptance test 3 and the only one of its five with NO fixture at any level.
The nearest existing one is not it: `tests/workspace/local-id-ambiguous` covers two children
sharing a `__local_id` inside ONE workspace (see [[#qmd72_finding_coverage_map]]).

Implementation measured the mechanism rather than assuming it, and the goal's original wording
was wrong: an unqualified reference is workspace-LOCAL, so from a third workspace it reaches
neither duplicate and the answer is `broken_link`, not `ambiguous_reference`. Nothing needed
changing — the invariant already held in all three (see [[#qmd72_finding_c3_mechanism]]).

Done when: a fixture pins all three facts on one composed set — two separate objects, each
qualified reference reaching its own, and the bare reference reaching neither.

- group: C_resolution
- done: true

#### C4: Query, validation, MCP and LSP read the same composed graph [[qmd72_goal_c4]]

The split described in the related-bug section must not be reachable through the new form.
MCP and LSP exist only in Rust, so in Python and TypeScript this reduces to `query` and
`workspace validate` (see [[#qmd72_finding_surface_scope]]).

Fixing the MCP seam left the LSP still disagreeing — it resolved against the workspace owning
the open document rather than the composed set, so a valid cross-workspace reference was
reported broken in the editor. Found only because the new microtest asserts a positive finding
beside the resolved one (see [[#qmd72_finding_lsp_split]]).

Done when: for one composed set, `query`, `workspace validate`, and — in Rust — MCP reference
validation and LSP diagnostics return the same verdict on the same reference.

- group: C_resolution
- done: true

#### C5: Keep the container form self-consistent [[qmd72_goal_c5]]

`workspace validate` on a container of sibling workspaces returns no `broken_link` for a
qualified cross-workspace reference that `query` resolves — in the root namespace and in a
namespaced form.

Triage found this ALREADY HOLDS on HEAD in all three. Issue 9 was measured on 1.0.7, and its
reference carries a `Kind` segment the qualified-reference grammar does not define; on that
malformed input all three now agree it is broken, and on the conformant form all three resolve
it and validate clean. So this goal is a regression to keep, not a defect to fix, and the CLI
half of issue 9 needs no code (see [[#qmd72_finding_issue9_stale]]).

Done when: the root-namespace and namespaced fixtures stay green in all three, and the same
reference gives one verdict from `query` and from `workspace validate`.

- group: C_resolution
- done: true

#### C6: MCP accepts a container of sibling workspaces [[qmd72_goal_c6]]

MCP reference validation composes a non-workspace container holding sibling workspaces
instead of refusing it with `ambiguous`. Ambiguity remains the right answer where a path
genuinely cannot be resolved to a workspace set, so the change is in composing rather than in
dropping the check.

Done when: validating the container over MCP returns the composed verdict, an LSP microtest
folder carries both its LSP and MCP expectations, and the existing ambiguity cases still fail
as before.

- group: C_resolution
- done: true

#### C7: A nested workspace composed as a member is not an error [[qmd72_goal_c7]]

`nested_workspace` reports a workspace whose files are missing from the result. When the inner
workspace is itself composed — `-w repo -w repo/.qmdc`, or a container holding both — nothing
is missing, and the report contradicted the composition the caller asked for. It is dropped
there and kept when the inner one is left out. Matched by path, not id. Added during B2 at the
owner's request; the case is qmdc-wiki's `.qmdc/` inside every repository it models (see
[[#qmd72_finding_nested_member]]).

Done: `tests/cli/034-with-nested-member-not-an-error` (caught a sabotage that never dropped
it) and its control `035-with-nested-left-out-still-reported`, identical in all three.

- group: C_resolution
- done: true

#### D1: Equivalent conformance coverage in all three [[qmd72_goal_d1]]

Done when: every new behaviour is pinned by data-driven fixtures running against Rust,
Python and TypeScript, `make test` is green, and no fixture carries an
implementation-specific exception.

- group: D_conformance
- done: true

#### E1: The new form is documented wherever the old one is [[qmd72_goal_e1]]

Done when: every place documenting the positional workspace argument also documents `-w` /
`--with` and their mutual exclusion — CLI reference, guide, README, CHANGELOG — and the
guide's byte-identical copy under `qmdc-rs/src/` matches.

- group: E_docs
- done: true
