# QMD-75: Result

## The declaration decides what a heading is; its body does not [[qmd75_result: Result]]

A heading's identity now comes from what it declares and from the structure below it, never from the
prose in between. `[[id: Kind]]` is an object and `[[id: text]]` is a text field that swallows
everything below it, deeper declarations included. A bare `[[id]]` is decided by ONE rule in all
three parsers: an object when, before the next heading at its own level or shallower, there is either
a field list of its own or a deeper heading that DECLARES an identifier; an implicit text field
otherwise, its value running to that same boundary. A paragraph, a `---`, a table, a code fence, a
blockquote or a list without fields changes neither answer — it lands in `__comments`, or in the text
field's value. And "declares" is the format's own definition syntax: brackets inside a code span and a
reference such as `[[#other]]` declare nothing.

Measured on the matrix the rule is about: three spellings × eight body shapes × with and without a
declared child, 48 cases. Rust, Python and TypeScript agreed on 31 of 48 before and agree on 48 of 48
now — the generator and both readings are kept as `reviews/qmd75_matrix.py`,
`reviews/qmd75-matrix-head.txt` and `reviews/qmd75-matrix-current.txt`, each recording the mtime and
size of the build it actually measured. On the live corpus that surfaced this — `tg-acp` at `36f37c5`,
250 `*.qmd.md`, 240 scanned — Python and TypeScript are byte-identical and all three carry the same
1134 authored objects over 1134 distinct global ids. Before this task the same tree gave 1 error in
Rust, 149 in Python and 150 in TypeScript, and Python and TypeScript each silently collapsed two
authored objects onto one id.

All six goals are complete. `make test` is green: 4216 cases in the unified report, 0 failures, with
sixteen new parser fixtures under enforced three-parser parity, each verified to fail against `HEAD`.

