# QMD-77: every remaining three-parser divergence, closed in one pass

## The known divergences, all of them, in one task [[qmd77_tails: Bug]]

Fifteen divergences between Rust, Python and TypeScript survive in the repository, carried forward
from QMD-71, QMD-72, QMD-73, QMD-74 and QMD-75 as measured residuals. None was caused by the task
that recorded it, each was confirmed by running the pre-change binaries, and every one of them means
the same file produces a different graph depending on which parser read it.

They are gathered here deliberately: individually each looked small enough to defer, and deferring
them is how the list grew to fifteen. Four of them need a FORMAT decision before any code is written
(see `[[#qmd77_finding_questions]]`), because the three parsers disagree and none of the three answers
is written down anywhere.

This task closes the list. It is the last residual-collection task; anything found after it gets its
own task and is not deferred into a list again.

- status: planned
- priority: high
- category: parser
- related_task: [[#qmd75_body_identity]]
- requires_changes: []
- findings: [[#qmd77_finding_identity]], [[#qmd77_finding_content]], [[#qmd77_finding_contract]], [[#qmd77_finding_robustness]], [[#qmd77_finding_fidelity]], [[#qmd77_finding_questions]], [[#qmd77_finding_tests]]
- result: null

### Goals [[goals: [Goal]]]

#### A1: A duplicate id is kept and reported the same way everywhere [[qmd77_goal_a1]]

Two objects that resolve to the same global id produce the same object set and the same diagnostics
in all three parsers. Measured on a nested pair: Rust emits five objects and two `duplicate_id`,
Python four and one, TypeScript three and **none at all** — so TypeScript silently discards an object
that the other two report. Rust's ordering of the duplicated subtrees is also inverted against the
document.

- group: A_identity
- done: false

#### A2: A wrapped field value means one thing [[qmd77_goal_a2]]

A field whose value continues on an indented next line parses to the same value in all three. Today
Rust joins the lines with a space, Python with a newline, and TypeScript keeps only the first line and
drops the rest in silence. Needs `[[#qmd77_finding_questions]]` Q1 first: the format does not say
whether a wrapped value is legal at all.

- group: A_identity
- done: false

#### A3: A declared array or map treats a deeper declaration the same way everywhere [[qmd77_goal_a3]]

`[[field: array]]` and `[[field: map]]` followed by a deeper heading that declares an object give the
same result in all three. Today Rust swallows the declaration into the array as a string element (or
into the map as an entry) and the object disappears; Python and TypeScript keep the object. QMD-75
settled this principle for `text` only. Needs Q2.

- group: A_identity
- done: false

#### A4: An indented sub-item is one thing in all three [[qmd77_goal_a4]]

A field carrying indented sub-items resolves identically whether or not a sub-item's own text happens
to look like `key: value`. Rust turns such a sub-item into a field of its own and loses the field it
was indented under; Python and TypeScript report `nested_subitems` and keep the rest. This is the case
QMD-71 left open deliberately, and it is the entire remaining error gap on the external corpus — 125
reports against 148. Needs Q3.

- group: A_identity
- done: false

#### B1: Non-field content above the first heading is preserved identically [[qmd77_goal_b1]]

The `__TextBlock` synthesised for content before a document's first heading holds the same value in
all three. Today Rust keeps only a code fence where Python and TypeScript keep prose, table and fence,
and all three drop a `---` entirely. Needs Q4.

- group: B_content
- done: false

#### B2: A wrapped list item keeps its shape in every carrier [[qmd77_goal_b2]]

A list item that continues on an indented next line parses to the same value wherever it sits — in
text-block content, in an object's body, in a comment. Three carriers are measured and all three
groupings appear: TypeScript strips the continuation's indent in text-block content, Rust joins the
lines in an object's body, and the field-value carrier is the three-way split of `[[#qmd77_goal_a2]]`.
Whatever Q1 decides applies to every carrier, with a fixture for each.

- group: B_content
- done: false

#### C1: System objects are synthesised on the same files [[qmd77_goal_c1]]

The `__Document` and `__TextBlock` objects a parser synthesises appear for the same files in all
three. Rust emits neither for a file whose content is entirely YAML front matter, where the other two
emit both.

- group: C_contract
- done: false

#### C2: A synthesised id is unique within the graph [[qmd77_goal_c2]]

No two objects in one parse result carry the same `__id`. Today two different files both get
`doc_ry4ljv` and `text_0`; they carry no `__global_id`, so nothing can reference them, but a graph
should not hold two objects under one id and report nothing.

- group: C_contract
- done: false

#### C3: An error names its object the same way in both commands [[qmd77_goal_c3]]

`workspace parse` and `workspace validate` describe an error's owning object with the same keys. Parse
says `object`; validate says `objectId` and adds `fieldName`. All three parsers agree within each
command, so this is one contract decision, not a parser bug — and it is a breaking change for a
consumer either way.

- group: C_contract
- done: false

#### C4: The remaining diagnostic details agree [[qmd77_goal_c4]]

Three small disagreements, each one line of output: the line number in `mixed_field_keys` (Rust points
at the offending item, the other two at the item above it, which has no colon); the count and line of
`invalid_map_content` (Rust one at the declaration, the other two at the offending line); and
`__has_explicit_id: false` on an object-array element with no `[[id]]`, which only Rust emits.

- group: C_contract
- done: false

#### C5: A comment anchor names a field that exists [[qmd77_goal_c5]]

The `after` anchor of a comment names a field of the object it sits on. After a `nested_subitems`
error Rust anchors the following comment on the offending field name, which no parser keeps as a
field, so the anchor dangles.

- group: C_contract
- done: false

#### C6: The parse envelope is pinned by a fixture [[qmd77_goal_c6]]

A `tests/workspace` fixture asserts the `workspace parse` envelope: `index` present with
`by_global_id`, `by_kind`, `by_file` in snake_case, the `errors` block's keys, and `workspaces` with
`{id, root, path}`. QMD-75 aligned these across the three parsers and nothing pins them, so the next
divergence in the envelope is invisible to the suite.

- group: C_contract
- done: false

#### D1: An unreadable directory does not lose the readable files [[qmd77_goal_d1]]

A workspace scan that meets a directory it cannot read skips it and returns everything else, in all
three. TypeScript's `scanWorkspace` throws an uncaught `EACCES` and returns NOTHING — not one readable
file — where Rust, Python and `git` itself skip the directory and carry on.

- group: D_robustness
- done: false

#### D2: A path byte that is not valid UTF-8 matches the same way [[qmd77_goal_d2]]

An ignore rule matches a path whose bytes are not valid UTF-8 identically in all three. Python carries
the bytes through `surrogateescape` and matches; TypeScript and Rust replace them with U+FFFD and do
not. Unreachable on APFS, which refuses such a name, and a real difference on Linux ext4 — so closing
it needs a Linux run, which no test currently performs.

- group: D_robustness
- done: false

#### E1: A field value keeps the markup the author wrote [[qmd77_goal_e1]]

A field value comes back byte-identical to the source, in all three. Rust rebuilds a list-item value
from its Markdown events, so it rewrites every inline construct whose canonical form differs from the
written one, and drops the ones it has no arm for. Filed as
[issue 12](https://github.com/mikilabs/qmdc/issues/12) and still live on this build:
`guard __main__, name _x_, tag <b>bold</b>` reads back from Rust as
`guard **main**, name *x*, tag bold`, while Python and TypeScript return it as written. A value read
through Rust and written back therefore changes the document. The `Event::Code` arm in the same match
already takes the raw source slice; taking the slice for the whole value closes every construct at
once instead of adding an arm per construct.

- group: E_fidelity
- done: true
