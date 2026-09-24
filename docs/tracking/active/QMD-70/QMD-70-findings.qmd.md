# QMD-70: Findings

## The array context outlives the array's own content [[qmd70_finding_scope: Finding]]

The table-to-array conversion is gated on a single boolean-ish state — "is an object array
open?" — which stays set for the whole array subtree because the next sibling element needs
it. A table inside an element therefore looks identical to a table under the array heading,
and the parser takes the rows.

- category: parser
- related_to: [[#qmd70_table_scope]]
- affected_files: [qmdc-rs/src/parser.rs, qmdc-py/qmdc/parser.py, qmdc-ts/src/parser.ts]
- affected_functions: [create_table_child_objects]
- solution: Distinguish "positioned in the array container" from "positioned inside one of its elements". The table branch must only fire in the former; in the latter the table is the element's text content.

### The state and the missing distinction [[qmd70_finding_scope_detail: text]]

- about: [[#qmd70_finding_scope]]

In Rust `pending_object_array` (`qmdc-rs/src/parser.rs:242`) holds
`(parent_id, field_name, kind, level)`. It is set when an array heading opens
(`parser.rs:827`, `parser.rs:875`) and cleared only when a heading at or above the array's
own level arrives (`parser.rs:966`) — that is, when the array genuinely ends. A deeper
heading becomes an element and deliberately leaves the state in place.

The table branch at `parser.rs:2664` tests only whether that state exists:

```text
if let Some((ref arr_parent_id, ref arr_field, ref arr_kind, _arr_level)) = pending_object_array
```

The level it carries is bound to `_arr_level` and never compared against the current
position. So "directly under the array heading" and "three levels deep inside an element"
are the same condition. The branch then calls `create_table_child_objects`
(`parser.rs:118`), pushes the new ids into the parent's array field, and `continue`s —
without finalizing the element currently being built, which is how the element is lost in
Rust.

`qmdc-py` and `qmdc-ts` carry their own copy of both the state and the gap. They are wrong
in different shapes rather than in the same one, which is itself evidence that nothing pins
this behaviour.

## Characterization: four shapes, no two implementations agree [[qmd70_finding_matrix: Finding]]

Measured on minimal probes, one object array with one element, varying only where the table
sits relative to the element's prose and fields. Each cell lists the `Goal` objects the
parse produces; `goal_a2` is the element that was written, `goals_0` is a phantom named
after the array field.

- category: parser
- related_to: [[#qmd70_table_scope]]
- solution: Use these four shapes as the acceptance matrix; after the fix every cell must read goal_a2 and nothing else, and the documented table-under-array case must still produce one object per row.

### The matrix [[qmd70_finding_matrix_table: text]]

- about: [[#qmd70_finding_matrix]]

| shape of the element | rs | py | ts |
| --- | --- | --- | --- |
| A — prose, table, nothing after | `goals_0` | `goal_a2` | `goal_a2` |
| B — table first, then fields | `goals_0` | `goal_a2`, `goals_0` | `goal_a2`, `goals_0` |
| C — fields, then table | `goal_a2`, `goals_0` | `goal_a2`, `goals_0` | `goal_a2`, `goals_0` |
| D — prose, table, then fields | `goals_0` | `goal_a2` | `goal_a2` |

Shape D is the real-world case from QMD-69. Reading the matrix:

- **Rust loses the element whenever the table is not preceded by a field** (A, B, D). The
  written heading, its explicit id and its fields vanish; only phantoms remain. This is
  silent data loss.
- **Python and TypeScript inject a phantom whenever the table is not preceded by prose**
  (B, C) and are correct otherwise (A, D).
- **Only shape C behaves the same in all three**, and it is wrong in all three.

### A third symptom: the array is terminated and siblings leak out [[qmd70_finding_matrix_leak: text]]

- about: [[#qmd70_finding_matrix]]

With a sibling element AFTER the table, Python and TypeScript do not merely add a phantom —
they close the array. Probe: `team` with `members: [User]`, element `alice` carrying a
field and then a table, followed by element `bob`.

| implementation | `team.members` | what happened to `bob` |
| --- | --- | --- |
| rs | `alice`, `members_0`, `bob` | stays a `User` element |
| py | `alice`, `members_0` | became a FIELD `team.bob`, Kind degraded `User` -> `__Object` |
| ts | `alice`, `members_0` | became a FIELD `team.bob`, Kind degraded `User` -> `__Object` |

So in py/ts one table inside one element silently removes every following sibling from the
array and re-parents them as scalar fields of the grandparent, with their declared Kind
replaced by `__Object`. A consumer querying `__kind = 'User'` simply stops seeing them.

## Table children compose their id differently in Rust [[qmd70_finding_id_compose: Finding]]

A narrow parity divergence found while probing, independent of the scope bug and visible
only when the array field name equals the parent object's id.

- category: parser
- related_to: [[#qmd70_table_scope]]
- affected_files: [qmdc-rs/src/parser.rs, qmdc-py/qmdc/parser.py, qmdc-ts/src/parser.ts]
- affected_functions: [create_table_child_objects]
- solution: Compose a table child's id by the same rule heading elements use, so the field segment is not doubled when the parent id already ends with it.

### Measurement [[qmd70_finding_id_compose_detail: text]]

- about: [[#qmd70_finding_id_compose]]

An object `items` carrying the array field `items`, with heading elements `first`/`second`
and a table:

| object | rs | py | ts |
| --- | --- | --- | --- |
| heading element | `items.first` | `items.first` | `items.first` |
| table child | `items.items.items_0` | `items.items_0` | `items.items_0` |

Rust composed `{parent_full_id}.{arr_field}.{local_id}` for table children
unconditionally, which doubles the segment here, while its own heading
elements and both other implementations produce `items.items_0`. When the field name
differs from the parent id all three agree (`team.members.members_0`), which is why this
never surfaced.

**Fixed (2026-09-23).** Rust now follows the same three-case rule Python's `resolve_child_id`
documents: a dot-ID array field is used as the prefix directly, a field name equal to the
parent's id is not repeated, and otherwise the field name sits between parent and local id.

Code review then found the same function carried a SECOND copy of the rule that had also
drifted: when the array's parent is a system container (`__Workspace` / `__Namespace`) the inline
code prefixed the parent id, giving `rows_ns_rows_0` where Python and TypeScript — and Rust's own
shared helper — give the bare `rows_0`. Nothing pinned it, because no fixture put a table-fed
array directly under a namespace root, and `make validate-compare` compares only validation
errors, not ids. Rust's table-child path now CALLS `resolve_child_id` instead of re-implementing
it, so both branches have one rule; that is what fixed the second divergence as a side effect.

Pinned by `tests/workspace/table-child-id-composition/001-table-child-id-field-equals-parent`
and `002-table-child-id-system-parent`,
in the SQL harness rather than as a parser microtest. The reason is worth recording: this shape
is a TOP-LEVEL array, and `parse -> rebuild` is not round-trip stable for it — rebuild emits a
wrapper heading plus a nested array heading. The microtest harness checks the round trip, so a
microtest here would have failed for an unrelated pre-existing reason and buried the id
assertion. That rebuild limitation is untouched by this task and remains unpinned.

## The lost element is visible on the validator, LSP and MCP surfaces too [[qmd70_finding_surfaces: Finding]]

Found by re-checking this triage rather than by the original pass, and it is the same class of
gap: the triage had covered only the PARSE surface. When the element that disappears carries an
explicit id, anything referring to it becomes a false `broken_link` — so the bug is observable
wherever references are validated, and there the three implementations disagree.

- category: parser
- related_to: [[#qmd70_table_scope]]
- affected_files: [tests/cli/015-validate-table-in-array-element, tests/lsp/microtests/diagnostics/035-table-in-array-element]
- solution: No extra code change — the fix for [[#qmd70_goal_a1]] closes this too, because the element stops disappearing. What was missing was the coverage; three cases now assert it on the validator, LSP and MCP.

### Measurement [[qmd70_finding_surfaces_detail: text]]

- about: [[#qmd70_finding_surfaces]]

A workspace whose `Bug` object carries `tracks: [[#goal_a2]]`, with goal A2 holding prose then a
table (shape D — the real-world case):

| surface | rs | py | ts |
| --- | --- | --- | --- |
| `qmdc workspace validate` | `broken_link` on `[[#goal_a2]]` | clean | clean |
| LSP `textDocument/diagnostic` | false QMDC001 | — | — |
| MCP `qmdc_validate_references` | false QMDC001 | — | — |

The MCP index itself is short an object — it reports 3 objects where the file declares 4 — so
this is not a diagnostic-formatting artefact but the lost object showing through.

The user-facing consequence is worse than the parse-level one: an editor puts a red squiggle on
a reference to a goal that is written a few lines further down the same file, and the CLI calls
a valid workspace broken. And because py and ts keep the element in this shape, the same file is
valid or invalid depending on which parser reads it.

### Why the first pass missed it [[qmd70_finding_surfaces_why: text]]

- about: [[#qmd70_finding_surfaces]]

The four microtests assert the parse output, which is where the defect originates, and the
reasoning stopped there — a parse bug felt like a parse-surface bug. But an object vanishing from
the index is not a formatting difference; it propagates to every consumer of the index. Nothing
in the fixture set referenced a lost element, so no surface past the parser was ever exercised.

The lesson is the same one QMD-69 recorded: coverage has to be reasoned about per SURFACE, not
per cause. One fixture per code path is not the same as one fixture per observable behaviour.

## Mandatory regression tests (data-driven, failing as designed) [[qmd70_finding_tests: Finding]]

Seven data-driven cases across three surfaces. Four parser microtests, one per shape of the
characterization matrix, plus three added when re-checking the triage found that the lost element
also surfaces on the validator, the LSP and MCP ([[#qmd70_finding_surfaces]]). All are plain
fixtures plus expected JSON — no new test code — and the microtests each run in all three
implementations through all three aspects (parse, rebuild, rebuild-text).

The documented feature needs no new guard: the shipped `tests/parser/032-table` already pins a
table directly under an array heading, and `tests/parser/065-text-table-in-array` pins a table
inside an element wrapped in a `text` field. Both stay green, as do `042-table-one-row`,
`064-text-field-with-table` and `089-comments-preserve-tables` — verified in all three parsers.

- category: testing
- related_to: [[#qmd70_table_scope]]
- affected_files: [tests/parser/212-table-in-array-element-after-prose.qmd.md, tests/parser/213-table-in-array-element-first.qmd.md, tests/parser/214-table-in-array-element-after-field.qmd.md, tests/parser/215-table-in-array-element-between.qmd.md, tests/cli/015-validate-table-in-array-element, tests/lsp/microtests/diagnostics/035-table-in-array-element]
- solution: Keep all seven as the acceptance gate; they turn green only when the array context is scoped to the array's own content, and they are what forces the three implementations to agree across all four surfaces.

### Where the expected JSON comes from [[qmd70_finding_tests_expected: text]]

- about: [[#qmd70_finding_tests]]

Not invented. Two shipped fixtures already establish how a table outside an array context is
represented, and the expected JSON applies that same representation inside an element:

`tests/parser/089-comments-preserve-tables` keeps a table verbatim in `__comments` with an
`after` anchor naming the preceding field. `tests/parser/065-text-table-in-array` keeps a
table verbatim in a declared `text` field of an array element, leaving the array untouched.
And undeclared prose inside an array element already becomes a `__comments` entry today.

A bare table inside an element is undeclared content, so it takes the undeclared-content
path: a `__comments` entry, verbatim, anchored `__self` when it follows prose and named after
the preceding field when it follows one. Shapes A and D already produce exactly that in
Python and TypeScript, so two of the four expected files are literally the current correct
output of two implementations — which is why this is a scope bug, not a missing feature.

### Measured failures [[qmd70_finding_tests_failures: text]]

- about: [[#qmd70_finding_tests]]

Recorded per the Bug triage exception. Every failure is one of the four new fixtures; no
pre-existing or unrelated case fails.

| fixture | shape | rs | py | ts |
| --- | --- | --- | --- | --- |
| `212-table-in-array-element-after-prose` | A | **fail** | pass | pass |
| `213-table-in-array-element-first` | B | **fail** | **fail** | **fail** |
| `214-table-in-array-element-after-field` | C | **fail** | **fail** | **fail** |
| `215-table-in-array-element-between` | D | **fail** | pass | pass |
| `cli/015-validate-table-in-array-element` | D, via `workspace validate` | **fail** | pass | pass |
| `diagnostics/035-table-in-array-element` (`expected.json`) | D, via LSP | **fail** | — | — |
| `diagnostics/035-table-in-array-element` (`mcp-expected.json`) | D, via MCP | **fail** | — | — |

Case counts, reading the JUnit reports: rs `rs-microtests.xml` 222/4,
`rs-microtests-rebuild.xml` 183/4, `rs-microtests-text.xml` 211/4 — twelve failures, the same
four fixtures across the three aspects. ts `ts-parser.xml` 616/6 and py 6 — two fixtures
across three aspects each. So this task adds 12 failing cases in Rust and 6 in each of the
other two, and nothing else fails.

These four fixtures are the ONLY failures on the branch: QMD-69 landed green at 3325 cases,
so `make test` now reports exactly 24 failures, all of them these. That is intended under the
Bug triage exception — a regression test is written before the fix and is expected to fail until
[[#qmd70_goal_a1]] lands. Anyone treating a red suite here as a broken branch should check the
failing case names first.

The columns are the matrix, restated as tests: the two shapes where Python and TypeScript are
already correct are exactly the two where Rust loses the element. After the fix all twelve
columns read pass.

## Prose between an array heading and its table splits the parsers [[qmd70_finding_prose_gap: Finding]]

Found while writing the fixture for [[#qmd70_goal_b2]], not by the triage. A paragraph between
an object-array heading and the table that feeds it is read differently by Rust than by the
other two, and nothing in the corpus covered it.

- category: parser
- related_to: [[#qmd70_table_scope]]
- affected_files: [qmdc-rs/src/parser.rs, qmdc-py/qmdc/parser.py, qmdc-ts/src/parser.ts]
- solution: ANSWERED by precedent, and the answer is Rust's: prose does not break the connection. FIXED in Python and TypeScript — their comment scan now stops before a table belonging to the array container. Pinned by `tests/cli/020-parse-prose-between-array-heading-and-table`, green in all three.

### Measurement [[qmd70_finding_prose_gap_detail: text]]

- about: [[#qmd70_finding_prose_gap]]

```text
## Items [[items: [Item]]]

Prose between the heading and the table.

| col_a | col_b |
|-------|-------|
| r1 | v1 |
```

| impl | elements created | `__syntax.items` |
| --- | --- | --- |
| rs | `items.items_0` | `table` |
| py | none | `headers` |
| ts | none | `headers` |

Rust's `has_table_after` lookahead skips paragraphs, so it still sees the table as the array's
own content. Python and TypeScript have no such lookahead: the paragraph takes the comment path
first, and the table follows it there, leaving the array empty.

**Answered (2026-09-23) by looking at what the three already agree on.** The question was framed as
"neither reading is obviously wrong", which was a failure to look for precedent. There is one, and
it settles it.

Take the SAME shape but put a heading element after the prose instead of a table:

```text
## Doc [[doc]]

### Items [[items: [Item]]]

Prose between.

#### A [[a]]

- role: x
```

All three implementations agree here: `items` holds the element AND the prose is preserved as
`__comments` anchored `__self`. Prose is separated out; the structural content still counts. The
same is true for a `text` field — prose before a table inside one is part of the field's value in
all three.

Measured against that precedent, Rust's reading of the table case is exactly right and Python's and
TypeScript's is the deviation:

| impl | `items` | `__comments` |
| --- | --- | --- |
| rs | the converted child | the prose, anchored `__self` |
| py, ts | empty | the prose AND the table |

So the fix belongs in Python and TypeScript, not in Rust.

A fixture was written earlier pinning Python's reading, on the grounds that Python is the reference
implementation. That was wrong — mechanical deference instead of checking the precedent — and it was
replaced. Worth recording as a method note: "Python is the reference" resolves a tie, it does not
answer a design question the corpus already answers.

One caveat found while writing the replacement. The sentence in `docs/format/arrays.qmd.md` saying
the table must sit "directly under the array heading" was added by THIS task and cannot be cited as
precedent for it.

**Fixed (2026-09-23) in Python and TypeScript.** Each has a forward scan that decides where a
comment block ends, and neither stopped at a table. Both now stop before a table that belongs to the
array container, using the same predicate as the conversion branch. In TypeScript the edit went
first onto the block-level comment branch, which this shape never reaches — it arrives through the
PARAGRAPH branch, since the prose opens the block; the fixture caught that immediately.

One defect in the same area is NOT fixed: with a TOP-LEVEL array (no structural parent) Rust
converts the table but **drops the prose entirely** — no `__comments` at all — while Python and
TypeScript keep it. Now that the parented form is settled, this is a Rust bug against the agreed
reading. It needs the same treatment the parented case got in `create_table_child_objects`, and it
is recorded in [[#qmd70_finding_rs_other_paths]] rather than fixed here.

This is the same family as the main defect (where does a table belong?) but a different cell:
the main defect is about a table inside an ELEMENT, this one is about a table separated from
its CONTAINER by prose. The fix for [[#qmd70_goal_a1]] does not touch it, and it was verified
still divergent after that fix landed.

## Two tables under one array heading lose rows in Rust [[qmd70_finding_two_tables: Finding]]

Found by the code review, reproduced in all three parsers. PRE-EXISTING and out of scope, recorded
so the discovery is not lost. Rust converts BOTH tables, both children take the same `local_id`,
the second overwrites the first, and the first table's rows vanish with no error.

- category: parser
- related_to: [[#qmd70_finding_id_compose]]
- affected_files: [qmdc-rs/src/parser.rs]
- solution: Fixed — Rust closes the array context after the container's table, as Python and TypeScript already did, and the leftover table is preserved on the parent instead of dropped. Pinned by `tests/cli/016-parse-two-tables-under-array-heading`.

### Measurement and options [[qmd70_finding_two_tables_detail: text]]

- about: [[#qmd70_finding_two_tables]]

```text
# Team [[team]]

## Members [[members: [User]]]

| name |
|------|
| Alice |

| name |
|------|
| Bob |
```

| impl | `team.members` | User objects |
| --- | --- | --- |
| rs | two refs, both `team.members.members_0` | one, holding **Bob** |
| py | one ref | one, holding Alice |
| ts | one ref | one, holding Alice |

Python clears `pending_object_array` after converting a container table and TypeScript does too;
Rust never does, so the second table converts as well and collides on `members_0`. Verified
pre-existing against `HEAD`, and the new scope guard is transparent for this input — no element is
open, so the guard holds for both tables exactly as before.

**Fixed (2026-09-23).** Rust now clears the array context after the container's table converts, so
the leftover table falls through to the comment path. Measuring the fix turned up two more
divergences in the same shape, both fixed for parity:

- Rust DROPPED the leftover table instead of keeping it. The parent leaves `current_obj` when the
  array heading opens, so the comment path had no in-flight object to attach to. Rust now
  remembers the parent and its anchor at that moment and writes the comment into `objects_map`,
  reusing the pattern its paragraph-comment path already had.
- TypeScript reset the comment anchor to `__self` for an `[[field: [Kind]]]` heading. Such a
  heading declares a FIELD on the parent, not a new object, so it must not reset the parent's
  anchor. All three now report `after: lead`.

Three anchor choices were measured before settling on Python's. Anchoring on the array field
(`members`) reads better and would preserve document order, but `rebuild` then DROPS the table
entirely — it cannot place a comment anchored on a heading-declared array field. See
[[#qmd70_finding_rebuild_anchor]] for what that costs and why this is pinned in the CLI corpus
rather than as a parser microtest.

## Rust drops non-table content after an array container's own table [[qmd70_finding_rs_after_array: Finding]]

Found by the SECOND code-review pass, by two reviewers independently. PRE-EXISTING, and it was the
gap in the first fix for [[#qmd70_finding_two_tables]] — that one taught Rust to keep a trailing
TABLE, and only a table. FIXED structurally, which also removed the earlier workaround.

- category: parser
- related_to: [[#qmd70_finding_two_tables]]
- affected_files: [qmdc-rs/src/parser.rs]
- solution: Fixed — the array's parent now stays in `current_obj` instead of being finalized at the array heading, so every path that writes to the in-flight object keeps working. Pinned by `tests/parser/219-field-after-array-table` and `tests/cli/017-parse-prose-after-array-table`.

### Measurements [[qmd70_finding_rs_after_array_detail: text]]

- about: [[#qmd70_finding_rs_after_array]]

Everything following an object-array heading's own table, other than another table, is lost in Rust
and kept by Python and TypeScript. Three shapes measured, all with the same head
(`# Team [[team: Group]]`, `- lead: Ann`, `## Members [[members: [User]]]`, the Alice table):

| what follows | rs | py and ts |
| --- | --- | --- |
| prose | dropped | `__comments` anchored `lead` |
| a field, `- note: something` | **the field itself is lost** | `note: something` |
| a `yaml` field heading, then prose | dropped | `__comments` anchored `lead` |

The field case is the worst: a declared field disappears, not merely prose.

The cause was structural rather than a missing branch. An object-array heading finalized its parent
out of `current_obj`, and Rust's field, comment and reference paths all write to the in-flight
object, so once the parent was gone they had no target. The first fix worked around this for tables
only, by remembering `(parent_id, anchor)` in a dedicated variable. Generalising that per content
type would have meant re-implementing every path.

**Fixed (2026-09-23) by not finalizing the parent.** The parent now stays in `current_obj` for the
whole array, so every path keeps working unchanged. Three supporting changes made that possible:

- The parent's slot in `objects_map` is reserved with a skeleton entry, so the array's children —
  inserted as soon as the table converts — still come after their parent in the output.
  `finalize_object` already treated an entry without `__line` as a skeleton and overwrote it in
  place without reporting a duplicate, so no new mechanism was needed. The skeleton carries
  `__kind`, because `resolve_child_id` reads it to recognise a `__Workspace` / `__Namespace`
  parent whose children take a bare local id.
- The array setup and the table conversion write to the in-flight parent when it is the one they
  mean, and fall back to the map for the case where an element heading has already finalized it.
- The scope guard from goal A1 changed from `current_obj.is_none()` to comparing the current
  object against the array's parent — which is what Python and TypeScript always did. The old form
  only held because the parent was being finalized; keeping it in flight made the two predicates
  genuinely different rather than coincidentally equal, and the SQL fixture for a top-level array
  caught that immediately. A top-level array owns itself from the stack rather than from
  `current_obj`, so the guard checks the stack top in that case.

The workaround variable had no assignment site left afterwards and was deleted rather than kept as
dead code — the reason the review's earlier "never cleared" note is moot.

Two divergences of the same family remain and are NOT part of this: a `yaml` field heading followed
by prose, and an indented block under a trailing list field. Both reproduce with **no array
anywhere in the document**, so they are separate defects in other paths — see
[[#qmd70_finding_rs_other_paths]].

## Rust drops trailing content in two other paths [[qmd70_finding_rs_other_paths: Finding]]

Two divergences of the same family as [[#qmd70_finding_rs_after_array]], separated out because both
reproduce with NO array anywhere in the document. Not fixed: different code paths, and neither is
reachable through anything QMD-70 changed.

- category: parser
- related_to: [[#qmd70_finding_rs_after_array]]
- affected_files: [qmdc-rs/src/parser.rs]
- solution: The `yaml` half is FIXED — `tests/cli/018-parse-yaml-field-then-prose` is green in all three. The indented-block half is NOT, and it is not the same defect: see [[#qmd70_finding_indented_block]].

### Measurements [[qmd70_finding_rs_other_paths_detail: text]]

- about: [[#qmd70_finding_rs_other_paths]]

**A `yaml` field heading followed by prose.** Rust drops the prose; Python and TypeScript keep it
anchored on the field before the heading.

The input is a `Group` object with a `lead` field, then a `## Conf [[conf: yaml]]` heading carrying
a YAML fence (`a: 1`), then a paragraph.

| impl | `conf` | `__comments` |
| --- | --- | --- |
| rs | `"Note after the yaml field."` | none |
| py, ts | `{"a": 1}` | the paragraph, anchored `lead` |

Worse than "the paragraph is dropped", which is how this was first recorded: the paragraph
OVERWRITES the field's value, so the YAML data is lost as well. Measured after the array fix
landed, so it is not a consequence of it.

**An indented block under a trailing list field.** Rust anchors on the wrong field and keeps the
raw indented text; Python and TypeScript anchor on the list field and normalise the block.

| impl | anchor | content |
| --- | --- | --- |
| rs | `lead` | the table's raw indented source |
| py, ts | `note` | `- ic` / `- x` |

Both were measured with a plain object and no array, which is what separates them from the array
case that this task fixed.

### The yaml half, and how it turned out contained [[qmd70_finding_rs_other_paths_fixed: text]]

- about: [[#qmd70_finding_rs_other_paths]]

**Fixed (2026-09-24).** The measurement below said this needed a shared accessor across five
`pending_text_field` write sites. It turned out to need one, because a second defect in the same
place removed the need for the other four.

Rust never closed the field after its fence. Python clears `pending_yaml_field` straight after, and
Rust kept `pending_text_field` live, so a paragraph following the fence was appended INTO the field
as text and overwrote the parsed object — `conf: {a: 1}` became `conf: "Note after the yaml field."`.
That is the data loss recorded above, and closing the field on the fence fixes it.

With the field closed there, the parent can stay in flight — which is what gives the trailing
paragraph somewhere to go — and only the FENCE site still needs to know where the parent lives. The
other four `pending_text_field` consumers are never reached for a `yaml`/`json` field any more. So
the fix is two small changes rather than a refactor, and the accessor is not needed after all.

A `text` field is deliberately left alone: a fence there is part of the content and more content may
follow, so the field stays open and the parent stays finalized exactly as before.

One pre-existing divergence in the same area was measured and NOT touched: for INVALID YAML, Rust
stores the fence markers with the text (`` ```yaml\n…\n``` ``) and no `__syntax`, where Python and
TypeScript store the content alone with `yaml_object`. Verified against the pre-fix build — identical
there, so it is not a regression.

### Why the array fix does not generalize to the indented-block case [[qmd70_finding_rs_other_paths_why: text]]

- about: [[#qmd70_finding_rs_other_paths]]

The `yaml` case has the SAME cause as the array case — a heading that declares a field on its parent
closes that parent out of `current_obj`, leaving later content with no target — and the same first
move works: reserve the parent's slot with a skeleton and keep it in flight. That was tried, and it
does keep the parent reachable with no other case regressing (157 Rust tests, same four failures).

It stops being a contained fix at the second step. The array's value had two write sites, so both
could take the in-flight parent directly. A `yaml` / `json` / `text` / table field routes its value
through `pending_text_field`, which is consumed at FIVE separate sites, all of them writing straight
into `objects_map`. With the parent in flight and those sites unchanged, the field ends up empty —
measured. Making them all work means introducing one shared accessor for "this parent's field slot,
wherever the parent currently lives" and routing every site through it.

That accessor is the right design, and it is the honest generalization of what this task learned.
But `pending_text_field` also carries every `text` field in the corpus, which is the most common
construct in the format, and the guard that had to change once already for exactly this reason
(`current_obj.is_none()` → compare against the parent) shows how such assumptions are spread around.
So it belongs in a change of its own, with its own triage and review, rather than as a third
expansion of QMD-70.

The indented-block case is not the same cause at all: the block is attached to the wrong field and
kept as raw source rather than normalised, which is list-item handling, not parent lifetime.

## Rust rebuilt blockquote comments instead of slicing them [[qmd70_finding_blockquote: Finding]]

Started as "Rust drops an empty blockquote", raised as an aside by the first review pass. Measuring
it showed one cause behind three divergences. FIXED.

- category: parser
- affected_files: [qmdc-rs/src/parser.rs]
- solution: Fixed — the blockquote comment is sliced verbatim from the source, as tables already were, and only the outermost level of a nested quote emits. Pinned by `tests/parser/221-empty-blockquote-comment`, `223-blockquote-leading-blank-line` and `224-nested-blockquote-comment`.

### One cause, three symptoms [[qmd70_finding_blockquote_detail: text]]

- about: [[#qmd70_finding_blockquote]]

Rust reconstructed a blockquote's comment content by collecting TEXT events and re-adding the `>`
prefixes. Python and TypeScript slice the raw source. Three consequences, all measured against a
plain object with no array involved:

| input | rs before | py and ts |
| --- | --- | --- |
| `>` alone | comment dropped — no text events, so the emit was skipped | `">"` |
| `>` then `> text` | `"> text"` — the blank quoted line lost | `">\n> text"` |
| `> > nested` | `"> nested"` — one level lost | `"> > nested"` |

Only the first was reported. The other two were found while checking the fix, and neither had a
fixture, so nothing would have caught them.

Slicing the source fixes all three, and it is the pattern the codebase already uses for tables in
comments (`raw_table_slice`, added precisely because reconstruction normalised the separator row and
diverged from the other two parsers). The same reasoning applies here; the blockquote path had
simply never been converted.

Slicing introduced one new problem and the nested case caught it: `TagEnd::BlockQuote` fires once
per level, so `> > nested` emitted the same slice twice. The branch now tracks depth and emits only
when the outermost level closes.

## A second table under an array heading emits no diagnostic [[qmd70_finding_no_diagnostic: Finding]]

Raised by both code-review passes with an in-repo precedent. A design question, not a defect.

- category: parser
- related_to: [[#qmd70_finding_two_tables]]
- solution: RESOLVED — the diagnostic exists now (`extra_table_in_array`). The cross-surface cost was real but paid: three parsers, the LSP and MCP matrix, and five documentation files.

### The precedent [[qmd70_finding_no_diagnostic_detail: text]]

- about: [[#qmd70_finding_no_diagnostic]]

**Resolved (2026-09-23).** The operator decided the construct is an error, so the silence is gone —
see [[#qmd70_finding_extra_table]]. The original text of this finding is kept below because the
reasoning is what led to the decision.

A second table under one array heading was preserved as comment content and nothing was reported on
any surface: `workspace validate` returned `[]` in all three parsers, and the LSP and MCP were
silent. The row the author almost certainly meant as an array element was quietly reclassified as
prose.

`docs/format/validation-errors.qmd.md` documents the opposite treatment for the closest analogue: a
numbered list in a heading-syntax array is *both* preserved in `__comments` *and* reported as
`ordered_list_in_array`. By that precedent an `extra_table_under_array` error would be defensible —
a second table directly under an array heading has no reading other than "more rows".

Not done here for two reasons. A new error type must be added to the three parsers, the LSP and MCP
surface matrix, and five documentation files, which is a feature rather than a fix. And the silence
is a strict improvement on what this task started from: Rust used to lose a row outright, so the
choice today is between silent preservation and silent loss, not between silence and an error.

## Rebuild cannot place a comment anchored on an array field [[qmd70_finding_rebuild_anchor: Finding]]

Discovered while choosing the anchor for [[#qmd70_finding_two_tables]]. NOT fixed: the anchor model
has no way to say "after the array's table", so no anchor value gives a faithful round trip.

- category: rebuild
- related_to: [[#qmd70_finding_two_tables]]
- affected_files: [qmdc-rs/src/parser.rs, qmdc-py/qmdc/parser.py, qmdc-ts/src/parser.ts]
- solution: RESOLVED by removing the construct rather than the limitation — a second table under one array heading is now an `extra_table_in_array` error, so there is no such content left to place. See [[#qmd70_finding_extra_table]].

### What was measured [[qmd70_finding_rebuild_anchor_detail: text]]

- about: [[#qmd70_finding_rebuild_anchor]]

A table following an object-array heading's own table is preserved as the parent's comment. Two
anchors are expressible and both lose on `parse | rebuild`:

| anchor | rebuild result |
| --- | --- |
| `lead` — the field before the array heading, what all three now report | the table is emitted after `- lead: Ann`, i.e. ABOVE the `## Members` heading; document order changes |
| `members` — the array field, which reads correctly | the table is DROPPED entirely; rebuild has no placement rule for a comment anchored on a heading-declared array field |

All three implementations rebuild identically, so this is a shared limitation and not a parity
defect. The parser harness has a content-motion check that the first case legitimately trips, which
is why the behaviour is pinned in the CLI corpus (`tests/cli/016-…`, parse only) instead. The CLI
corpus is data-driven across all three parsers, so coverage is not weaker — only narrower.

Related: the same round-trip weakness appears for a top-level array on its own, where `rebuild`
emits a wrapper heading plus a nested array heading. That is why
[[#qmd70_finding_id_compose]]'s cases live in the SQL harness.

## TypeScript drops the separator row of a header-only table [[qmd70_finding_hdr_only: Finding]]

Also found by the code review, also pre-existing, and unrelated to arrays: it reproduces in a
plain object with no array anywhere. Recorded here only because the review surfaced it.

- category: parser
- affected_files: [qmdc-ts/src/parser.ts]
- solution: Fixed — TypeScript's comment slice took the maximum end line instead of overwriting it. Pinned by `tests/parser/218-header-only-table-in-comment`. The empty-blockquote half turned out to be a separate, wider defect — see [[#qmd70_finding_blockquote]].

### Measurement [[qmd70_finding_hdr_only_detail: text]]

- about: [[#qmd70_finding_hdr_only]]

A table with a header and separator row but NO data rows, carried as comment content, keeps the
separator row in Rust and Python and loses it in TypeScript. With any data row present all three
agree exactly, so the divergence is confined to the empty case.

The reviewer suspected the loosened comment-path guard had made it reachable; a plain-object
reproduction shows it was always reachable.

**Fixed (2026-09-23).** The comment path's forward scan assigned `endLine = scanTok.map[1]` for
every token it walked past. A child token's map can be NARROWER than its container's: a table with
no data rows is `table_open [4,6]` while its `thead_open` and `tr_open` are `[4,5]`, so the
assignment SHRANK the slice and cut the separator row off. Taking the maximum fixes it. Rust and
Python were never affected because neither re-derives the end from child tokens this way.

## A table under a primitive array field is now an error [[qmd70_finding_table_in_array: Finding]]

Found while laying out the open decisions: a Markdown table under `[[field: array]]` was silently
dropped by all three parsers, which also disagreed about whether the field existed. Operator decided
it must be an error. FIXED.

- category: parser
- affected_files: [qmdc-py/qmdc/parser.py, qmdc-rs/src/parser.rs, qmdc-ts/src/parser.ts, qmdc-ts/src/workspace.ts, docs/format/validation-errors.qmd.md, docs/format/arrays.qmd.md]
- solution: Fixed — new `table_in_array` error type, modelled on `ordered_list_in_array`. Pinned by `tests/parser/225-table-in-primitive-array` and `tests/lsp/microtests/diagnostics/036-table-in-primitive-array` (LSP + MCP).

### The decision and what it cost [[qmd70_finding_table_in_array_detail: text]]

- about: [[#qmd70_finding_table_in_array]]

Before: the table vanished with no diagnostic, and the three did not even agree on the object —
Rust produced `tags: []` while Python and TypeScript produced no `tags` field at all.

The operator's reason for erroring rather than picking a reading: it is not possible to say what
object a table should become under a primitive array. A primitive array holds scalars, a table has
columns, and nothing defines the mapping. Under an OBJECT array (`[[field: [Kind]]]`) it IS defined —
one row is one object — so the distinction is the declared field type, not the table.

The operator's first instruction was to forbid tables in array fields "altogether", which reads two
ways. The wider one would have deleted the table-to-object-array feature, which is documented in
`docs/format/arrays.qmd.md` and `docs/guides/qmdc-guide.qmd.md`, pinned by shipped fixtures
`032-table`, `042-table-one-row`, `065-text-table-in-array` and `089-comments-preserve-tables`, and
released in 1.0.2 — every existing document using it would start erroring. Measuring that blast
radius before writing anything is what surfaced the ambiguity; the narrow reading was confirmed.

Modelled on `ordered_list_in_array`, the only other construct forbidden in an array field: the array
stays empty, the content is preserved verbatim in `__comments` so the round trip is lossless, and a
`__ParsingError` carries `type`, `field`, `object` and `line`.

The three implementations needed different edits because they represent a primitive array field
differently. Python and TypeScript hold a `pending_array_field` and create the field only when a
list arrives. Rust creates the field eagerly at the heading and routes the rest through
`pending_text_field` with type `"array"` — which is why its table was consumed by the text-field
branch and dropped there rather than never reaching a handler.

No LSP or MCP change was needed: both surface `__ParsingError` generically, filtering only
`duplicate_id`. That was verified by measurement, not assumed — the new error appears in
`workspace validate` in all three parsers, in LSP diagnostics, and in `qmdc_validate_references`.

The documentation cascade was five files: the error definition, the arrays spec, the guide's error
table, the LSP surface matrix, and the per-document validation guide. One further artifact is
knowingly stale: `docs/.qmdc-semantic/hints.json` does not contain the new `err_table_in_array`
object, because hints are derived from `embeddings.db`, whose refresh needs an embedding provider and
is a release-time step per `RELEASING.md`.

## A second table under one array heading is now an error [[qmd70_finding_extra_table: Finding]]

The operator's decision, and it dissolved [[#qmd70_finding_rebuild_anchor]] instead of working
around it. FIXED in all three.

- category: parser
- related_to: [[#qmd70_finding_two_tables]], [[#qmd70_finding_rebuild_anchor]], [[#qmd70_finding_table_in_array]]
- affected_files: [qmdc-py/qmdc/parser.py, qmdc-rs/src/parser.rs, qmdc-ts/src/parser.ts, qmdc-ts/src/workspace.ts, docs/format/validation-errors.qmd.md, docs/format/arrays.qmd.md, docs/guides/qmdc-guide.qmd.md]
- solution: Fixed — new `extra_table_in_array` error, same shape as `table_in_array`. Pinned by `tests/parser/222-extra-table-under-array-heading`.

### Why an error dissolves the rebuild problem [[qmd70_finding_extra_table_detail: text]]

- about: [[#qmd70_finding_extra_table]]

The second table could not be placed on rebuild under ANY anchor: `lead` (the field before the array
heading) moved it above that heading, `members` made rebuild drop it outright, and the anchor model
cannot express "after the array's own table". Three implementations agreed on the wrong layout, so it
was not a parity defect but a gap in the format.

Making the construct an error removes the content that had nowhere to go. The mechanism is the
corpus's own rule, not a workaround: both the Rust and Python microtest harnesses skip the round-trip
check for a document that has parsing errors — "rebuild of invalid docs is undefined". So the fixture
that used to pin the motion now passes by being invalid, which is the honest reading.

The rule is scoped to the array CONTAINER's own content. Four neighbouring shapes were measured after
the change and all three parsers agree on every one:

| shape | result |
| --- | --- |
| one table under the array heading | converts, no error |
| **two tables under the array heading** | **first converts, second is an error** |
| table inside an array element | the element's comment content — the main QMD-70 case, untouched |
| prose, then a table under the heading | converts, prose kept as comment — settled earlier |
| container table, then an element with its own table | both valid |

Detection needed a new piece of state in each parser: the array context is cleared as soon as the
container's table is consumed, so "a second table" is not visible from it. Each now records
`(parent_id, field_name)` when the container's table converts, and clears that at the next heading —
a heading either opens an element, in which case a following table belongs to the element, or leaves
the array.

One pre-existing behaviour surfaced while writing the fixture and is worth knowing: once a document
has any parsing error, Python sorts the output by `__line`, and a table-fed child has no `__line`, so
it sorts BEFORE its parent. Rust and TypeScript match. Odd, but committed and intended, with a
comment saying so.

`tests/cli/016-parse-two-tables-under-array-heading` was deleted: it existed only because the parser
corpus tripped on the rebuild motion, which no longer happens, so the microtest covers the shape and
the CLI case was a duplicate input.

## An array cannot be fed by a table AND heading elements [[qmd70_finding_mixed_array: Finding]]

Triage question 1, left open because the corpus had no example either way. Measurement showed the
behaviour is worse than the question assumed, and the operator decided it is an error. FIXED.

- category: parser
- related_to: [[#qmd70_finding_questions]], [[#qmd70_finding_extra_table]]
- affected_files: [qmdc-py/qmdc/parser.py, qmdc-rs/src/parser.rs, qmdc-ts/src/parser.ts, qmdc-ts/src/workspace.ts, docs/format/validation-errors.qmd.md, docs/format/arrays.qmd.md, docs/guides/qmdc-guide.qmd.md]
- solution: Fixed — new `mixed_array` error. Pinned by `tests/parser/226-mixed-array-table-then-heading`, with `227-element-then-table-is-element-content` pinning the valid neighbour, and `tests/lsp/microtests/diagnostics/037-mixed-array` covering the LSP and MCP.

### What was actually happening [[qmd70_finding_mixed_array_detail: text]]

- about: [[#qmd70_finding_mixed_array]]

The triage described the mixed array as "expressible only as a side effect of this bug". It was worse
than that. Measured on a `[User]` array fed by a table, followed by `#### Bob [[bob]]`, all three
parsers agreed:

- `bob` did NOT join `members`
- it became a plain scalar field on the parent, `bob: "[[#team.bob]]"`
- its Kind was degraded from the declared `User` to `__Object`
- nothing was reported on any surface

So an author who wrote a Kind got an object without it, and no warning. That is what made the
decision easy.

The rule is scoped tightly, and each boundary was measured across all three:

| shape | result |
| --- | --- |
| **table, then an element heading** | **`mixed_array` error** |
| element heading, then a table | the table is the element's content — valid, the main QMD-70 case |
| two element headings, no table | valid |
| table, then a `text` field heading | valid — a field on the parent, not an element |
| table, then a heading at the array's own level | valid — a sibling, not an element |

Two conditions therefore gate the error: the heading must be DEEPER than the array's own level, and
it must carry no field type. The array's level had to be added to the state the
[[#qmd70_finding_extra_table]] fix introduced, since the array context itself is already gone by
then.

The element's parse is left exactly as it is rather than being discarded. Nothing the author wrote
disappears — they can still see the object — and the error names the problem. That differs from
`table_in_array` and `ordered_list_in_array`, where the offending content is a list or a table with
nowhere to go and is preserved as a comment instead.

In Rust the check could not live where the existing flag is cleared (`Tag::Heading`), because the
header — and therefore its field type — is only parsed at `TagEnd::Heading`. The clear moved there
too.

## An indented block under an inline field is broken in all three [[qmd70_finding_indented_block: Finding]]

Split out of [[#qmd70_finding_rs_other_paths]] once the `yaml` half was fixed: this is a DIFFERENT
defect, it affects all three implementations rather than Rust alone, and closing it needs a decision
rather than a fix. The last deliberately failing case,
`tests/cli/019-parse-indented-block-under-list-field`.

- category: parser
- related_to: [[#qmd70_finding_rs_other_paths]]
- solution: Operator decision. Either the construct becomes a parsing error — consistent with [[#qmd70_finding_table_in_array]], [[#qmd70_finding_extra_table]] and [[#qmd70_finding_mixed_array]], all added this session for constructs with no defined meaning — or two separate refactors are needed, one per direction of the disagreement.

### Nobody is right [[qmd70_finding_indented_block_detail: text]]

- about: [[#qmd70_finding_indented_block]]

A Markdown table indented under an inline field (`- note: something`, then the table two spaces in):

| impl | anchor | content |
| --- | --- | --- |
| py, ts | `note` — correct | `- ic` / `- x` — the table is DESTROYED, reduced to its cell texts as list items |
| rs | `lead` — wrong, that is the field before it | the table verbatim — correct, though with the source indentation |

So Python and TypeScript have the right anchor and Rust has the right content, and the correct output
is neither: `after: note` with the dedented table. Confirmed by `parse | rebuild`, which round-trips
in none of them — Python emits the table as a bullet list, Rust emits it above `note`.

Why each is wrong is structural, and the two causes are unrelated:

- Rust commits a list field and moves the comment anchor at `TagEnd::Item`, but the table lives
  INSIDE that item, so `TagEnd::Table` fires first and the anchor is still on the previous field.
  Fixing it means committing list fields earlier — a restructure of the list path.
- Python and TypeScript are token-based and commit `note` first, so their anchor is right; they then
  collect the table's inline cells into `comment_list_items`, which is what turns it into
  `- ic` / `- x`. Fixing that is in their list-comment path.

The neighbouring shape is worse in Rust and was measured too: with an indented LIST instead of a
table (`- sub_a` / `- sub_b`), Rust loses the `note` FIELD entirely (`note: None`) where Python and
TypeScript keep it. Same cause — the field is committed too late.

Worth noting that `nested_subitems` does not fire on either shape, because the blank line makes the
indented content a separate block rather than a sub-item.

The reason this is a decision and not a fix: the format has no meaning for a block attached to an
INLINE field. Fields are `- key: value` scalars; there is nowhere for such a block to belong. That is
exactly the shape of the three errors added this session.

## The suite is deliberately red [[qmd70_finding_red_suite: Finding]]

Five failing tests were added ON PURPOSE, at the operator's instruction, to pin defects this task
did not fix. `make test` is therefore RED and QMD-70 cannot reach `done` until this is resolved.

- category: process
- affected_files: [tests/parser/220-prose-between-array-heading-and-table.qmd.md, tests/parser/221-empty-blockquote-comment.qmd.md, tests/parser/222-rebuild-content-after-array-table.qmd.md, tests/cli/018-parse-yaml-field-then-prose, tests/cli/019-parse-indented-block-under-list-field]
- solution: Operator decision — fix the five defects, quarantine the cases behind an expected-failure marker, or accept a red baseline. See the detail below for the cost of each.

### What is red and why [[qmd70_finding_red_suite_detail: text]]

- about: [[#qmd70_finding_red_suite]]

Each case was verified to fail for its OWN stated reason, not incidentally. Python is the reference
implementation, so every expected file holds Python's output; TypeScript agrees with it everywhere
here.

Two of the five were fixed rather than left red, once the discussion settled which side was right.

| case | red in | the defect it pins |
| --- | --- | --- |
| `cli/020-parse-prose-between-array-heading-and-table` | — FIXED | py and ts swallowed the table into the container's comment and left the array empty; see [[#qmd70_finding_prose_gap]] |
| `parser/221`, `223`, `224` (blockquotes) | — FIXED | Rust rebuilt blockquote comments instead of slicing them; see [[#qmd70_finding_blockquote]] |
| `parser/222-extra-table-under-array-heading` | — FIXED | a second table under one array heading is now an error, which removes the unplaceable content entirely; see [[#qmd70_finding_extra_table]] |
| `cli/018-parse-yaml-field-then-prose` | — FIXED | a paragraph after a `yaml` field heading OVERWROTE the field's value, losing the YAML data; see [[#qmd70_finding_rs_other_paths]] |
| `cli/019-parse-indented-block-under-list-field` | rs | an indented block under an inline field: all three are wrong, differently — see [[#qmd70_finding_indented_block]] |

**Case 020 replaced an earlier fixture that pinned the wrong side.** The first attempt asserted
Python's reading because Python is the reference implementation; checking the precedent showed Rust
is right and the other two deviate. It also lived in the parser corpus, where it went red in Rust
for an unrelated second reason — the rebuild motion already pinned by case 222 — so it moved to the
CLI corpus, which is parse-only and leaves exactly one reason for the failure. Both defects it
exposed are now fixed, so it is green.

**One case remains red.** `019` needs an operator decision rather than a fix, because all three
implementations are wrong in different directions — see [[#qmd70_finding_indented_block]]. Every other
deliberately failing case was closed by fixing the defect it pinned.

**Case 222 is red everywhere, so it is not a parity defect.** All three parse identically and all
three rebuild the same wrong layout. It pins a shared limitation, which is why no implementation can
make it pass without the format gaining a way to express "content after an array's own table".

There is no expected-failure marker in any of the three corpora — the parser, CLI, SQL, workspace,
LSP and MCP runners all treat a mismatch as a failure. Adding one means editing three runners in
three languages, which is a change to test infrastructure and deserves its own review rather than
being smuggled in here.

## Open questions for the operator [[qmd70_finding_questions: Finding]]

One question, and it is about scope of the fix rather than about representation — triage
closed the representation question by finding the precedent already in the corpus.

- category: parser
- related_to: [[#qmd70_table_scope]]
- solution: Answer question 1; goals A1, A2, B1, C1 and D1 are ready to implement as written, and B2 is independent of it.

### The question [[qmd70_finding_questions_detail: text]]

- about: [[#qmd70_finding_questions]]

1. **Should a table under an array heading still convert once an element has appeared?** A
   mixed array — some elements written as headings, then a table adding further rows — is
   expressible today only as a side effect of this bug. If it should stay legal, the rule is
   "a table converts while positioned in the container, whether before or after the element
   headings"; if not, a table appearing after the first element heading is an error and needs
   a diagnostic rather than silence. The corpus contains no example either way, so neither
   reading breaks a fixture.

   **Fully answered now.** With the array context scoped to the container, a table AFTER an element
   heading is inside that element and becomes its comment content — valid, and the main case this
   task fixed. The reverse order — a table first, then element headings — was measured and turned out
   worse than described here: the element does NOT join the array, it becomes a plain field on the
   parent with its Kind degraded from the declared one to `__Object`, silently. The operator decided
   that is an error; see [[#qmd70_finding_mixed_array]].

2. **Does prose between an array heading and its table break the connection?** Rust says no and
   still converts the table; Python and TypeScript say yes and treat both as comment content.
   See [[#qmd70_finding_prose_gap]] for the measurement. A decision is needed because the three
   implementations must agree — Rust's reading is more forgiving, the other two are more
   literal about the documented syntax.

Closed during triage, recorded here because the task originally listed it as open: **how a
table inside an element is represented** is already settled by the corpus. `__comments`,
verbatim, anchored like any other undeclared content — see the "where the expected JSON comes
from" section of [[#qmd70_finding_tests]]. There was never a gap, only an array context that
stole the table before the existing comment path could run.

Not a question but worth recording: this bug is why the QMD-69 task file carries its
comparison tables as prose rather than as tables. Any tracking document that puts a table
inside a Goal is currently corrupting itself, and in Rust it loses the Goal.
