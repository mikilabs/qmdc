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
- solution: Its own ticket, and it is a REFACTOR rather than a fix — see the structural assessment below. Pinned as deliberately failing tests: `tests/cli/018-parse-yaml-field-then-prose` and `tests/cli/019-parse-indented-block-under-list-field`, both red in Rust only.

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

### Why the array fix does not simply generalize [[qmd70_finding_rs_other_paths_why: text]]

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
- solution: Its own ticket — a new error type is a cross-surface addition with a documentation cascade.

### The precedent [[qmd70_finding_no_diagnostic_detail: text]]

- about: [[#qmd70_finding_no_diagnostic]]

A second table under one array heading is now preserved as comment content and nothing is reported
on any surface: `workspace validate` returns `[]` in all three parsers, and the LSP and MCP are
silent. The row the author almost certainly meant as an array element is quietly reclassified as
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
- solution: Its own ticket. Needs a representation for content that follows a heading-declared array, which is a format decision, not a bug fix. Pinned as a deliberately failing test: `tests/parser/222-rebuild-content-after-array-table`, red in ALL THREE implementations because the defect is shared.

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
| `parser/222-rebuild-content-after-array-table` | **all three** | `parse` agrees, but `rebuild` moves the trailing table above the array heading — the content-motion check catches it |
| `cli/018-parse-yaml-field-then-prose` | rs | a paragraph after a `yaml` field heading OVERWRITES the field's value, so the YAML data is lost too |
| `cli/019-parse-indented-block-under-list-field` | rs | rs anchors an indented block on the wrong field and keeps its raw source |

**Case 020 replaced an earlier fixture that pinned the wrong side.** The first attempt asserted
Python's reading because Python is the reference implementation; checking the precedent showed Rust
is right and the other two deviate. It also lived in the parser corpus, where it went red in Rust
for an unrelated second reason — the rebuild motion already pinned by case 222 — so it moved to the
CLI corpus, which is parse-only and leaves exactly one reason for the failure. Both defects it
exposed are now fixed, so it is green.

**Three cases remain red.** `018` and `019` need the shared field accessor described in
[[#qmd70_finding_rs_other_paths]]; `222` needs the format to gain a way to express content after an
array's own table, per [[#qmd70_finding_rebuild_anchor]]. Neither is a small fix, and both were left
deliberately.

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

   **Answered by the implementation, for the case that matters.** With the array context scoped
   to the container, a table after an element heading is inside that element and becomes its
   comment content — so a mixed array is no longer expressible even by accident, and no
   diagnostic is needed for it. What remains genuinely open is only the reverse order: a table
   BEFORE any element heading, followed by element headings. That still converts, and the
   elements are appended after the table's rows. Nothing pins it, and nothing in the corpus
   writes it.

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
