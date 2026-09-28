# QMD-74: Findings

## The spec names a value no parser writes [[qmd74_finding_spec: Finding]]

`docs/format/workspaces.qmd.md:60` — "If no anchor found: `__workspace: "default"`, `__namespace`
not set". Measured on a container holding one workspace and one unanchored file
(`docs/readme.qmd.md` declaring `[[wsx:__Workspace]]`, plus `lib/orphan.qmd.md` declaring
`[[orph]]`), parsed with the container as the root:

| parser | `__workspace` of `orph` | `__global_id` of `orph` | `thing` (inside the workspace) |
|---|---|---|---|
| spec | `"default"` | — | — |
| rs | unset | `::orph` | `wsx::thing` |
| ts | unset | not computed at parse level | `wsx::thing` |
| py | `"renamed_root"` | `renamed_root::orph` | `wsx::thing` |

All three agree on the anchored object and none matches the spec on the unanchored one. Python's
value is the **container directory's name**: the fixture above lives in a directory called
`renamed_root`, and in this repository's own worktree the same object comes out as
`qmd-69::qmdc_guide`, after the worktree directory. That makes the graph depend on the checkout
location, the invariant QMD-72's B2 finding was written to protect.

- category: parser
- related_to: [[#qmd74_unanchored]]
- solution: Decide the value once (Q1), then write it in all three parsers at the point where a file fails to find an anchor. Pin it with a container-root fixture so the parity corpus covers it.

## The spec line covers two different shapes, and all three parsers already agree on one of them [[qmd74_finding_two_shapes: Finding]]

"If no anchor found" reads as one rule but names two situations, and the parsers treat them
differently. Measured on two minimal fixtures, `workspace parse` with the directory as the root:

```markdown example
shape 1 — no __Workspace anywhere under the root
  case1/a.qmd.md  [[alpha: Thing]]  - to: [[#beta]]
  case1/b.qmd.md  [[beta: Thing]]

rs: workspaces=[{id: "case1"}]  alpha.__workspace="case1"  beta.__workspace="case1"  errors=0
ts: workspaces=[{id: "case1"}]  alpha.__workspace="case1"  beta.__workspace="case1"  errors=0
py: workspaces=[{id: "case1"}]  alpha.__workspace="case1"  beta.__workspace="case1"  errors=0

shape 2 — a workspace exists, one file lies outside it
  case2/ws/readme.qmd.md   [[docs_ws: __Workspace]]  - to: [[#stray_thing]]
  case2/stray/loose.qmd.md [[stray_thing: Thing]]

rs: workspaces=[{id: "docs_ws"}]  stray_thing.__workspace=unset  errors=0
ts: workspaces=[{id: "docs_ws"}]  stray_thing.__workspace=unset  errors=1 broken_link
py: workspaces=[{id: "docs_ws"}]  stray_thing.__workspace="case2"  errors=1 broken_link
```

In shape 1 all three synthesise a workspace and name it after the **directory the parse was pointed
at**, and that synthetic workspace appears in the `workspaces` list as if it had been declared. So the
path-dependent identity is not Python's alone: rename the checkout and every `__global_id` in an
unanchored tree changes, in every parser. Shape 1 is also the shape the spec line most plainly
addresses, and none of the three writes `"default"` there either.

Shape 2 is where the parsers split, and it is the eight-line reproduction of
[[#qmd74_finding_ambiguous]]: only Rust resolves the workspace's unqualified reference into the file
outside it. Python still reports a directory name for the stray, and that name is absent from its own
`workspaces` list — a `__workspace` pointing at no `__Workspace` object, which
`docs/format/workspaces.qmd.md:52` defines as a reference to one.

- category: parser
- related_to: [[#qmd74_unanchored]]
- solution: Split the spec line into the two shapes and answer them separately — a root with no anchor needs a path-independent id, a file outside an existing workspace needs a scope that reaches nothing.

## Rust resolves an unqualified reference into an unanchored file [[qmd74_finding_ambiguous: Finding]]

`workspace validate` at this repository's root, all three parsers, measured:

```text
rs: errors=2 ambiguous=2
    docs/mcp/discovery.qmd.md:86  Ambiguous reference '#qmdc_guide' - multiple objects match
    docs/mcp/resources.qmd.md:15  Ambiguous reference '#qmdc_guide' - multiple objects match
py: errors=0
ts: errors=0
```

The id `qmdc_guide` is declared twice: in `docs/guides/qmdc-guide.qmd.md`, inside workspace `docs`
under namespace `guides`, and in `qmdc-rs/src/qmdc-guide.qmd.md`, the copy the Rust crate embeds in
its binary, which lies outside every workspace. All three parsers scan both files — the file lists
at the repository root are identical, 118 paths each — so the divergence is in resolution, not in
discovery.

The two referrers are inside workspace `docs` and write the reference unqualified. Rust matches both
candidates, because the unanchored object's empty workspace segment does not exclude it; Python
matches only `docs:guides:qmdc_guide`. Rust's own resolver rule, recorded in
`tests/workspace/container-root-single-workspace/tests/001-reference-to-workspace-root.sql`, says an
empty-workspace object is admitted "only when the looked-up id IS the workspace's own name" —
`qmdc_guide` is not a workspace name, so the intended rule already excludes this match.

The admission test is `workspace_matches` at `qmdc-rs/src/core/reference_scan.rs:161`. For an
unqualified reference it returns `obj_workspace.is_empty() || cand_workspace.is_empty() ||
cand_workspace == obj_workspace`, so an empty workspace is a **wildcard in both directions**: an
unanchored object is a candidate for a reference made from anywhere, and an unanchored object's own
reference reaches every workspace. Rust's graph path already spells the intended rule correctly —
`db/mod.rs:725` admits an empty-workspace row only via `(__workspace = '' AND __id = ?1)`, the id
being the workspace's own name — so the two paths inside one parser disagree, and the narrow one is
right. The empty value carries three meanings at once today: "I am a workspace", "I have no
workspace", and "match anything".

CHANGELOG (QMD-69) states the contract this violates: "A reference that crosses a workspace boundary
MUST now be qualified; an ambiguous qualified reference produces no edge rather than an arbitrary
one."

- category: parser
- related_to: [[#qmd74_unanchored]]
- solution: Fix the admission test in the Rust resolver so an unanchored object is not a candidate for an unqualified lookup made from inside a workspace. Depends on A1, because what "unanchored" looks like in the index changes with the value chosen there.

## Python stops descending at the first nested workspace [[qmd74_finding_chain: Finding]]

Three workspaces in a chain, each directory inside the previous one, validated with the outermost as
the root:

```markdown example
fixture: readme.qmd.md [[chain_a: __Workspace]]
         inner/readme.qmd.md [[chain_b: __Workspace]]
         inner/deeper/readme.qmd.md [[chain_c: __Workspace]]

rs: nested_workspace=2 -> ['inner/readme.qmd.md', 'inner/deeper/readme.qmd.md']
ts: nested_workspace=2 -> ['inner/readme.qmd.md', 'inner/deeper/readme.qmd.md']
py: nested_workspace=1 -> ['inner/readme.qmd.md']
```

Control, on the repository's existing single-nesting fixture
`tests/workspace/nested-workspace-error`, which expects one such error: all three report exactly 1.
So the divergence appears only from the second level of nesting down, and no existing fixture has a
second level.

`nested_workspace` exists to tell the operator which files were left out of the result. Reporting
only the first one under-reports what was skipped: `inner/deeper` is equally absent from the graph
and equally unreported.

- category: parser
- related_to: [[#qmd74_unanchored]]
- solution: Keep descending past a nested workspace in the Python scan and report every marker below the root, as Rust and TypeScript do. Pin with a chain fixture; the existing single-level fixture cannot catch it.

## Noticed while writing this file, out of scope: a wrapped field value means three things [[qmd74_finding_wrapped_field: Finding]]

A field whose value continues on the next line is parsed three different ways, and one parser loses
the continuation silently. Minimal document, eight lines:

```markdown example
# Doc

## Thing [[w_thing: Section]]

Body.

- solution: first line of the value
  second line of the value
```

The `solution` field, each parser:

| parser | value |
|---|---|
| rs | `first line of the value second line of the value` (joined with a space) |
| py | `first line of the value\nsecond line of the value` (joined with a newline) |
| ts | `first line of the value` (continuation dropped) |

Found because the first draft of this very file wrapped three `- solution:` values, and
`make validate-compare` reported the document as divergent across all three parsers. The values are
now on single lines, so the corpus is exact again; the defect is untouched.

This is the QMD-71 class — the same construct read differently by each parser — and it is worse than
a formatting difference, because the TypeScript reading discards authored content with no error. It
is unrelated to unanchored workspaces, so it does not belong to this task.

- category: parser
- related_to: [[#qmd74_unanchored]]
- solution: A separate task. Decide first whether a wrapped field value is legal QMD at all; if it is not, the answer is a parsing error in all three rather than three readings.

## The same construct in comment content: TypeScript strips the continuation's indent [[qmd74_finding_wrapped_comment: Finding]]

The sibling of the finding above, in the other half of the parser. A list item inside COMMENT content
— the body of a heading that declares no object — may also continue on an indented next line, and
there TypeScript does not drop the continuation, it drops the continuation's INDENT. Minimal
document, seven lines:

```markdown example
# T

## Checklist

- [x] short item
- [ ] wrapped item that continues
  on the next line, indented two spaces
```

The `content` of the unnamed `text_1` object, each parser:

| parser | last two lines of `content` |
|---|---|
| rs | `- [ ] wrapped item that continues\n  on the next line, indented two spaces` |
| py | identical to rs |
| ts | `- [ ] wrapped item that continues\non the next line, indented two spaces` |

Found the same way as its sibling, one task later: the QMD-61 checklist gained three wrapped `- [x]`
bullets and `make validate-compare` reported the document divergent, `rs+py vs ts`. The bullets are
now on single lines, so the corpus is exact again; the defect is untouched.

Two reasons it is worth its own line rather than a footnote to the finding above. The grouping is
different — a wrapped FIELD value splits all three parsers, a wrapped COMMENT line splits ts from a
rs+py majority — so they are two defects, not one seen twice, and a fix for either does not touch
the other. And the consequence is different in kind: dropping the indent does not lose characters,
it changes what the Markdown MEANS, because two-space indentation is what makes the line a
continuation of the list item rather than a new paragraph. A `rebuild` of the TypeScript reading
emits a document that renders differently from the one that was parsed.

- category: parser
- related_to: [[#qmd74_finding_wrapped_field]]
- solution: Fold into the same separate task as the wrapped field value — both are "what does an indented continuation line mean", one in a field value and one in comment content, and both should be decided once.

## The nested-workspace rule and the synthesized root are one question [[qmd74_finding_nested_virtual: Finding]]

Three shapes, `workspace validate` on each with all three parsers:

| shape | result |
|---|---|
| container root, no file of its own, one declared sub-workspace | all three: `[]`, the sub-workspace's two objects in the graph |
| the same container plus one stray `.qmd.md` at its root | all three: `[]`; the stray is in the graph, `__workspace` empty in rs and ts, the directory name in py; nothing reports it |
| declared root workspace with a declared workspace inside it | all three: `nested_workspace` on the inner marker |

So the rule fires on a declared workspace inside a declared workspace, and nowhere else. A container
is not a workspace today, which is the contract QMD-59 decided for it: validate each contained
workspace independently rather than calling the container's members nested.

That contract is what makes Q1a's answer collide with the rule. The moment a root with no anchor
becomes a workspace, every declared workspace under it is nested inside one, so the rule as written
would report a finding on every repository that keeps its documents in a subdirectory — the common
case. The rule therefore has to say *declared* inside declared: nesting inside a synthesized workspace
is not a finding, because the synthesized root does not exclude its members, it contains them.

The message QMD-76 rewrote is still accurate for the case that remains: a declared workspace inside a
declared workspace really is excluded from the outer graph, and composition is the cure. What that
does not cover is a tool-owned directory such as `.qmdc`, which sits inside a declared root in every
initialised repository and is reported on every validate — that needs either exclusion from discovery
like `.git`, or a way for the root to name its members, and it is a separate decision from Q1.

- category: parser
- related_to: [[#qmd74_unanchored]]
- solution: Write the nested-workspace rule as "declared inside declared" before implementing Q1a, and pin both halves with fixtures — a synthesized root holding a declared workspace must stay clean, a declared root holding one must still report.

## Open questions [[qmd74_finding_questions: Finding]]

**Q1 — what workspace does an unanchored file belong to?** The measurement in
[[#qmd74_finding_two_shapes]] splits this in two, and each half has its own answer.

**Q1a — a root with no anchor anywhere.** All three already agree: one synthetic workspace named
after the directory the parse was pointed at. The shape is right and the name is not, because the
identity of every object then changes when the checkout is renamed — the invariant QMD-72's B2
finding protects. The spec's intent (`"default"`) is the right shape; its spelling is unsafe, because
`default` is a legal user workspace id and a user who declares `[[default: __Workspace]]` would merge
with every unanchored tree. The format reserves system names with a `__` prefix (`__Workspace`,
`__Namespace`, `__TextBlock`), so a reserved `__default` is the spelling consistent with the format,
and the human-readable location stays in the `workspaces` entry's `root`/`path`, where it already is.

**Q1b — a file outside an existing workspace.** `"default"` is the wrong answer here whatever it is
spelled: pooling every stray under one name gives two files that share no directory a shared
reference scope, so they resolve each other's bare ids and collide as `duplicate_id`, while the
spec defines a workspace as "a directory of QMD.md files that can reference each other". Three
candidates:

1. **Unset and unreachable**, which is what Rust and TypeScript already write and what Python and
   TypeScript already enforce (`broken_link` both ways). Needs the Rust wildcard fixed (A2) and the
   spec line split. Smallest change; leaves a stray silently in the graph.
2. **Unset and unreachable, plus a new workspace-level diagnostic** naming each file that fell
   outside every workspace. Same resolution semantics as 1, but the stray stops being silent — this
   repository's own embedded copy sat in the graph unnoticed until an ambiguity surfaced it.
3. **Left out of the result and reported**, exactly as `nested_workspace` already does for files
   missing from the result. Cleanest semantically and the only one that makes a stray impossible to
   query; it drops objects the three parsers return today.

Whatever wins, the empty `__workspace` value keeps meaning "I am a workspace" for a `__Workspace`
root object, so the two meanings must be told apart by `__kind`, not by the field.

**Q2 — should the embedded guide copy be a scanned file at all?** `qmdc-rs/src/qmdc-guide.qmd.md`
exists to be compiled into the Rust binary, not to be part of any graph. Excluding it — a
`.qmdcignore` line, or moving it out of the scanned tree — removes the duplicate id that surfaces
this bug at the repository root, but it does NOT fix the class: any unanchored file with an id that
a workspace also uses reproduces it. Decide whether to do it anyway, as hygiene, in addition to the
fix.

- category: parser
- related_to: [[#qmd74_unanchored]]
- solution: Answer Q1 before implementation; Q2 is independent and optional.

## Test plan [[qmd74_finding_tests: Finding]]

**Existing tests — what covers this today:** nothing. Parse-output parity and the validation-error
comparison both run with `docs/` as the root, and `docs/` is a workspace, so no file in the corpus
is unanchored. `tests/workspace/container-root-single-workspace` does use a container root, but its
only unanchored object is the workspace root object itself, whose empty segment is documented and
expected. `tests/workspace/nested-workspace-error` has one level of nesting, and the divergence
starts at the second.

**New tests needed, all data-driven workspace fixtures:**

1. `tests/workspace/nested-workspace-chain/` — three workspaces in a chain, root is the outermost.
   Expects two `nested_workspace` errors. Fails in Python only (reports one) and is the mandatory
   regression for `[[#qmd74_finding_chain]]`.
2. `tests/workspace/container-root-unanchored-file/` — a container holding one workspace plus an
   unanchored `.qmd.md` file. A SQL case pins the unanchored object's `__global_id` to the value Q1
   settles on, so all three parsers must agree and the value cannot depend on the directory name.
3. Same fixture, second SQL case: the workspace file and the unanchored file both declare one id,
   and a reference inside the workspace written unqualified resolves to exactly one edge — the
   workspace's own object — with no `ambiguous_reference`. Fails in Rust today.
4. Extend the parity corpus (goal C1) so at least one container root is compared across the three
   parsers, rather than only `docs/`.

**How to verify:** each new fixture must fail before implementation for its documented reason and in
the documented parser only — recorded per fixture, then re-run after the fix. For fixture 2 the
pre-implementation failure is a three-way disagreement, so assert the chosen value and confirm all
three parsers move to it.

- category: testing
- related_to: [[#qmd74_unanchored]]
- solution: Write fixtures 1-3 during triage, record their failures, and leave 4 for implementation.
