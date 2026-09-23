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

Rust composes `{parent_full_id}.{arr_field}.{local_id}` for table children
(`qmdc-rs/src/parser.rs:145`), which doubles the segment here, while its own heading
elements and both other implementations produce `items.items_0`. When the field name
differs from the parent id all three agree (`team.members.members_0`), which is why this
never surfaced.

## Mandatory regression tests (data-driven, failing as designed) [[qmd70_finding_tests: Finding]]

Four parser microtests were added during triage per the Bug triage exception, one per shape
of the characterization matrix. They are plain fixtures plus expected JSON — no new test
code — so each runs in all three implementations and through all three microtest aspects
(parse, rebuild, rebuild-text). The documented feature needs no new guard: the shipped
`tests/parser/032-table` already pins a table directly under an array heading, and
`tests/parser/065-text-table-in-array` pins a table inside an element wrapped in a `text`
field. Both stay green.

- category: testing
- related_to: [[#qmd70_table_scope]]
- affected_files: [tests/parser/212-table-in-array-element-after-prose.qmd.md, tests/parser/213-table-in-array-element-first.qmd.md, tests/parser/214-table-in-array-element-after-field.qmd.md, tests/parser/215-table-in-array-element-between.qmd.md]
- solution: Keep all four as the acceptance gate; they turn green only when the array context is scoped to the array's own content, and they are what forces the three implementations to agree.

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

Closed during triage, recorded here because the task originally listed it as open: **how a
table inside an element is represented** is already settled by the corpus. `__comments`,
verbatim, anchored like any other undeclared content — see the "where the expected JSON comes
from" section of [[#qmd70_finding_tests]]. There was never a gap, only an array context that
stole the table before the existing comment path could run.

Not a question but worth recording: this bug is why the QMD-69 task file carries its
comparison tables as prose rather than as tables. Any tracking document that puts a table
inside a Goal is currently corrupting itself, and in Rust it loses the Goal.
