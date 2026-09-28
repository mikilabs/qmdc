# QMD-72: Findings

## Issue 9's reported contradiction does not reproduce on HEAD — its reference was malformed [[qmd72_finding_issue9_stale: Finding]]

The issue reports `query` resolving a cross-workspace reference that `workspace validate` calls
broken, measured on 1.0.7. On HEAD all three implementations agree on that input, and they agree it
is broken — because the reference in the issue is not a form the format defines. The half of issue 9
that is still live is MCP, not the CLI.

- category: parser
- priority: high
- related_to: [[#qmd72]]
- affected_files: [docs/format/references.qmd.md]
- solution: Keep the CLI half as a regression rather than a fix — C5 is already satisfied on HEAD and needs a fixture to stay that way. Spend the implementation budget on the MCP half (C6) and on the one uncovered acceptance test (C3).
- test_plan: [[#qmd72_finding_regressions]]

### What each version actually answers [[qmd72_finding_issue9_stale_detail: text]]

- about: [[#qmd72_finding_issue9_stale]]

The reference as written in the issue carries four segments, including a `Kind`:

```markdown example
- includes: [[#repo_a:services:Service:payments_api]]
```

The qualified-reference grammar has no `Kind` segment. So this is a malformed reference, and 1.0.7's
`query` was the implementation in the wrong: it emitted an `includes` edge for it while
`workspace validate` correctly called it broken. The issue reads that disagreement as validation
being wrong; it was resolution being too permissive.

On HEAD, measured on the same input, all three now agree — no `includes` edge, and
`broken_link: Object 'Service:payments_api' not found`, exit 1.

With the spec-conformant three-segment form on the same two-workspace container, all three resolve
it and all three validate clean:

| surface | rs | py | ts |
| --- | --- | --- | --- |
| `query` edges | `commerce -> repo_a:services:payments_api` | same | same |
| `workspace validate` | `[]`, exit 0 | same | same |

So the CLI's composition of sibling workspaces works, and works identically in three
implementations. That is a QMD-69 outcome, not a QMD-72 one.

## MCP resolves a path to ONE workspace where the CLI composes a SET [[qmd72_finding_seam_split: Finding]]

This is the live half of issue 9, and the split is not in the resolution rules — both sides resolve
references the same way. It is in what "this path" means before resolution starts: the MCP/LSP seam
picks a single workspace root, while the CLI composes every workspace it finds.

- category: lsp
- priority: high
- related_to: [[#qmd72]]
- affected_files: [qmdc-rs/src/core/index_seam.rs]
- affected_functions: [resolve_root, resolve_root_bidirectional]
- solution: Express the seam in terms of the composition primitive B1 introduces, so a container of sibling workspaces composes instead of being refused. Ambiguity stays the right answer for a path that genuinely cannot be resolved to a workspace set, so the change is in composing, not in dropping the check.
- test_plan: [[#qmd72_finding_regressions]]

### Where it refuses, and what must keep refusing [[qmd72_finding_seam_split_detail: text]]

- about: [[#qmd72_finding_seam_split]]

`resolve_root` returns a single `PathBuf`. Its `n > 1` arm is a deliberate QMD-63 decision:

```text
path '<container>' contains 2 workspaces (searched N levels down);
re-call with one of `candidates` as `path`
```

The envelope carries `candidates` so the caller can pick one — which is exactly what makes the
container unanswerable, because neither candidate alone can see the other's target.

The existing `qmd63-ambiguous` fixture pins that refusal, so it is the boundary C6 has to respect:
after the change a container of sibling workspaces composes, and a path that resolves to no
workspace set at all still answers `ambiguous`. That fixture must stay green, and a change that
makes it fail has removed the check rather than narrowed it.

## MCP and LSP exist only in Rust, so the task's surface list is narrower than written [[qmd72_finding_surface_scope: Finding]]

Measured from each CLI's own subcommand list, not from the source: Python offers
`parse / query / rebuild / workspace`, TypeScript offers `parse / rebuild / workspace / query`.
Neither has `mcp` or `lsp`.

- category: testing
- priority: medium
- related_to: [[#qmd72]]
- solution: Scope the goals to what exists — `-w` on `mcp`, `--force-root` and MCP/LSP agreement are Rust-only; three-way conformance can only cover `query`, `workspace parse` and `workspace validate`. State that in the goals rather than discovering it during implementation.
- test_plan: [[#qmd72_finding_regressions]]

### Consequences per goal [[qmd72_finding_surface_scope_detail: text]]

- about: [[#qmd72_finding_surface_scope]]

A1 names four commands; only three of them exist in three implementations. A3 is Rust-only already —
`--force-root` is a flag of the Rust `mcp` subcommand, not a global option. C4 asks query,
validation, MCP and LSP to agree on one composed set: in Python and TypeScript that reduces to query
and validation, which is a weaker claim and should be written as one.

D1's "no fixture carries an implementation-specific exception" stays achievable, because the fixture
directories are per surface: a Rust-only surface gets Rust-only fixtures rather than a three-way
fixture with two exemptions.

## `__file` is relative to the container today, so B2 is a breaking output change [[qmd72_finding_file_relative: Finding]]

B2 is written as preserving a property. It is not: the property does not hold today, in any of the
three. Composition currently reports `__file` relative to the composed container, so the same
workspace produces different `__file` values depending on what it was composed with — which is the
filesystem-layout leak the task set out to remove.

- category: parser
- priority: high
- related_to: [[#qmd72]]
- affected_files: [qmdc-rs/src/workspace.rs, qmdc-py/qmdc/workspace.py, qmdc-ts/src/workspace.ts]
- solution: Relativise `__file` to its owning workspace, turn `workspaces` from a list of ids into the id-to-local-root mapping the invariant requires, and decide what `root` means for a set of unrelated paths. All three are output-shape changes and belong in the version decision.
- test_plan: [[#qmd72_finding_regressions]]

### Measured output shape [[qmd72_finding_file_relative_detail: text]]

- about: [[#qmd72_finding_file_relative]]

`workspace parse` over a container holding `project/` and `repo_a/`, identical in all three:

| field | today | what B2 requires |
| --- | --- | --- |
| `__file` of an object in `repo_a` | `repo_a/services/readme.qmd.md` | `services/readme.qmd.md` |
| `workspaces` | `["qmd72_project", "qmd72_repo_a"]` — ids only | id to local root |
| `root` | one path string | a set has no single root |

Two smaller observations from the same measurement. `root` already diverges: Rust echoes the
argument as given (relative), Python and TypeScript emit an absolute path — so the field that a
composed set has to replace is not even consistent today. And a `__Workspace` root object carries
`__workspace: null` rather than its own id, which any consumer grouping objects by owner has to
special-case.

## `-w` fails three different ways today, and the harness already pins exit codes [[qmd72_finding_usage_exit: Finding]]

A2 asks for identical usage failures. The starting point is already a divergence, on the simplest
possible input — an option none of them has.

- category: testing
- priority: medium
- related_to: [[#qmd72]]
- affected_files: [qmdc-rs/src/main.rs, qmdc-py/qmdc/cli.py, qmdc-ts/src/cli.ts]
- solution: Pin the exit code for each of A2's five shapes. No harness work is needed — the CLI conformance runner already compares a per-case `exit` file, used today by `013-validate-qualified-local-id-unknown-workspace`.
- test_plan: [[#qmd72_finding_regressions]]

### Today's three answers [[qmd72_finding_usage_exit_detail: text]]

- about: [[#qmd72_finding_usage_exit]]

`workspace validate -w ./repo_a -w ./project`:

| impl | exit | first line |
| --- | --- | --- |
| rs | 2 | `error: unexpected argument '-w' found` |
| py | 2 | `Usage: qmdc.py workspace validate [OPTIONS] [PATH]` |
| ts | 1 | (npm wrapper noise, then its own usage) |

TypeScript's `1` is the odd one, and its stderr is polluted by npm warnings before its own message.
Code review closed both halves: the exit code was brought to `2` in all three (see
[[#qmd72_finding_review_conformance]]), and the three conformance harnesses gained an optional
per-case `expected.stderr` **substring** file, so a usage fixture now pins the reason for the refusal
instead of only "exited 2 and printed nothing on stdout". Substring rather than whole-stream, which
is what makes the npm noise harmless.

## Two regressions added by triage, both red for their own declared reason [[qmd72_finding_regressions: Finding]]

The SOP's bug-triage exception applies to the issue-9 half, so triage adds the smallest data-driven
regression that reproduces the live defect, and it is red before implementation. A second fixture
pins the new CLI surface.

- category: testing
- priority: high
- related_to: [[#qmd72]]
- affected_files: [tests/mcp/qmd72-container-validate-references, tests/cli/023-compose-with-flag]
- solution: Keep both red until implementation. Neither is red for an incidental reason — each was cross-checked against a passing control so that it can only go green when its goal is met.
- test_plan: [[#qmd72_finding_regressions]]

### What each one pins, and how it was controlled [[qmd72_finding_regressions_detail: text]]

- about: [[#qmd72_finding_regressions]]

`tests/mcp/qmd72-container-validate-references/` — `qmdc_validate_references` over a container of two
sibling workspaces, expecting `diagnostics: []`. This is issue 9's acceptance test 4 (one verdict
from query, CLI validation and MCP validation) and it is the required bug regression. It fails with
the `ambiguous` envelope quoted above. Control: the same workspace answers `[]` from
`workspace validate` in all three, so the content is correct and only the seam refuses it.

`tests/cli/023-compose-with-flag/` — `workspace validate -w ./repo_a -w ./project`, expecting `[]`.
The case directory is deliberately NOT a workspace, which is what distinguishes it from
`011-validate-cross-workspace`: there the implicit positional `.` composes the same shape and already
passes, so only the flag can bind this one. Both paths are peers, with the reference in the second
and its target in the first, so treating the first `-w` as primary would still have to resolve across
them. Control: the same two directories validate to `[]` through the positional form, so the fixture
is red only because the flag does not exist.

Full-suite reading after adding both: 4 failures across 2 fixtures and nothing else. That needed
`--no-fail-fast` to see — nextest stops at the first failure and then writes only some of the 20 XML
reports, so an aggregated count read without it describes a partial run.

## Three of issue 9's five acceptance tests are already green from QMD-69 [[qmd72_finding_coverage_map: Finding]]

SOP requires checking existing coverage before adding fixtures. QMD-69 built most of issue 9's
acceptance suite already, one fixture citing the issue by number, which is why the CLI half needs no
work.

- category: testing
- priority: medium
- related_to: [[#qmd72]]
- solution: Add only what is missing — acceptance test 4 (the new MCP regression) and acceptance test 3, duplicate local ids across sibling workspaces, which has no fixture at any level.
- test_plan: [[#qmd72_finding_regressions]]

### The map [[qmd72_finding_coverage_map_detail: text]]

- about: [[#qmd72_finding_coverage_map]]

| issue 9 acceptance test | status | fixture |
| --- | --- | --- |
| 1. root-namespace cross-workspace reference | green | `tests/cli/011-validate-cross-workspace` (cites issue 9) |
| 2. namespaced cross-workspace reference | green | `tests/cli/012-validate-qualified-hierarchical` |
| 3. duplicate object ids across sibling workspaces | UNCOVERED | — |
| 4. one verdict from query, CLI and MCP validation | red | `tests/mcp/qmd72-container-validate-references` |
| 5. wrong qualifier does not fall back globally | green | `tests/cli/013-validate-qualified-local-id-unknown-workspace` |

Acceptance test 3 is the gap C3 has to close, and the nearest existing fixture is not it:
`tests/workspace/local-id-ambiguous` covers two children sharing a `__local_id` inside ONE workspace,
which the sibling-workspace case does not reduce to.

## C3 needed no code — a bare reference never crosses a workspace [[qmd72_finding_c3_mechanism: Finding]]

The goal was written expecting `ambiguous_reference` for an id present in two composed
workspaces. Measured behaviour is different and already correct in all three: a reference
without a qualifier is workspace-LOCAL, so from a third workspace it reaches neither duplicate
and the answer is `broken_link`.

- category: parser
- priority: medium
- related_to: [[#qmd72]]
- solution: Pin the measured invariant instead of changing behaviour. Ambiguity across workspaces cannot arise from a bare reference, so no ambiguity rule is needed for duplicate local ids.
- test_plan: [[#qmd72_finding_regressions]]

### What the three facts are [[qmd72_finding_c3_mechanism_detail: text]]

- about: [[#qmd72_finding_c3_mechanism]]

Composed set: `repo_a` and `repo_b` each declaring `config`, plus a `consumer` referring to
both. Identical in Rust, Python and TypeScript:

| reference from `consumer` | result |
| --- | --- |
| `[[#repo_a::config]]` | resolves to `repo_a`'s object |
| `[[#repo_b::config]]` | resolves to `repo_b`'s object |
| `[[#config]]` | `broken_link` — reaches neither |

The two objects stay separate (`c3_repo_a::config` and `c3_repo_b::config` in the edge table),
which is identity being workspace-scoped (QMD-67) rather than a rule about composition.

## The LSP had the same split as MCP, and only a fixture found it [[qmd72_finding_lsp_split: Finding]]

C4 asks query, validation, MCP and LSP to agree on one composed set. After the MCP seam was
fixed, three of the four agreed — the LSP still reported a VALID qualified cross-workspace
reference as broken in the editor, because it resolved against the workspace OWNING the open
document instead of the composed set.

- category: lsp
- priority: high
- related_to: [[#qmd72]]
- affected_files: [qmdc-rs/src/lsp/server.rs, qmdc-rs/src/lsp/workspace.rs]
- affected_functions: [siblings_of, publish_diagnostics]
- solution: Build the resolution index from every workspace sharing the open file's `project_root`, which is what the registry already holds — the same container the CLI composes. Identity stays workspace-scoped, so this cannot make a bare local id cross a boundary; it only lets a qualified reference find the workspace it names.
- test_plan: [[#qmd72_finding_regressions]]

### Why it survived the MCP fix [[qmd72_finding_lsp_split_detail: text]]

- about: [[#qmd72_finding_lsp_split]]

The MCP seam and the LSP reach composition by different routes: MCP resolves a path then calls
`get_index`, which composes; the LSP indexes each workspace under the project folder
separately and picks the owner per request. Fixing the seam therefore fixed MCP and left the
editor wrong — the user-visible half.

It was found only because the new microtest asserts a POSITIVE finding beside the resolved
one. An expectation of "no diagnostics" would have been satisfied by a resolver that failed to
load the document at all, so the case carries a deliberately-absent target as its control.

Same class as the `-w` fixture, where `workspace validate` returning `[]` was measured to pass
while composing only the first path — an empty result proves nothing about what was loaded.

## `ambiguous` was narrowed, not removed, and QMD-63's own fixture had to move [[qmd72_finding_ambiguous_narrowed: Finding]]

`tests/mcp/qmd63-ambiguous` pinned exactly the shape C6 has to compose: a container holding two
sibling workspaces with distinct ids. The two cannot both be right, so the fixture changed.

- category: testing
- priority: high
- related_to: [[#qmd72]]
- affected_files: [qmdc-rs/src/core/index_seam.rs, tests/mcp/qmd63-ambiguous]
- solution: Keep the error code with a sharper meaning — a set that cannot be composed because two members declare the same workspace id — and make QMD-63's fixture pin THAT, so the code stays reachable and its intent stays pinned. A second fixture keeps the composed-success case covered for a different MCP tool.
- test_plan: [[#qmd72_finding_regressions]]

### Why composing is not a weakening [[qmd72_finding_ambiguous_narrowed_detail: text]]

- about: [[#qmd72_finding_ambiguous_narrowed]]

QMD-63 did not weigh composing and reject it. Its design section states the premise plainly —
"MCP has a single-root contract (one `path` in → one root to index)" — and notes that the CLI's
"parse/union contract differs from MCP's single-root". The ambiguity was a consequence of the
one-root contract, not a judgement that a container is meaningless.

What the old error offered was `candidates` and an instruction to re-call with one of them. But
neither candidate alone can see the other's objects, so that path could not answer the question
either: the caller was asked to choose between two wrong answers.

The surviving case is real and cannot be composed at all. Two workspaces declaring the same id
make every id in one collide with the other's, so no reference could resolve to a single
object. `colliding_workspace_id` decides it from a cheap marker read rather than a full parse of
each candidate, and the same rule is the fifth usage shape refused by `--with`.

## Carried over: Python and TypeScript miss a reference on a wrapped line [[qmd72_finding_wrapped_ref: Finding]]

Found by accident while writing this task's own goals, and NOT about composition — a parser
defect of the QMD-71 family. A reference sitting on the SECOND line of a wrapped paragraph
inside an object-array element is invisible to Python and TypeScript: they record the prose
correctly but collect no reference from it, so a broken link there is silently missed.

- category: parser
- priority: high
- related_to: [[#qmd72]]
- affected_files: [qmdc-py/qmdc/parser.py, qmdc-ts/src/parser.ts]
- solution: Out of scope for QMD-72 — it belongs in its own task, with a regression per cause as QMD-71 established. Recorded here because it means the docs corpus can hide broken references from two of the three parsers.
- test_plan: [[#qmd72_finding_regressions]]

### The minimal repro, and why the gate almost missed it [[qmd72_finding_wrapped_ref_detail: text]]

- about: [[#qmd72_finding_wrapped_ref]]

One line versus two, everything else identical:

```markdown example
### Goals [[goals: [Goal]]]

#### G1: case [[g1]]

A sentence that wraps
onto a second line with [[#absent_one]].

- group: A
- done: false
```

| body | rs | py | ts |
| --- | --- | --- | --- |
| reference on a single-line paragraph | 1 broken_link | 1 | 1 |
| reference on the second line of a wrapped paragraph | 1 broken_link | **0** | **0** |

The cause is visible in the parse output, and it is not the prose capture: `__comments` content
is byte-identical in all three and holds both lines. What differs is `__references` — Rust
records the reference found in a multi-line comment, Python and TypeScript emit `null`. So the
text is kept and only the reference collection is skipped.

Two notes for whoever takes it. The reference produces no EDGE in any of the three, because
prose is not a field — so the loss is confined to validation and to `__references`. And
`make validate-compare` is only sensitive to a reference that is BROKEN: every reference in the
corpus resolves today, so the divergence sat invisible until a goal body happened to name an id
that did not exist yet.

## Code review: four `-w` conformance breaks the corpus could not see [[qmd72_finding_review_conformance: Finding]]

Code review measured the `-w` surface on inputs no fixture carried, and found four inputs where
the same invocation behaves differently per implementation. Each is now identical in all three and
pinned. None was visible to `make test` before, because every `-w` fixture used a well-formed
shallow path.

- category: conformance
- priority: high
- related_to: [[#qmd72]]
- affected_files: [qmdc-rs/src/main.rs, qmdc-py/qmdc/workspace.py, qmdc-py/qmdc/cli.py, qmdc-ts/src/workspace.ts]
- solution: Fixed all four, and closed the measurement gap that hid them — the three conformance harnesses now support an optional per-case `expected.stderr` substring, so a usage fixture pins the reason for a refusal rather than only its exit code.
- test_plan: [[#qmd72_finding_regressions]]

### The four, and the two traps found while fixing them [[qmd72_finding_review_conformance_detail: text]]

- about: [[#qmd72_finding_review_conformance]]

| input | before | after |
| --- | --- | --- |
| `query "<sql>"` with no `-w` | rs runs it against an implied `.`; py/ts refuse, exit 2 | all three refuse, exit 2 |
| workspace 6+ levels below a `-w` path | rs refuses (bounded at 5); py/ts compose it (unbounded) | all three refuse, exit 2 |
| `-w` a plain file, or an unreadable directory | rs/py exit 2; ts exits 1 with a raw `ENOTDIR`/`EACCES` | all three exit 2, same message |
| `-w` a directory whose readme is unreadable | rs exits 2; py/ts exit 1 with a raw OS error | all three exit 2, same message |

The `query` one was **introduced by this task** and classified by running the old code, not by
reading the diff: pre-change Python, executed from a `git archive HEAD` tree, refuses the single
positional with exit 2, and pre-change Rust declared both positionals as required. Only Rust moved,
via a new `(Some(q), None) => (".", q)` arm. Removed.

The depth one needed a contract, not a patch: `WORKSPACE_SCAN_MAX_DEPTH = 5` exists in Rust so that
pointing a tool at a large checkout cannot become a full-tree crawl, and `docs/mcp/readme.qmd.md`
documents the number. Python and TypeScript gained the same bounded scan beside their unbounded one,
mirroring Rust's split rather than changing the unbounded function other callers use.

Two traps surfaced while fixing, both caught by measurement rather than reasoning, and both worth
carrying forward:

**A depth cap applies to the marker, not to the directory holding it.** The first TypeScript
attempt checked the directory's depth before recursing, which finds a marker one level deeper than
Rust and Python do. Measured across depths 1 to 7, the boundary was `found` through 5 and `MISSED`
at 6 in rs/py but still `found` at 6 in ts. A fixture at one depth would have passed; the pair of
fixtures at 5 and 6 pins both sides of the boundary.

**`Path.exists()` does not swallow a permission error.** Python 3.12 ignores only ENOENT, ENOTDIR,
EBADF and ELOOP, so an unreadable parent directory makes the `stat` raise `PermissionError` out of
`exists()` itself — the first fix wrapped only the `read_text` and still exited 1. Rust's
`Path::exists()` reports false on any error, which is the behaviour the three had to agree on.

Pinned by three new CLI fixtures — `tests/cli/030-with-at-scan-depth-limit` (a marker at exactly
depth 5 composes), `tests/cli/031-with-deeper-than-scan-depth-rejected` (depth 6 does not) and
`tests/cli/032-query-without-path-rejected` — plus an `expected.stderr` added to each of the five
existing A2 refusals. The new assertion was itself verified by planting a wrong expectation: all
three harnesses failed the case, so it is an assertion and not decoration. The permission cases are
deliberately NOT fixtures: a mode-000 path cannot be committed to git, so they live in this note and
in the probe scripts instead of pretending to be pinned.

## B2: owner-relative `__file` is not unique, so the base stays and `-w` mounts at the id [[qmd72_finding_file_identity: Finding]]

The plan approved in chat was "`__file` relative to its owning workspace, in every form". Measured
while implementing: in any composed result that value collides, because every workspace's root file
is `readme.qmd.md` by definition, and several consumers key on `__file` alone.

- category: design
- priority: high
- related_to: [[#qmd72]]
- affected_files: [qmdc-rs/src/workspace.rs, qmdc-rs/src/main.rs, qmdc-py/qmdc/workspace.py, qmdc-ts/src/workspace.ts]
- affected_functions: [compose_workspace_roots, rescan_composed_references, compose_with_paths, workspace_to_json]
- solution: Keep `__file` relative to the result's base. Under `-w` the base is virtual and each workspace sits at its own id; the envelope always carries `workspaces` with `{id, root, path}` so a consumer maps any `__file` back to disk. The container form is unchanged.
- test_plan: [[#qmd72_finding_review_conformance]]

### What keys on `__file`, and what the change moved [[qmd72_finding_file_identity_detail: text]]

- about: [[#qmd72_finding_file_identity]]

Keyed on `__file` alone, so an owner-relative value would silently merge two files: the `files`
list; an error's `file`, which is how a reader finds the offending document; the Python and
TypeScript `index.by_file`; and the reference scanner's source-line cache, which suppresses a
reference written inside inline code (a collision reads the other file's line). The MCP rename tool
joins its index root with `__file` to locate each file it edits, so changing the container form
would also have sent its edits to the wrong path.

What moved: under `-w`, `__file` became `<id>/<path in workspace>`; the envelope lost the QMD-59
`workspace` / `workspaces: [ids]` / `workspace: null` shapes; `root` became canonical (a symlinked
`/tmp` now reads the same in all three) and null for `-w`. Fixture movement: one existing expectation
(`tests/cli/029-compose-duplicate-local-id`, an error's `file` gained the id prefix), one harness
reader (the Rust workspace harness read the `workspace` key), three unit tests.
`qmdc-mkdocs` is unaffected: it parses a single workspace, where `path` is `""` and `__file` did not
change.

Rejected earlier in the same discussion as "option C", on the grounds that mkdocs uses `__file` as
a page path. That objection holds only for a form mkdocs never calls.

## A nested workspace composed as a member was reported as an error [[qmd72_finding_nested_member: Finding]]

The owner asked whether this task fixes the long-known `nested_workspace` report on a workspace that
"is not really nested": qmdc-wiki keeps a runtime workspace in `.qmdc/` inside every repository it
models, and the repository root is itself a workspace. Measured on the tg-acp checkout, read-only:
before this change, even `-w . -w .qmdc` reported it, although both were composed and the outer scan
already left `.qmdc/` out.

- category: bug
- priority: medium
- related_to: [[#qmd72]]
- affected_files: [qmdc-rs/src/workspace.rs, qmdc-py/qmdc/workspace.py, qmdc-ts/src/workspace.ts, docs/format/validation-errors.qmd.md]
- affected_functions: [compose_workspace_roots]
- solution: Drop `nested_workspace` for an inner workspace that is itself a member of the composed set, matched by canonical path. Keep it when the inner one is left out, which is the case it exists for.
- test_plan: [[#qmd72_goal_c7]]

### What still reports, and two divergences seen on the way [[qmd72_finding_nested_member_detail: text]]

- about: [[#qmd72_finding_nested_member]]

After the change, on tg-acp: `validate -w . -w .qmdc` is clean in Rust; `validate .` and
`validate -w .` still report `nested_workspace`, correctly, since `.qmdc/` is then missing from the
result. Making the positional form at a workspace root compose nested workspaces automatically is a
format decision, not taken here: it would also swallow the authoring mistake the error catches (a
subfolder marked `__Workspace` where `__Namespace` was meant), which would then surface only as a
`broken_link`.

Two cross-parser divergences surfaced on the same runs. Both are pre-existing, classified by running
the pre-change Python and TypeScript from a `git archive HEAD` tree on the same input, and both are
outside this task:

- On tg-acp, Python reports 148 `nested_subitems` and TypeScript 148 plus one `mixed_field_keys`,
  while Rust reports none. The parse-parity gate covers only this repository's `docs/`.
- `workspace validate .` at this repository's root: TypeScript reports 293 errors, Rust 2 and Python
  0. TypeScript matches `.qmdcignore`'s `tests/*` with a `*` that does not cross `/`; the other two
  treat it as `tests/**`.
