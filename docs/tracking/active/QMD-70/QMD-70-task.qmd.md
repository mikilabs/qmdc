# QMD-70: A table inside an array element is captured by the parent array

## Table-to-array conversion is not scoped to the array container [[qmd70_table_scope: Bug]]

A Markdown table written as ordinary content inside an object-array ELEMENT is parsed as
if it stood directly under the array heading, so its rows become sibling elements of that
array. The documented syntax puts the table immediately under the array heading; nothing
closes that context when an element heading opens, so every table deeper inside the
array's subtree is captured too. Consequences diverge by implementation and by the shape
of the element: at best the array gains phantom elements labelled after table cells, at
worst an object with an explicit id disappears from the parse entirely (Rust) or the array
is terminated and following siblings leak out as plain fields with their Kind degraded
(Python, TypeScript). Every outcome is SILENT — no `__ParsingError`, no validation error.

Found while authoring the QMD-69 task document (cross-workspace reference qualifiers, GitHub
issue #9): a comparison table inside that task's goal A2 replaced the goal with four anonymous
ones labelled after the table's first column. The two tasks share nothing but the file the bug
was found in — different code path, different symptom, different fix — so this one is tracked
separately even though both land on the same branch.

- status: in_progress
- priority: high
- category: parser
- related_task: [[#qmd69_cross_ws_refs]], [[#qmd66_dot_notation_discrepancies]]
- requires_changes: []
- findings: [[#qmd70_finding_scope]], [[#qmd70_finding_matrix]], [[#qmd70_finding_id_compose]], [[#qmd70_finding_surfaces]], [[#qmd70_finding_prose_gap]], [[#qmd70_finding_two_tables]], [[#qmd70_finding_rs_after_array]], [[#qmd70_finding_rs_other_paths]], [[#qmd70_finding_blockquote]], [[#qmd70_finding_table_in_array]], [[#qmd70_finding_extra_table]], [[#qmd70_finding_mixed_array]], [[#qmd70_finding_indented_block]], [[#qmd70_finding_no_diagnostic]], [[#qmd70_finding_red_suite]], [[#qmd70_finding_rebuild_anchor]], [[#qmd70_finding_hdr_only]], [[#qmd70_finding_tests]], [[#qmd70_finding_questions]]
- result: [[#qmd70_result]]

### Reproduction [[qmd70_repro: text]]

- about: [[#qmd70_table_scope]]

A typed object array whose element contains prose and then a table. This is the exact
shape that destroyed goal A2 in QMD-69:

```text
## Bug [[bug: Bug]]

### Goals [[goals: [Goal]]]

#### A2 [[goal_a2]]

Prose before the table.

| form | means |
| --- | --- |
| `id` | this workspace |
| `ns:id` | that namespace |

Prose after the table.

- group: A
- done: false
```

Run `qmdc parse -i <file>` and list the `Goal` objects.

**Expected** — one Goal, `goal_a2`, carrying the table as text content, exactly as a
table inside any other prose position is carried.

**Actual (Rust)** — `goal_a2` does not exist. In its place are `goals_0` and `goals_1`,
labelled `` `id` `` and `` `ns:id` `` — the table's first column. The heading, its explicit
id and all of its fields are gone.

**Actual (Python, TypeScript)** — `goal_a2` survives here, but the same file in other
shapes gains phantom elements or loses the NEXT sibling; see [[#qmd70_finding_matrix]].

### Why this is a defect and not the documented feature [[qmd70_intent: text]]

- about: [[#qmd70_table_scope]]

Tables as a compact object-array syntax are intentional and specified in
`docs/format/arrays.qmd.md`: "Tables provide a compact alternative for homogeneous object
arrays… First row = field names, each data row = one object." The spec's example places
the table DIRECTLY under the array heading with no element heading in between:

```text
### Members [[members: [User]]]

| name | role |
| --- | --- |
| Alice | admin |
```

So the feature is right and only its SCOPE is wrong. The array context must end where the
array's own content ends — when an element heading opens, a table inside that element is
the element's content, not the array's. Nothing in the spec suggests otherwise, and no
fixture in the corpus places a table inside an element.

### Root cause (diagnosed) [[qmd70_rootcause: text]]

- about: [[#qmd70_table_scope]]

One cause with three divergent symptoms. In Rust the array context lives in
`pending_object_array` (`qmdc-rs/src/parser.rs:242`), set when an array heading is opened
(`parser.rs:827`, `parser.rs:875`). When a deeper heading is found it becomes an array
ELEMENT (`parser.rs:884` onward) and `pending_object_array` is deliberately left set,
because the next sibling element still needs it. The table branch
(`parser.rs:2664`) then tests only `if let Some(...) = pending_object_array` — it cannot
tell "table under the array heading" from "table inside an element", so it converts the
rows and `continue`s without finalizing the element being built. `qmdc-py` and `qmdc-ts`
carry their own equivalent of the same state and the same missing distinction, which is
why all three are wrong and wrong differently.

### Goals [[goals: [Goal]]]

#### A1: Scope the array context to the array's own content [[qmd70_goal_a1]]

A table converts to array elements only while the parser is positioned in the array
container itself. Once an element heading has opened, a table belongs to that element and
must be treated exactly as a table in any other prose position — carried as text content,
with the element, its id and its fields intact.

Keep the documented case working: a table directly under `### Members [[members: [User]]]`
still produces one object per data row.

**Done (2026-09-23), all three implementations.** The table branch now fires only while
positioned in the array CONTAINER, which the current object distinguishes: it is the array's
parent in the container and the element itself once an element heading has opened. Everything
else falls through to the comment path, whose `pending_object_array.is_none()` guard was removed
so it can be reached from inside an array subtree.

Implementation found a SECOND cause the triage had missed, and it is what made shapes A and D
lose the element rather than merely gain a phantom: Rust's `has_table_after` lookahead
skips paragraphs, so an element written as prose-then-table matched
the "bare `[[id]]` followed by a table" table-field pattern and was claimed as a table FIELD of
the grandparent — its explicit id became an empty array and its fields were dropped. That
lookahead now declines any heading which would be an array element.

- group: A_scope
- done: true

#### A2: No silent loss, ever [[qmd70_goal_a2]]

Whatever the resolution of the open questions, the parse must never drop an object that
carries an explicit id, and must never silently move a heading out of the array it was
written in. If a construct is genuinely unsupported, it has to surface as a
`__ParsingError` rather than as a quietly different graph.

**Done (2026-09-23).** Follows from A1: the element is no longer claimed by either the array
branch or the table-field lookahead, so nothing with an explicit id disappears. No
`__ParsingError` was needed — the construct turned out to be representable, not unsupported.

- group: A_scope
- done: true

#### B1: Parity across the three parsers [[qmd70_goal_b1]]

All three implementations agree on every shape in the characterization matrix
([[#qmd70_finding_matrix]]). Today no two of them agree on all four shapes, and
`make validate-compare` does not catch it because no fixture puts a table inside an array
element.

**Done (2026-09-23).** All four shapes now agree across the three implementations. Python needed
only the container check; TypeScript needed that plus the same unblocking of its comment path,
which had refused tables anywhere inside an array subtree.

- group: B_parity
- done: true

#### B2: Consistent id composition for table children [[qmd70_goal_b2]]

Rust and the other two compose a table child's hierarchical id differently when the array
field name equals the parent object's id — see [[#qmd70_finding_id_compose]]. One rule,
and the same rule table children and heading elements already share.

**Done (2026-09-23).** Rust now composes a table child's id exactly as Python's documented
`resolve_child_id` does, including the case where the array field name IS the parent's id (a
top-level array) and the dot-ID case. Pinned by
`tests/workspace/table-child-id-composition/001-table-child-id-field-equals-parent`, which no
fixture covered before — asserted through the SQL harness because this shape's `parse -> rebuild`
is not round-trip stable, a separate pre-existing limitation.

- group: B_parity
- done: true

#### C1: Regression tests for every shape, on every surface [[qmd70_goal_c1]]

Data-driven, covering the four shapes of the matrix on the PARSE surface plus the same defect
as it shows on `qmdc workspace validate`, the LSP and MCP — where a lost element turns a
reference to it into a false `broken_link` and the three parsers disagree about whether the file
is valid ([[#qmd70_finding_surfaces]]). Written during triage per the Bug triage exception and
failing as designed; see [[#qmd70_finding_tests]] for the measured columns.

The documented feature needs no new guard: shipped `tests/parser/032-table` (table under the
array heading) and `tests/parser/065-text-table-in-array` (table inside an element, wrapped in
a declared `text` field) already pin it and must stay green.

**Done (2026-09-23).** Eight cases: the four shape microtests, the validator/LSP/MCP trio from
[[#qmd70_finding_surfaces]], and the id-composition case. All green in every implementation that
runs them, and the five shipped table fixtures (`032`, `042`, `064`, `065`, `089`) stay green.

- group: C_tests
- done: true

#### D1: State the scope rule in the format spec [[qmd70_goal_d1]]

`docs/format/arrays.qmd.md` says what a table means under an array heading but not where
that meaning stops. Say explicitly that the table must be the array container's own
content, that a table inside an element is ordinary content carried as a comment, and that a
declared `text` field is how to attach one deliberately — the form
`tests/parser/065-text-table-in-array` already relies on.

**Done (2026-09-23).** `docs/format/arrays.qmd.md` now states that the table must be the array
container's own content, shows the element case explicitly, and points at the declared `text`
field as the way to attach a table to an element deliberately.

- group: D_docs
- done: true