- feature: [[#qmd75_body_identity]]
- completed: 2026-09-30
- files_changed: [qmdc-rs/src/parser.rs, qmdc-rs/src/workspace.rs, qmdc-rs/src/main.rs, qmdc-py/qmdc/parser.py, qmdc-py/qmdc/workspace.py, qmdc-ts/src/parser.ts, qmdc-ts/src/workspace.ts, docs/format/headings.qmd.md, CHANGELOG.md]
- tests_added: [tests/parser/254-bare-id-prose-body-with-declared-child.qmd.md, tests/parser/255-bare-id-thematic-break-body-with-declared-child.qmd.md, tests/parser/256-bare-id-table-body-with-declared-child.qmd.md, tests/parser/257-bare-id-fence-body-with-declared-child.qmd.md, tests/parser/258-text-field-swallows-declared-child.qmd.md, tests/parser/259-bare-id-table-body-no-child.qmd.md, tests/parser/260-nested-subitems-on-array-element.qmd.md, tests/parser/261-indented-orphan-bullet-is-prose.qmd.md, tests/parser/262-hr-then-ordered-item-nested-bullets.qmd.md, tests/parser/263-blockquote-then-comment-heading-bullets.qmd.md, tests/parser/264-bare-id-undeclared-deeper-heading-is-text.qmd.md, tests/parser/265-bare-id-prose-before-field-list.qmd.md, tests/parser/266-bare-id-table-before-field-list.qmd.md, tests/parser/267-code-span-in-deeper-heading-declares-nothing.qmd.md, tests/parser/268-reference-in-deeper-heading-declares-nothing.qmd.md, tests/parser/269-fence-body-then-comment-heading-bullets.qmd.md, reviews/qmd75_matrix.py]

### What changed [[qmd75_result_changes: text]]

- about: [[#qmd75_result]]

- **One rule, written down.** `docs/format/headings.qmd.md` now states it: a declared field kind
  decides, a bare `[[id]]` is decided by structure, and content that is not a field list is
  preserved on the object it belongs to whether or not the heading spelled a Kind. Whether a
  document's objects exist, and what their ids are, no longer depends on its prose.
- **Rust.** A table body no longer claims a heading that has its own declaration below it. Non-field
  content — `---`, table, fence — now survives under a bare `[[id]]` object, where the capture gate
  tested the Kind and so kept it only under `[[id: Kind]]`. `nested_subitems` was disabled for the
  whole subtree of an object array (`pending_object_array.is_none()`), the same over-wide condition
  QMD-70 narrowed twice before; it now fires on array elements too, which is 125 of the reports on
  `tg-acp` that Rust used to miss entirely. The `__syntax: table` label on a table under a bare
  `[[id]]` is gone: under this rule a table body declares no array, and the common path already
  produced what Python and TypeScript produce.
- **Python.** The look-ahead for a declared child existed but was not consulted on this branch, so
  prose closed the object and took its subtree with it. The same capture gate as Rust's was widened.
  A false `mixed_field_keys` on a branch that parses no fields at all is gone — it named a line the
  parser itself had read as prose.
- **TypeScript.** Five defects, all one family. A deeper heading escaped an explicit `[[id: text]]`
  field when it carried its own `[[id]]`, closing the field early and re-parenting that heading onto
  the field's *owner*. The body alone decided whether a bare `[[id]]` was an object, so a `---` above
  a declaration cost two nesting levels. `pendingTextFieldStartLine` was read from `heading_close`,
  whose map markdown-it sets to `null`, so the raw-slice path that closes a text field was **always**
  dead and a `---`-only body came out empty; every termination path now assigns one slice instead of
  appending fragments, which also stops blank lines inside a text field being normalised. A bullet
  list nested under an ordered item was read as fields, where QMD-71 settled it as prose for the
  other two. And a comment scan walked through a deeper undeclared heading, merging two `__comments`
  entries into one and handing its field-like bullets to the field parser, which invented a field and
  then reported `mixed_field_keys` on a line that is only prose with a colon in it. That comment scan
  existed in two copies — the table/`---`/quote capture and the fence capture — and both corrections
  landed on the first only, so the fence path still invented a field; the two are now one helper,
  `findCommentEnd`.
- **All three, the declaration test.** The "declared child" lookahead matched any `[[...]]` in a deeper
  heading's text, so brackets in a code span counted and so did a reference — `### See [[#s]]` made
  the heading above it an object and gave it a field literally named `#s`. Code spans are stripped and
  a leading `#` is excluded, in all three.
- **Shape.** `index` is emitted by all three and its sub-key is `by_global_id` everywhere
  (TypeScript spelled it `byGlobalId`); `workspace parse`'s `errors` block carries the same keys in
  all three, which is not the same shape as `workspace validate`'s (see residuals).
- **Pinned.** Sixteen parser fixtures. The first ten: the four bare-`[[id]]` bodies with a declared
  child, the explicit `text` field swallowing one, a table body with no child, `nested_subitems` on an
  array element, the prose bullet, the ordered-item list and the comment-heading bullets. Six more
  from the review round (264-269): an undeclared deeper heading, a body in front of a field list as
  prose and as a table, a code span and a reference in a deeper heading, and the fence path of the
  comment scan. Each was checked against `HEAD`'s parsers and fails there — 266 fails in all three,
  because at `HEAD` all three agreed on the wrong answer.

### Walked back during the work [[qmd75_result_walkback: text]]

- about: [[#qmd75_result]]

Two formulations of the rule were wrong before the third held, and both were caught by measurement
rather than by reading.

The first was too WIDE — "a bare `[[id]]` is always an object" — and it broke 15 pinned cases, because
an implicit text field (`### Description [[description]]` followed by prose) is not an accident but a
pinned feature: parser fixtures 021, 030 and 107, eight LSP cases and two workspace cases depend on it.

The second was too NARROW, and the blind review found it: "an object when a deeper heading declares its
own `[[...]]` below it" omitted that a field list of its own also makes it an object, which all three
parsers have always done. Worse, the implementation kept two extra readings in Python and TypeScript —
any deeper heading, and a field list only as the immediately next token — so prose still decided
identity one step outside the 48-case matrix, and the removal of Rust's table lookahead introduced a
NEW parity break on a table in front of a field list. The written rule claimed all of this was closed.
Both are fixed and pinned; the review round is `[[#qmd75_finding_review]]`.

### Residuals [[qmd75_result_residuals: text]]

- about: [[#qmd75_result]]

Measured, confirmed pre-existing by running `HEAD`'s binaries on the same input, and left for their
own tasks. None is about whether a heading's body decides its identity.

- Rust parses an indented sub-item whose text has a valid field key as a *field* of its own, losing
  the sibling field it was indented under and reporting nothing. This is the case QMD-71 left open
  deliberately, and it accounts for the whole remaining error gap on `tg-acp`: 125 `nested_subitems`
  in Rust against 148 in the other two.
- Rust synthesises no `__Document`/`__TextBlock` pair for one steering file whose content is entirely
  YAML front matter, where Python and TypeScript synthesise both — 2 system objects against 4. The
  authored objects are identical in all three.
- The synthesised ids of those system objects (`doc_*`, `text_0`) repeat across files with no
  diagnostic. They carry no `__global_id`, so nothing can reference them, but two files sharing
  `doc_ry4ljv` is not an identity a graph should hold.
- The line number in `mixed_field_keys` differs between Rust and the other two.
- A declared `[[id: array]]` or `[[id: map]]` above a deeper declaration: Rust swallows the
  declaration into the array or map, Python and TypeScript keep it as an object. The "declared kind
  decides" principle is settled here only for `text`.
- `nested_subitems`' comment anchor: all three now report the error on the same shapes, but Rust
  anchors the following comment on the offending field name, which is not a field of the object in any
  parser, so that anchor dangles.
- A top-level text block before the first heading keeps different content in each parser (Rust the
  fence only, the others prose + table + fence; all three drop a `---`).
- TypeScript keeps only one of two duplicate objects and emits no `duplicate_id` where Rust and Python
  keep both and report; Rust and Python also order duplicate subtrees differently.
- An object-array element with no `[[id]]` carries `__has_explicit_id: false` in Rust and omits it in
  the other two.
- `workspace parse` and `workspace validate` name the owning object differently — `object` against
  `objectId` plus `fieldName`. All three parsers agree within each command; the two commands do not
  agree with each other.
- Not pinned by any fixture, though changed here: the `workspace parse` contract keys (Rust's `index`,
  TypeScript's `by_global_id`), and Python's `__positions` for the first of two duplicates. A
  `tests/workspace` fixture asserting the parse envelope would close this.
