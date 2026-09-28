# QMD-74: an unanchored file gets a different workspace in every parser

## A file outside every workspace has no agreed workspace [[qmd74_unanchored: Bug]]

`docs/format/workspaces.qmd.md:60` states the rule for a file that no anchor covers: `__workspace:
"default"`, `__namespace` not set. No parser does this. Rust and TypeScript leave `__workspace`
unset; Python sets it to the **name of the container directory**, so the same repository produces
different global ids depending on where it is checked out.

Two visible defects follow, both older than QMD-73 and both invisible to the suite, because
parse-output parity is measured with `docs/` as the root — a directory that IS a workspace, so no
file in that corpus is unanchored.

Found while closing QMD-73's review gate, which recorded the two symptoms without their cause.

- status: planned
- priority: medium
- category: parser
- related_task: [[#qmd73_qmdcignore]]
- requires_changes: []
- findings: [[#qmd74_finding_spec]], [[#qmd74_finding_ambiguous]], [[#qmd74_finding_chain]], [[#qmd74_finding_wrapped_field]], [[#qmd74_finding_questions]], [[#qmd74_finding_tests]]
- result: null

### Goals [[goals: [Goal]]]

#### A1: One workspace for an unanchored file, in all three parsers [[qmd74_goal_a1]]

A `.qmd.md` file that no `__Workspace` anchor covers gets the same `__workspace` in Rust, Python and
TypeScript, and that value is the one the format spec names. The value is an operator decision (see
`[[#qmd74_finding_questions]]` Q1) because the spec's `"default"` is currently implemented by
nobody: Rust and TypeScript leave it unset, Python uses the container directory's name. Whichever
value is chosen, no parser may derive it from a path outside the repository, so the same checkout
produces the same graph under any directory name.

- group: A_unanchored
- done: false

#### A2: An unqualified reference does not reach an unanchored object [[qmd74_goal_a2]]

A `[[#id]]` written inside a workspace resolves only within that workspace. It must not also match
an object in a file outside every workspace, so two files that declare the same id — one in the
workspace, one unanchored — do not make the reference ambiguous. Rust reports exactly this
ambiguity at the repository root today, against its own documented resolver rule.

- group: A_unanchored
- done: false

#### B1: Every nested workspace in a chain is reported [[qmd74_goal_b1]]

`nested_workspace` fires once for each workspace nested below the root, not only for the first one.
Three workspaces in a chain produce two errors in all three parsers; Python produces one, because
its scan stops descending at the first nested workspace it finds.

- group: B_nesting
- done: false

#### C1: The parity corpus covers an unanchored file [[qmd74_goal_c1]]

The three-parser parity check exercises at least one root that is a container rather than a
workspace, so a divergence in `__workspace`, in `__global_id` or in the resolution of an unqualified
reference fails the suite instead of surviving in it. Both defects above lived through every release
because the corpus root is always `docs/`.

- group: C_coverage
- done: false
