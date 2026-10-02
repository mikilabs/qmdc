# QMD-70: Result

## A table inside an array element stays that element's content [[qmd70_result: Result]]

A Markdown table written as ordinary content inside an object-array ELEMENT is no longer
mistaken for more rows of the array. The element keeps its explicit id and its fields, the table
is carried as the element's comment content, and the three parsers agree on every shape.

All six goals are complete. `make test` is green end to end: **3525 cases, 0 failures**.

- feature: [[#qmd70_table_scope]]
- files_changed: [qmdc-rs/src/parser.rs, qmdc-py/qmdc/parser.py, qmdc-ts/src/parser.ts, docs/format/arrays.qmd.md]
- tests_added: [tests/parser/212-table-in-array-element-after-prose.qmd.md, tests/parser/213-table-in-array-element-first.qmd.md, tests/parser/214-table-in-array-element-after-field.qmd.md, tests/parser/215-table-in-array-element-between.qmd.md, tests/workspace/table-child-id-composition, tests/cli/015-validate-table-in-array-element, tests/lsp/microtests/diagnostics/035-table-in-array-element]

### What changed [[qmd70_result_changes: text]]

- about: [[#qmd70_result]]

**Scope.** The table-to-array conversion now fires only while the parser is positioned in the
array CONTAINER. `pending_object_array` deliberately stays set for the whole array subtree —
each sibling element still needs it — so it cannot make that distinction on its own; the current
object can, being the array's parent in the container and the element itself once an element
heading has opened. The comment path lost its `pending_object_array.is_none()` guard so it can be
reached from inside an array subtree, and a table inside an element takes it.

**A second cause the triage missed.** Shapes A and D lost the element rather than merely gaining
a phantom, and the reason was not the scope issue at all: Rust's `has_table_after` lookahead
skips paragraphs, so an element written as prose-then-table matched the "bare `[[id]]` followed by
a table" table-field pattern and was claimed as a table FIELD of the grandparent. Its explicit id
became an empty array, its fields were dropped, and the table's rows still landed in the
enclosing array. That lookahead now declines any heading that would be an array element.

**Id composition.** Rust's table-child path re-implemented the id rule inline, and the copy had
drifted from Rust's own `resolve_child_id` helper in two places: it composed
`{parent}.{field}.{local}` unconditionally, doubling the segment when the array field name IS the
parent's id (a top-level array), and in the system-container case it prefixed the parent id where
the helper, Python and TypeScript all use the bare local id. The path now calls the helper, so
the table children and the heading elements of an array cannot drift apart again.

The system-container half was found by the code review, not by the task: it is the same rule but
a branch the tests never reached, and `make validate-compare` could not see it because it
compares validation errors rather than ids.

### Verification [[qmd70_result_verification: text]]

- about: [[#qmd70_result]]

`make test`: 3525 cases, 0 failures, including cross-parser validation comparison, markdownlint
and the guide token budget.

Eight regression cases were written before the fix and now pass: the four shapes of the
characterization matrix on the parse surface, the same defect through `qmdc workspace validate`,
the LSP and MCP, and two id-composition cases (in the SQL harness, because that shape's rebuild is
not round-trip stable — a separate pre-existing limitation). The five shipped table fixtures — `032-table`,
`042-table-one-row`, `064-text-field-with-table`, `065-text-table-in-array` and
`089-comments-preserve-tables` — were verified green in all three implementations, so the
documented feature and the declared-`text`-field escape both still work.

Two shipped fixtures were re-checked specifically because they pin behaviour this change could
have broken: `032-table` (a table directly under an array heading must still convert) and
`065-text-table-in-array` (a table inside an element wrapped in a declared `text` field must go
to that field, not to comments).

### Fixed after the code review [[qmd70_result_review: text]]

- about: [[#qmd70_result]]

The review found a further cluster in the same function, all of it silent data loss or parity drift
that no fixture reached.

Rust's table-child path re-implemented `resolve_child_id` inline and the copy had drifted in two
places — the doubled segment this task already knew about, and a system-container branch that
prefixed the parent id where the helper, Python and TypeScript use the bare local id. It now calls
the helper, so one rule serves both table children and heading elements.

A second table under one array heading was worse: Rust converted both, both children took the same
`local_id`, the second overwrote the first, and the array carried the same reference twice — one
row gone with no error. Python and TypeScript closed the array context after the first table and
Rust did not. Fixing that exposed two more divergences in the same shape: Rust DROPPED the leftover
table rather than keeping it, because the parent has already left `current_obj` by then, and
TypeScript reset the comment anchor to `__self` for a heading that declares a field rather than an
object. All three now agree.

TypeScript also cut the separator row off a table with no data rows, because its comment scan
overwrote the slice end with child-token maps that are narrower than their container's.

### Second review pass [[qmd70_result_review2: text]]

- about: [[#qmd70_result]]

A second review, scoped to the fixes the first one produced, found that the two-table fix taught
Rust to keep a trailing TABLE and only a table. Prose after an array container's own table was
dropped, and a declared field written there was lost outright, while Python and TypeScript kept
both.

That was fixed structurally rather than patched per content type: the array's parent now STAYS in
`current_obj` instead of being finalized at the array heading, so the field, comment and reference
paths keep working unchanged — they had simply lost their target. Its slot in `objects_map` is
reserved with a skeleton entry so the array's children still follow their parent in the output,
reusing the skeleton handling `finalize_object` already had.

Two consequences worth recording. The goal A1 scope guard had to change from `current_obj.is_none()`
to comparing the current object against the array's parent — which is what Python and TypeScript
always did; the old form held only because the parent was being finalized, and the SQL fixture for a
top-level array caught the difference immediately. And the workaround variable the first fix
introduced had no assignment site left, so it was deleted rather than kept as dead code.

Two divergences of the same family remain, both reproducing with no array anywhere in the document,
so they are separate defects in other paths — see [[#qmd70_finding_rs_other_paths]].

Two things were fixed. TypeScript's anchor guard was narrower than Python's: it skipped the
`__self` reset only for `object_array`, while Python skips it for every field-type heading that has
a parent. A `yaml` field heading exposed the difference, because unlike `text` and `array` it never
re-sets the anchor afterwards. The guard now mirrors Python's condition exactly. Separately the new
Rust target is cleared once consumed — no input was found where a stale one attached to the wrong
object, but leaving that to be incidental rather than explicit invites a later regression.

Documentation carried a claim this work falsifies: `docs/parsers/commands.qmd.md` promised that
`parse | rebuild` "restores the original document". It is qualified now, naming both shapes where
the layout shifts while the graph stays identical. `docs/format/arrays.qmd.md` also gained the rule
that only the first table under an array heading feeds it.

### Four new format rules, and what they cost [[qmd70_result_rules: text]]

- about: [[#qmd70_result]]

The task grew well past its triage. Four constructs were found to have no defined meaning in the
format, each mishandled differently by the three parsers and each silently losing or mangling data.
The operator decided all four are errors, and each was verified to occur NOWHERE in the repository
outside the fixtures written for it, so no existing document could have depended on the old reading:

| error | the construct |
| --- | --- |
| `table_in_array` | a table under a primitive array field — a primitive array holds scalars, a table has columns, no mapping exists |
| `extra_table_in_array` | a SECOND table under one object-array heading — its rows would collide on the generated ids |
| `mixed_array` | a heading element after the array was already fed by a table — the element left the array and its declared Kind was degraded |
| `block_in_inline_field` | an indented block under an inline field that has a value — an inline field holds a scalar and has no content of its own |

Each costs the same: three parsers, four surfaces (parse, `workspace validate`, LSP, MCP — all
generic over `__ParsingError`, so no surface code changed), and five documentation files plus
`make guide-sync`.

Making them errors dissolved two problems rather than working around them. `extra_table_in_array`
removed content that `rebuild` could not place under any anchor, and the mechanism is the corpus's own
rule: both microtest harnesses skip the round-trip check for a document with parsing errors.
`block_in_inline_field` replaced three different manglings with one message.

### Out of scope [[qmd70_result_scope: text]]

- about: [[#qmd70_result]]

One divergence found while writing the id-composition fixture is left open as a question, not
fixed: prose between an array heading and its table. Rust still converts the table, Python and
TypeScript treat both as comment content. It is the same family — where does a table belong? —
but a different cell, and neither reading is obviously wrong, so it needs a decision before a
fix. See [[#qmd70_finding_prose_gap]].

The reverse-order mixed array (a table BEFORE any element heading, then element headings) also
remains unpinned. It still converts, with the heading elements appended after the table's rows.

One defect is recorded and deliberately NOT fixed: `rebuild` cannot place a comment anchored on a
heading-declared array field, so the leftover table from the two-table shape does not survive a
round trip under any anchor — see [[#qmd70_finding_rebuild_anchor]]. Closing it means deciding how
content following such an array is represented, which is a format decision rather than a bug fix.
