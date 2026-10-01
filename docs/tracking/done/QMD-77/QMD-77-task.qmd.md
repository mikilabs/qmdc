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

- status: done
- priority: high
- category: parser
- related_task: [[#qmd75_body_identity]]
- requires_changes: []
- findings: [[#qmd77_finding_identity]], [[#qmd77_finding_content]], [[#qmd77_finding_contract]], [[#qmd77_finding_robustness]], [[#qmd77_finding_fidelity]], [[#qmd77_finding_questions]], [[#qmd77_finding_synth_ids]], [[#qmd77_finding_tests]]
- result: [[#qmd77_result]]

### Goals [[goals: [Goal]]]

#### A1: A duplicate id is kept and reported the same way everywhere [[qmd77_goal_a1]]

Two objects that resolve to the same global id produce the same object set and the same diagnostics
in all three parsers. Measured on a nested pair: Rust emits five objects and two `duplicate_id`,
Python four and one, TypeScript three and **none at all** — so TypeScript silently discards an object
that the other two report. Rust's ordering of the duplicated subtrees is also inverted against the
document.

DONE, in two unrelated halves. TypeScript had no duplicate check on the NESTED heading path at all,
so the first `dup.c` was overwritten and vanished with nothing reported. And Rust ordered the
duplicated subtrees by an (id, label) line look-up, which is not unique when BOTH collide — two
`## Child [[c]]` headings under two `[[dup]]` parents shared one entry, so one line was lost and the
subtrees came out in assembly order. The builder now carries the line under a private key the
caller strips after sorting, so the look-up that could collide is gone rather than patched.

- group: A_identity
- done: true

#### A2: A wrapped field value means one thing [[qmd77_goal_a2]]

A field whose value continues on an indented next line parses to the same value in all three. Today
Rust joins the lines with a space, Python with a newline, and TypeScript keeps only the first line and
drops the rest in silence. Needs `[[#qmd77_finding_questions]]` Q1 first: the format does not say
whether a wrapped value is legal at all.

DONE. Q1 was answered "an error": the value is the authored first line in all three and the
continuation raises the new `wrapped_field_value`. The two legal multiline forms are excluded by the
only thing visible on the first line — a YAML pipe (`- key: |`) and a bracket that opens a YAML array
(`- key: [`) — so neither needed a special case. An array ELEMENT is a value too and follows the same
rule. Not one document in this repository or in the external corpus raises it.

- group: A_identity
- done: true

#### A3: A declared array or map treats a deeper declaration the same way everywhere [[qmd77_goal_a3]]

`[[field: array]]` and `[[field: map]]` followed by a deeper heading that declares an object give the
same result in all three. Today Rust swallows the declaration into the array as a string element (or
into the map as an entry) and the object disappears; Python and TypeScript keep the object. QMD-75
settled this principle for `text` only. Q2 is answered — the mix is reported, with `mixed_array` for
an array and `invalid_map_content` for a map, one error at the offending heading's own line.

DECIDED and implemented (the design question below was settled by measurement, not preference): the
declared kind decides, so the heading creates NO object, and it CLOSES the collection's content
exactly as prose between two lists does. That second half is not a taste call — all three parsers
already agreed that a primitive array is fed by the FIRST list under its declaration and that
everything after it is the container's comment content, so "the heading swallows what follows"
would have been a new rule invented for this one shape. The map half was decided by the spec:
`[[#err_invalid_map_content]]` already said a map is populated only from the first valid bullet list
and that additional bullet lists are an error, which Rust contradicted by merging every list item in
the section.

- group: A_identity
- done: true

#### A4: An indented sub-item is one thing in all three [[qmd77_goal_a4]]

A field carrying indented sub-items resolves identically whether or not a sub-item's own text happens
to look like `key: value`. Rust turns such a sub-item into a field of its own and loses the field it
was indented under; Python and TypeScript report `nested_subitems` and keep the rest. This is the case
QMD-71 left open deliberately, and it is the entire remaining error gap on the external corpus — 125
reports against 148. Needs Q3.

- group: A_identity
- done: true

#### B1: Non-field content above the first heading is preserved identically [[qmd77_goal_b1]]

The `__TextBlock` synthesised for content before a document's first heading holds the same value in
all three. Today Rust keeps only a code fence where Python and TypeScript keep prose, table and fence,
and all three drop a `---` entirely. Needs Q4.

DONE. Q4 was answered "every block, and `---` everywhere". Rust dropped the prose because nothing
claimed it — no blockquote, no text field, no pending text block, no object yet — so a paragraph above
the first object now OPENS the text block, and the table after it joins in without a second fix. The
`---` needed a branch none of the three had: in an object's body it survives only because a comment is
a raw source slice that spans it.

- group: B_content
- done: true

#### B2: A wrapped list item keeps its shape in every carrier [[qmd77_goal_b2]]

A list item that continues on an indented next line parses to the same value wherever it sits — in
text-block content, in an object's body, in a comment. Three carriers are measured and all three
groupings appear: TypeScript strips the continuation's indent in text-block content, Rust joins the
lines in an object's body, and the field-value carrier is the three-way split of `[[#qmd77_goal_a2]]`.
Whatever Q1 decides applies to every carrier, with a fixture for each.

DONE, and the four carriers split two ways. A wrapped value (a field value, an array element) is now
an error — Q1. A wrapped item in PROSE is content and only has to be preserved identically: an
object's comment already was in all three, and TypeScript's text-block content was not, because it
rebuilt the list from inline tokens, where markdown-it has already stripped the continuation indent.
It takes the raw source slice now, like the other two.

- group: B_content
- done: true

#### C1: System objects are synthesised on the same files [[qmd77_goal_c1]]

The `__Document` and `__TextBlock` objects a parser synthesises appear for the same files in all
three. Rust emits neither for a file whose content is entirely YAML front matter, where the other two
emit both.

DONE, and the cause was not in the synthesis code at all: Rust took pulldown-cmark's
`Options::all()` minus three, so the YAML- and `+++`-delimited METADATA BLOCK extensions were on.
A `---` fence at the top of a file was swallowed whole and emitted no events, so a file holding
nothing else produced NO objects, where the other two read it as ordinary Markdown — a thematic
break and a setext heading — and synthesised a `__Document` and a `__TextBlock`. QMD.md defines no
front matter, exactly as it defines no footnotes, so the extension set is now an explicit list of
the four extensions the format does use. This is the other half of the QMD-71 finding: `all()`
minus a few still grows on a dependency bump.

- group: C_contract
- done: true

#### C2: A synthesised id is unique within the graph [[qmd77_goal_c2]]

No two objects in one parse result carry the same `__id`. Two different files used to both get
`doc_ry4ljv` and `text_0`, and the graph should not hold two objects under one id and report nothing.

CLOSED with a readable path-derived id rather than a wider hash, decided 2026-10-01: a hash buys
opacity at the price of a probability (1.2% at ten thousand documents even with a wide id, because the
LCG state is 32 bits), and these ids exist to make a rendered document legible, which a hash does not
serve. A file's path is unique, so the derived id needs no probability at all. The stem is the path
relative to the directory that declared the namespace, which keeps it short:
`format/deep/commands.qmd.md` becomes `doc_deep_commands`, and a text block keeps its ordinal.

Measured after: `workspace parse ./docs` and the query layer agree at 125 objects in all three
parsers, where the query used to see 14; the sample workspace's edge count rose from 74 to 79 as the
containment edges came back. A single-file `parse` keeps the counter form, so no parser fixture
changed — only the workspace, sql and lsp expectations that pinned the literal old ids.

- group: C_contract
- done: true

#### C3: An error names its object the same way in both commands [[qmd77_goal_c3]]

`workspace parse` and `workspace validate` describe an error's owning object with the same keys. Parse
says `object`; validate says `objectId` and adds `fieldName`. All three parsers agree within each
command, so this is one contract decision, not a parser bug — and it is a breaking change for a
consumer either way.

DONE: `workspace parse` now names the owning object `objectId` and adds `fieldName`, the same keys
`workspace validate` already used, and `docs/parsers/commands.qmd.md` says so. It is a breaking
change for a consumer reading `object`, which is why it is recorded here and pinned by C6. The rs
conformance harness accepts both spellings, because it normalises the fixtures and the envelope
through one function and the fixtures keep their own vocabulary.

- group: C_contract
- done: true

#### C4: The remaining diagnostic details agree [[qmd77_goal_c4]]

Three small disagreements, each one line of output: the line number in `mixed_field_keys` (Rust points
at the offending item, the other two at the item above it, which has no colon); the count and line of
`invalid_map_content` (Rust one at the declaration, the other two at the offending line); and
`__has_explicit_id: false` on an object-array element with no `[[id]]`, which only Rust emits.

DONE, and two of its three parts had already closed under other goals. The `mixed_field_keys` line
now agrees in all three. The `invalid_map_content` count and line were settled by A3 — one error per
offending block, at the block's own line. What was left was `__has_explicit_id` on an object-array
element written as a bare `### Alice`: the spec says the key is `false` when the id was
AUTO-GENERATED, so Rust was right and the other two now mark it — without it a rebuild prints an id
the author never wrote.

- group: C_contract
- done: true

#### C5: A comment anchor names a field that exists [[qmd77_goal_c5]]

The `after` anchor of a comment names a field of the object it sits on. After a `nested_subitems`
error Rust anchors the following comment on the offending field name, which no parser keeps as a
field, so the anchor dangles.

- group: C_contract
- done: true

#### C6: The parse envelope is pinned by a fixture [[qmd77_goal_c6]]

A `tests/workspace` fixture asserts the `workspace parse` envelope: `index` present with
`by_global_id`, `by_kind`, `by_file` in snake_case, the `errors` block's keys, and `workspaces` with
`{id, root, path}`. QMD-75 aligned these across the three parsers and nothing pins them, so the next
divergence in the envelope is invisible to the suite.

DONE as a cross-parser TEST rather than a `_expected.json` fixture: the workspace fixture runner
asserts only workspace id, file list, objects-by-kind and errors, and has no way to say anything
about the envelope. `test_qmd77_workspace_parse_envelope` runs all three CLIs and pins the
top-level keys, the snake_case `index` sub-keys (with `byGlobalId` explicitly absent), the
`{id, root, path}` shape of a `workspaces` entry, and the error key names C3 settled.

- group: C_contract
- done: true

#### D1: An unreadable directory does not lose the readable files [[qmd77_goal_d1]]

A workspace scan that meets a directory it cannot read skips it and returns everything else, in all
three. TypeScript's `scanWorkspace` throws an uncaught `EACCES` and returns NOTHING — not one readable
file — where Rust, Python and `git` itself skip the directory and carry on.

- group: D_robustness
- done: true

#### D2: A path byte that is not valid UTF-8 matches the same way [[qmd77_goal_d2]]

MOVED to [[#qmd78_non_utf8_path]]. An ignore rule matches a path whose bytes are not valid UTF-8 in
Python, which carries them through `surrogateescape`, and not in TypeScript or Rust, which replace them
with U+FFFD. The divergence is unreachable on APFS, which refuses such a filename, and real on Linux
ext4, so it cannot be seen red here — and this task's own verification rule
[[#qmd77_finding_tests]] says a goal in that position is split out rather than claimed.

- group: D_robustness
- done: true

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
