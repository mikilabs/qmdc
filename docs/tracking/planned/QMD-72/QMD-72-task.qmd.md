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
- findings: []
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

Upstream issue 9, measured on 1.0.7: two sibling workspaces under a non-workspace container,
one holding a reference qualified with the other's workspace id. `query` returns the edge
with the expected target. `workspace validate` on the same container reports `broken_link`
for that reference. MCP reference validation refuses the container with `ambiguous` — "path
contains 2 workspaces" — and validating either candidate alone cannot see the other's target
at all.

Its own stated risk matters as much as the symptom: query may be finding the target by a
globally unique object id without enforcing every supplied qualifier, which would make the
resolution correct by luck. That is what the qualifier and duplicate-id goals below pin.

Because this half is a Bug, the SOP's bug-triage exception applies to it: triage adds the
smallest data-driven regression reproducing the three-way disagreement, and that regression
is expected to be red before implementation.

### Goals [[goals: [Goal]]]

#### A1: Repeatable `-w` / `--with` on every workspace-aware command [[qmd72_goal_a1]]

`query`, `workspace parse`, `workspace validate` and `mcp` accept one or more `-w` /
`--with` paths. Every path is a peer — the first is not primary — and a single `-w` is
valid. The existing positional form keeps today's behaviour, and `.` stays the implicit
workspace wherever a command already supports one.

Done when: the positional form's output is unchanged on the existing corpus, and two
workspaces at unrelated paths compose through repeated `-w` in Rust, Python and TypeScript.

- group: A_cli_surface
- done: false

#### A2: Every rejected input fails as a usage error, not as a wrong answer [[qmd72_goal_a2]]

Five shapes must be refused: a positional path together with `-w`; a `-w` path resolving to
zero workspaces; a `-w` path resolving to more than one; the same path twice; two paths
carrying the same workspace id.

Done when: each shape exits non-zero naming the offending path, identically in all three,
and a CLI conformance fixture pins each one — including its exit code, which the harness
compares separately from stdout.

- group: A_cli_surface
- done: false

#### A3: `--force-root` bounds every composed path [[qmd72_goal_a3]]

`--force-root` exists today only on the Rust `mcp` subcommand, where it fences the server to
one directory. A composed set has to honour it: a `-w` path outside the boundary is rejected
rather than read.

Done when: an outside `-w` under `--force-root` exits non-zero, an inside one still works,
and the existing force-root tests still pass.

- group: A_cli_surface
- done: false

#### B1: One composition primitive per implementation [[qmd72_goal_b1]]

A single function — `compose_workspaces(paths)` or the local equivalent — discovers the
workspaces and builds the composed index. Query, `workspace parse`, `workspace validate`,
MCP and LSP consume it rather than each doing its own discovery and resolution, and the
existing common-parent discovery is expressed in terms of it rather than beside it.

Done when: every one of those surfaces reaches composition through that function, verified by
reading each call site, so a surface cannot diverge without editing it.

- group: B_shared_primitive
- done: false

#### B2: Identity and paths survive composition [[qmd72_goal_b2]]

Objects keep their owning `__workspace`; `__file` stays relative to its owner; the composed
context carries the workspace-id-to-local-root mapping so a consumer can turn a `__file`
back into a real path. No absolute path is stored in the graph.

Done when: a composed query over two unrelated paths returns `__file` relative to each
owner, and moving both workspaces to different directories produces an identical graph apart
from that mapping.

- group: B_shared_primitive
- done: false

#### C1: Every supplied workspace contributes to the answer [[qmd72_goal_c1]]

Done when: a query over a composed set returns objects and edges from all supplied
workspaces, and dropping one path drops exactly its rows.

- group: C_resolution
- done: false

#### C2: A workspace qualifier is enforced, never a hint [[qmd72_goal_c2]]

A reference naming a workspace resolves in that workspace only. A wrong qualifier is a
broken reference — never a fallback to a same-named object in another workspace.

Done when: fixtures pin both the correct-qualifier resolution and the wrong-qualifier
failure, and `query` and `workspace validate` agree on both.

- group: C_resolution
- done: false

#### C3: The same local id in two workspaces stays two objects [[qmd72_goal_c3]]

Done when: an unqualified reference to an id present in two composed workspaces is reported
ambiguous rather than resolved arbitrarily, and the fixture forces that determinism instead
of passing on row order.

- group: C_resolution
- done: false

#### C4: Query, validation, MCP and LSP read the same composed graph [[qmd72_goal_c4]]

The split described in the related-bug section must not be reachable through the new form.

Done when: for one composed set, `query`, `workspace validate`, MCP reference validation and
LSP diagnostics return the same verdict on the same reference.

- group: C_resolution
- done: false

#### C5: The existing container form stops contradicting itself [[qmd72_goal_c5]]

`workspace validate` on a container of sibling workspaces returns no `broken_link` for a
qualified cross-workspace reference that `query` resolves — in the root namespace and in a
namespaced form.

Done when: the regression built during triage is green in all three, and the same reference
gives one verdict from `query` and from `workspace validate`.

- group: C_resolution
- done: false

#### C6: MCP accepts a container of sibling workspaces [[qmd72_goal_c6]]

MCP reference validation composes a non-workspace container holding sibling workspaces
instead of refusing it with `ambiguous`. Ambiguity remains the right answer where a path
genuinely cannot be resolved to a workspace set, so the change is in composing rather than in
dropping the check.

Done when: validating the container over MCP returns the composed verdict, an LSP microtest
folder carries both its LSP and MCP expectations, and the existing ambiguity cases still fail
as before.

- group: C_resolution
- done: false

#### D1: Equivalent conformance coverage in all three [[qmd72_goal_d1]]

Done when: every new behaviour is pinned by data-driven fixtures running against Rust,
Python and TypeScript, `make test` is green, and no fixture carries an
implementation-specific exception.

- group: D_conformance
- done: false

#### E1: The new form is documented wherever the old one is [[qmd72_goal_e1]]

Done when: every place documenting the positional workspace argument also documents `-w` /
`--with` and their mutual exclusion — CLI reference, guide, README, CHANGELOG — and the
guide's byte-identical copy under `qmdc-rs/src/` matches.

- group: E_docs
- done: false
