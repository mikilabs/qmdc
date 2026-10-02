# QMD-77: Result

## Every remaining three-parser divergence, closed one measurement at a time [[qmd77_result: Result]]

Fifteen divergences were collected here because each had survived at least one release for the same
reason: parity is measured on `docs/`, a corpus we write ourselves and which therefore avoids every
shape that breaks. Fourteen are closed. The fifteenth is split out as
[[#qmd78_non_utf8_path]] because it cannot be seen red on this machine at all.

The rules the operator chose, and what each turned out to cost:

A field value is **one line**. A continuation used to be joined with a space by Rust, with `\n` by
Python, and dropped entirely by TypeScript — so the three disagreed on the author's own text. It is now
a reported error, `wrapped_field_value`, with the value being the first line as written. The legal
multi-line forms are told apart by what the first line shows: a `|` block scalar and an opening `[`.
Eight places in our own documents had to be straightened, and all eight had been written by an agent
that could not see the damage.

A declaration inside a declared collection **does not create an object**, which extends the rule
QMD-75 settled for `text`. The heading closes the collection's content instead, exactly as prose
between two lists does, and is reported once at its own line: `mixed_array` for an array,
`invalid_map_content` for a map. Rust used to swallow the heading into the array as the string
`"status: x"`; the other two created the object. Three map divergences that were not on the list fell
out of the same fix — Rust merged a second bullet list into the map where the spec says only the first
populates it, nobody reported a map entry without a colon, and a nested sub-item became an entry the
author never wrote.

A nested sub-item is **never a field**. Rust turned `- AMBIGUOUS: text` under `- issues:` into a real
field whose key came from prose, lost the parent key, and reported nothing. That single behaviour was
the whole error gap on the external corpus: 125 errors in Rust against 148 in the other two, now 148
in all three.

A synthesised id is **derived from its file**. Every document carried the same literal `doc_ry4ljv` and
every file's first text block `text_0`, and the graph keys on `<workspace>:<namespace>:<id>` — so files
sharing a namespace collided and the last one read won. The query layer returned 14 of the 125
synthesised objects in `docs/`; it now returns every one, and the sample workspace's edge count rose from
74 to 79 as the containment edges came back with them. A readable path-derived stem was chosen over a
wider hash: a hash keeps a collision probability (1.2% at ten thousand documents even at eight
characters, because the LCG state is 32 bits) and tells a reader nothing. The stem alone is NOT
unique, as the code review showed: folding maps `a-b`, `a_b` and `a/b` to one stem and a Cyrillic name
to nothing. So uniqueness is assigned — files in path byte order, a later file whose ids are taken by
an earlier file or by an author-written id of the namespace gets the next free `_1`, `_2` suffix. See
[[#qmd77_finding_synth_ids]] for the numbers behind the choice and [[#qmd77_finding_review_round1]]
for the collision.

A field value keeps **the markup the author wrote**, which closes
[issue 12](https://github.com/mikilabs/qmdc/issues/12). Rust rebuilt list-item values from Markdown
events, so `__main__` came back as `**main**`, `_x_` as `*x*`, and inline HTML vanished. The fix was the
one the issue itself proposed — take the source slice, as the `Event::Code` arm already did — plus a
depth counter so inner events of a covered construct are not appended twice, and a one-byte reach for
an escape, whose backslash pulldown-cmark includes in no range at all.

- feature: [[#qmd77_tails]]
- completed: 2026-10-01
- files_changed: [qmdc-rs/src/parser.rs, qmdc-rs/src/workspace.rs, qmdc-rs/src/parser_modules/output.rs, qmdc-rs/src/lsp/server.rs, qmdc-py/qmdc/parser.py, qmdc-py/qmdc/parsers/field.py, qmdc-py/qmdc/workspace.py, qmdc-ts/src/parser.ts, qmdc-ts/src/parsers/field.ts, qmdc-ts/src/workspace.ts, docs/format/arrays.qmd.md, docs/format/fields.qmd.md, docs/format/objects.qmd.md, docs/format/validation-errors.qmd.md, docs/parsers/commands.qmd.md, docs/lsp/diagnostics.qmd.md, CHANGELOG.md]
- tests_added: [tests/parser/270-field-value-keeps-authored-markup.qmd.md, tests/parser/271-nested-subitem-is-never-a-field.qmd.md, tests/parser/272-nested-subitem-field-shape-under-empty-key.qmd.md, tests/parser/273-nested-subitem-then-prose-anchor.qmd.md, tests/parser/274-nested-subitem-sole-content-still-reported.qmd.md, tests/parser/275-declaration-inside-declared-array.qmd.md, tests/parser/276-declaration-inside-declared-map.qmd.md, tests/parser/277-map-item-without-colon-is-reported.qmd.md, tests/parser/278-map-nested-subitem-under-empty-entry.qmd.md, tests/parser/279-map-nested-subitem-under-valued-entry.qmd.md, tests/parser/280-wrapped-field-value-is-reported.qmd.md, tests/parser/281-wrapped-array-element-is-reported.qmd.md, tests/parser/282-legal-multiline-values-are-not-wrapped.qmd.md, tests/parser/283-textblock-list-keeps-continuation-indent.qmd.md, tests/parser/284-textblock-keeps-every-block-above-first-heading.qmd.md, tests/parser/285-nested-duplicate-id-kept-and-reported.qmd.md, tests/cli/037-query-field-markup-verbatim/cmd, tests/workspace/synthetic-ids-unique-per-file/readme.qmd.md, tests/workspace/synthetic-ids-namespace-relative/readme.qmd.md, tests/parser/286-error-ids-unique-in-one-result.qmd.md, tests/parser/287-textblock-keeps-leading-quote-list-and-html.qmd.md, tests/parser/288-textblock-setext-heading-is-verbatim.qmd.md, tests/parser/289-textblock-front-matter-is-rule-and-setext.qmd.md, tests/parser/290-textblock-leading-table-opens-block.qmd.md, tests/parser/291-textblock-heading-keeps-markup-quote-and-html.qmd.md, tests/parser/292-textblock-leading-field-list-keeps-bullets.qmd.md, tests/parser/293-textblock-keeps-html-comment-inside-block.qmd.md, tests/parser/294-textblock-fence-offsets-after-table.qmd.md, tests/workspace/synthetic-ids-folding-collisions/readme.qmd.md, tests/cli/038-orphan-files-synthesised-ids-distinct/cmd, tests/workspace/duplicate-id-file-order/readme.qmd.md, tests/cli/039-orphan-files-read-in-workspace-order/cmd]

### The rest of what closed [[qmd77_result_rest: text]]

- about: [[#qmd77_result]]

- **Content above the first heading (B1).** Prose and tables now reach `__TextBlock` in Rust too, and
  a `---` inside a text block survives in all three. A `---` inside an object's body was already
  preserved as a comment after QMD-75, so dropping it at the top of a file was inconsistent with the
  format rather than a separate decision.
- **Front matter (C1), which was not about synthesis at all.** Rust passed `Options::all()` minus
  three flags to pulldown-cmark, so metadata blocks were enabled and a leading `---` fence was
  swallowed whole, emitting no event: a file holding nothing else produced ZERO objects. The format
  defines no front matter, exactly as it defines no footnotes, so the extension set is now an explicit
  list of four. This is the other half of QMD-71's finding — `all()` minus a few keeps growing on a
  dependency bump.
- **Duplicate ids (A1).** TypeScript never checked for a duplicate on a nested heading, so the first
  of a pair vanished silently, and Rust ordered duplicates by the pair (id, label), which is not
  unique when both match. After the second review round all three also read files in one order:
  TypeScript compared names with locale collation, so when two files differed only in case or
  punctuation it reported the other occurrence as the duplicate, and the files outside every
  workspace were read in directory-listing order everywhere.
- **One vocabulary for an error (C3, C4, C6).** `workspace parse` now names the owning object
  `objectId` and adds `fieldName`, the keys `workspace validate` already used — a breaking envelope
  change, pinned by a conformance test over the whole envelope so the two commands cannot drift again.
- **A comment anchor names a field that exists (C5).** Rust hung a following comment on a key it had
  just deleted, so `after` pointed at a field no parser reported.
- **An unreadable directory (D1).** An `EACCES` from `readdirSync` escaped one of four scanners in
  TypeScript and lost the ENTIRE result, not just the unreadable subtree. The test builds the tree and
  removes the permission at run time, because a mode cannot be committed to git.
- **Side effect worth naming.** The A4 fix made the LSP offer `error_0` as a reference target: the
  completion filter excluded system objects by id prefix (`doc_`, `text_`) and so missed
  `__ParsingError`. It now filters by kind, which is the real test of addressability — none of these
  objects carries a `__global_id`.

### How it was verified [[qmd77_result_verification: text]]

- about: [[#qmd77_result]]

Each goal has a data-driven fixture seen RED on the previous commit's binaries, built and run rather
than reasoned about from the diff. Four fixtures had to fail in all three parsers, because for those
shapes all three agreed on the wrong answer.

`make test` is green at 4519 cases with 0 failures: validation errors IDENTICAL across the three,
parse output exact on all 137 documents, `make lint` clean including clippy under `-D warnings`. The
QMD-75 declaration matrix still reads 48 of 48. On the external corpus — `tg-acp`, 240 files scanned,
not written by us — all three report 149 errors and 1139 objects with no duplicate (namespace, id)
key, where this task started from 125 errors in Rust against 148 and 149 in the others. The 1139th
object is a second `__TextBlock` in a steering file that opens with YAML front matter: its first `---`
is a thematic break and the rest a setext heading, now kept as written instead of rebuilt.

### Code review gate, round 1 [[qmd77_result_review: text]]

- about: [[#qmd77_result]]

The SOP gate was run late — after the work was committed and `done_review` set — and the status went
back to `in_progress` until it passed. A blind reviewer found three blockers, one high, one medium and
one info; every finding was reproduced by running all three binaries before anything changed, see
[[#qmd77_finding_review_round1]]. All are fixed with fixtures seen red in all three parsers on the
reviewed commit (`09f91c9`): the synthesised-id collision (my own claim, wrong), a duplicate error id
in Rust, and the text-block content, which is now the verbatim source of its region — the rebuild had
lost a blockquote, an HTML block, a setext heading's second line and a heading's markup, differently
in each parser. The review also led to the second place synthesised ids are made, files outside every
explicit workspace, which C2 had not reached at all.

### Code review gate, round 2 [[qmd77_result_review_round2: text]]

- about: [[#qmd77_result]]

A second blind reviewer verified every round-1 fix on the three binaries and approved, with one
medium and one low note, see [[#qmd77_finding_review_round2]]. The low note, TypeScript's file order,
was measured further and decides which occurrence of a duplicate id is reported, so it is fixed under
A1 with two fixtures: red in TypeScript on the reviewed build, and on the path for files outside every
workspace red in Rust too. The medium note is a residual below.

### Code review gate, round 3 [[qmd77_result_review_round3: text]]

- about: [[#qmd77_result]]

A third blind reviewer checked only the round-2 delta and approved with one low note, see
[[#qmd77_finding_review_round3]]. The note was that no round had checked the CHANGELOG text written
after round 1. Checked by running the code before and after the round-1 fixes in all three parsers:
one sentence was wrong — a list above the first heading did open a text block in Python and
TypeScript — and one loss was missing, a `~~~` fence rewritten with backticks. Both are corrected.

### Residuals [[qmd77_result_residuals: text]]

- about: [[#qmd77_result]]

- **A non-UTF-8 path byte** is [[#qmd78_non_utf8_path]], split out rather than claimed: the filename
  cannot be created on APFS, so the divergence is unreachable here and needs a Linux run.
- **`text_<n>` ordinals remain positional within a file.** Two blocks in one file are `text_<stem>_0`
  and `text_<stem>_1`, so inserting a block above another renumbers the one below it. Unique and
  stable per parse, not stable across an edit.
- **The semantic index must be rebuilt.** It stores the old synthesised ids as `object_id`, so
  `make semantic-index` has to run before those rows mean anything again.
- **`workspace parse` and `parse` disagree on a synthesised id by design.** A single-file parse has no
  workspace and no namespace to make the id relative to, and one document cannot collide with itself,
  so it keeps the counter form. Both spellings are documented.
- **An HTML comment inside an object's `__comments` still diverges** — Rust drops it as the spec says,
  Python and TypeScript keep it. Found during the review round, outside this task's goals, so it is
  [[#qmd79_html_comment_in_comments]] under this task's own rule rather than a new tail here.
- **Which duplicate the query layer keeps.** The occurrence read last, which is itself one of those
  reported as `duplicate_id`. Identical in all three now; the spec does not say which should win.
- **Text-block fragments are still assembled (review note G2).** Since content became the source
  slice, they are read only by the `structured_in_textblock` gate, and that gate is not the same
  expression in the three parsers, so removing them means deciding the gate first. All three
  declarations say so.
- **Files outside every workspace load to different depths** — 5 in Rust, any depth in Python and
  TypeScript. Pre-existing and outside this task's goals, so it is [[#qmd80_orphan_depth]].
