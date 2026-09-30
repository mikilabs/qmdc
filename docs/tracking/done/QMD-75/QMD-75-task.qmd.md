# QMD-75: a heading body that is not a field gives three different object trees

## Non-field bodies make object identity parser-dependent [[qmd75_body_identity: Bug]]

An object heading whose body holds a block that is not a field — a prose paragraph, or a `---`
thematic break — before a deeper heading produces a different set of objects in each parser. Rust
keeps the object and nests the child under it; Python drops the object **and its whole subtree**;
TypeScript drops the object and **re-parents** the child onto the grandparent. So the same file
yields different `__id` values, and a reference that resolves in one parser is broken in another.

Measured on a real-world corpus instead of the repository's own docs: `tg-acp` at `a654ce5`, 250
`*.qmd.md`, 240 scanned, 1136 objects. On that tree `workspace validate` reports 1 error in Rust,
149 in Python and 150 in TypeScript, and all three scan the same 240 files, so none of it is a scan
divergence. Python emits 1136 objects over 1134 distinct ids and TypeScript 1137 over 1135 — each
silently collapses two objects onto one id and reports no `duplicate_id`.

Three more divergences on the same corpus, two of them the same root cause: TypeScript reads a prose
bullet as a field where Rust and Python read prose, `nested_subitems` fires on mirror-image shapes
(Rust on a top-level object only, Python and TypeScript on array elements only), and Rust omits the
top-level `index` key that the other two emit.

Invisible to the suite because the three-parser parity corpus is `docs/`, where every object's body
is a field list. Found while looking into the error a user hit running `workspace validate` on a
repository (see `[[#qmd76_nested_message]]` for that error, which is unrelated).

- status: done
- priority: high
- category: parser
- related_task: [[#qmd74_unanchored]]
- requires_changes: []
- findings: [[#qmd75_finding_body]], [[#qmd75_finding_kind_decides]], [[#qmd75_finding_content_gate]], [[#qmd75_finding_ts_startline]], [[#qmd75_finding_ts_hr_levels]], [[#qmd75_finding_py_mixed]], [[#qmd75_finding_subitems]], [[#qmd75_finding_shape]], [[#qmd75_finding_questions]], [[#qmd75_finding_tests]], [[#qmd75_finding_residuals]], [[#qmd75_finding_review]]
- result: [[#qmd75_result]]

### Goals [[goals: [Goal]]]

#### A1: One object tree for a non-field body [[qmd75_goal_a1]]

A heading with a prose body, and a heading with a `---` body, produce the same objects with the same
`__id` values in Rust, Python and TypeScript, and a deeper heading below such a body nests under it
in all three. The correct tree is a spec decision (`[[#qmd75_finding_questions]]` Q1) because the
format does not currently say what a heading's body is when it is not a field list; whichever answer
is chosen, no object may be dropped in one parser and kept in another.

- group: A_identity
- done: true

#### A2: Two objects never collapse onto one id in silence [[qmd75_goal_a2]]

When two objects would resolve to the same global id, every parser reports `duplicate_id`. Today
Python and TypeScript each produce a colliding pair on `tg-acp` and report nothing, so the graph
holds an object that no reference can reach and no diagnostic names.

- group: A_identity
- done: true

#### A3: A prose bullet is prose in all three [[qmd75_goal_a3]]

A bullet whose text happens to contain a colon, written as prose under an id-less heading, is prose
in every parser. TypeScript currently promotes the first such bullet to a field and then reports
`mixed_field_keys` on the next one, which is the same field-or-prose decision as A1.

- group: A_identity
- done: true

#### B1: nested_subitems fires on the same shape everywhere [[qmd75_goal_b1]]

A field carrying indented sub-items is reported identically wherever it sits: on a file's top-level
object, on a plain subobject, and on an element of an object array. Rust reports only the first case
and Python and TypeScript only the last two, so on `tg-acp` the same 148 array elements are 148
errors in two parsers and none in the third.

- group: B_diagnostics
- done: true

#### C1: parse output carries the same top-level keys [[qmd75_goal_c1]]

`workspace parse` returns the same top-level keys in all three parsers. Rust omits `index`, which
Python and TypeScript both emit, so a consumer written against one parser reads a missing key from
another.

- group: C_shape
- done: true

#### D1: the parity corpus covers bodies that are not fields [[qmd75_goal_d1]]

The three-parser parity check exercises a prose body, a `---` body, a prose bullet with a colon, and
a field with sub-items on all three positions, so any divergence in objects, ids or diagnostics fails
the suite. All five divergences above survived every release because the corpus is `docs/`, whose
objects always carry field bodies.

- group: D_coverage
- done: true
