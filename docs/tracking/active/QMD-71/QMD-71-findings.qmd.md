# QMD-71: Findings

## Rust anchors a comment on the preceding object, not the text field [[qmd71_finding_anchor: Finding]]

The largest group — 20 of the 32 documents, and a further 3 where it also suppresses the field's
`__syntax` and `__types`. Rust is the only outlier; Python and TypeScript agree.

- category: parser
- priority: high
- affected_files: [qmdc-rs/src/parser.rs]
- affected_functions: [parse, "Event::End(TagEnd::Heading)", "Event::End(TagEnd::Paragraph)"]
- solution: Move the comment anchor to the text FIELD when a heading declares one, so content following it anchors there rather than on the last object seen. Python does this at its heading handler; the anchor is already moved for several other field types in Rust, so the question is which path misses it.
- test_plan: [[#qmd71_finding_tests]]

### The shape [[qmd71_finding_anchor_detail: text]]

- about: [[#qmd71_finding_anchor]]

```markdown example
# D [[d: HowTo]]

- goal: g

## Gen [[gen: ContentGenerator]]

- target: [[#d.content]]

## Content [[content: text]]

Intro line.

## A plain heading

Body under it.
```

Rust anchors the comment on `gen` — the preceding OBJECT — where the other two anchor it on
`content`, the text FIELD the content actually follows.

The `docs/` corpus hits this constantly because the generated-content pattern is used throughout the
guides: a `ContentGenerator` object, then a `content` text field, then prose under plain headings.

Three of the 32 show a stronger form of the same bug, in which Rust also fails to record the field's
`__syntax: multiline_text` and `__types: string` — `docs/guides/readme.qmd.md` is the example. The
field's VALUE is still correct there, so it is the metadata and the anchor that are lost, not the
content.

## TypeScript loses content inside a text field [[qmd71_finding_textfield: Finding]]

Five documents. The only cause in this task that destroys what an author wrote, which is why it is
recommended first despite being a quarter the size of the anchor group.

- category: parser
- priority: high
- affected_files: [qmdc-ts/src/parser.ts]
- affected_functions: [parse, parseFieldsFromList]
- solution: The ordered-list handling inside a text field consumes the fence that follows each item. Capture the field's content as a raw source slice, which is what Rust and Python already do here.
- test_plan: [[#qmd71_finding_tests]]

### The shape [[qmd71_finding_textfield_detail: text]]

- about: [[#qmd71_finding_textfield]]

An ORDERED LIST followed by a fence, inside a `text` field:

```markdown example
# D [[d: G]]

## Solution [[solution: text]]

1. First:

(a fenced json block here)

1. Second:

(a second fenced json block here)
```

Rust and Python keep the whole value verbatim. TypeScript yields `"1. First:\n\nSecond:"` — both
fences gone, and the second item's ordered-list marker gone with them.

A bare fence inside a text field is handled identically by all three; it takes the ordered list to
trigger it. That is why an earlier probe with just a fence did not reproduce it, and it is worth
remembering when writing the regression: the list is load-bearing.

Visible today in `docs/format/validation-errors.qmd.md`, where the `err_multiple_definitions`
section's `solution` field loses both of its examples and the remaining prose stops making sense —
the project is shipping a documentation page whose TypeScript-parsed content is wrong.

## TypeScript drops a trailing zero from a float [[qmd71_finding_float: Finding]]

Three documents, and the cheapest of the five.

- category: parser
- priority: medium
- affected_files: [qmdc-ts/src/parsers/field.ts]
- affected_functions: [parseFieldValue]
- solution: Decide whether the fix belongs in the value parse or in JSON serialisation, then apply it — see the open question.
- test_plan: [[#qmd71_finding_tests]]

### The shape [[qmd71_finding_float_detail: text]]

- about: [[#qmd71_finding_float]]

| input | rs | py | ts |
| --- | --- | --- | --- |
| `- version: 2.0` | `2.0` | `2.0` | `2` |
| `- z: 0.0` | `0.0` | `0.0` | `0` |
| `- x: 1.50` | `1.5` | `1.5` | `1.5` |
| `- y: 3.10` | `3.1` | `3.1` | `3.1` |

Only a float whose fractional part is zero differs, so this is JavaScript number formatting rather
than parsing: `JSON.stringify(2.0)` is `"2"`. `__types` says `number` in all three, so the type is
agreed and only the rendered value differs.

## Rust extracts fields from a nested list item [[qmd71_finding_nested: Finding]]

One document, `docs/tracking/workflow.sop.qmd.md`, which differs on ten keys across three objects —
all of them this one cause. Pre-existing, and already known: the QMD-70 code review found it while
probing, and QMD-70's `block_in_inline_field` guard was deliberately restricted to top-level list
items so it would not add a second axis of divergence on top of it.

- category: parser
- priority: medium
- affected_files: [qmdc-rs/src/parser.rs, qmdc-py/qmdc/parsers/field.py, qmdc-ts/src/parsers/field.ts]
- affected_functions: ["Event::End(TagEnd::Item)", parse_fields_from_list, parseFieldsFromList]
- solution: Needs a decision first — see the open question. Whichever reading wins, all three must adopt it.
- test_plan: [[#qmd71_finding_tests]]

### The shape [[qmd71_finding_nested_detail: text]]

- about: [[#qmd71_finding_nested]]

An indented list under an ordered-list item, where the indented items look like fields:

```markdown example
## Step [[step: S]]

1. Create Finding objects with:
   - affected_files: concrete file paths
   - affected_functions: function names
   - solution: implementation approach
```

Rust extracts `affected_files`, `affected_functions` and `solution` as FIELDS of `step`. Python and
TypeScript treat the whole construct as comment content and extract nothing.

This is prose in the source — the SOP describing what a Finding should contain — so Rust is inventing
fields from documentation text. That argues Python and TypeScript are right, but it is a decision
about reach rather than an obvious bug, which is why it carries an open question.

## Python renumbers a nested bullet list as ordered items [[qmd71_finding_renumber: Finding]]

Found while writing the regression for [[#qmd71_finding_nested]], not during classification — the
two causes share one document, so the aggregate diff hid this one behind the other.

- category: parser
- priority: medium
- affected_files: [qmdc-py/qmdc/parser.py]
- affected_functions: [parse, append_comment]
- solution: Capture the comment as a raw source slice, as Rust and TypeScript do here, instead of reconstructing the list from its items.
- test_plan: [[#qmd71_finding_tests]]

### The shape [[qmd71_finding_renumber_detail: text]]

- about: [[#qmd71_finding_renumber]]

An ordered item with a nested bullet list, captured as comment content:

```markdown example
## S [[s: S]]

1. First:
   - a
   - b
```

| impl | stored content |
| --- | --- |
| rs, ts | `1. First:` then the two indented bullets, verbatim |
| py | `1. First:` / `2. a` / `3. b` |

Python flattens the nested bullets into the outer ordered list and RENUMBERS them, so the indentation
and the bullet markers are lost and two items that were never numbered acquire numbers. The same
reconstruction-instead-of-slicing pattern that QMD-70 fixed for blockquotes in Rust.

This is why `tests/cli/022` fails in TypeScript as well as Rust: the two parsers disagree about the
FIELDS (cause 4) and Python disagrees about the COMMENT (this one), so no two of the three match on
that input.

## Rust splits a comment block at a sub-heading [[qmd71_finding_split: Finding]]

The SEVENTH cause, found by re-measuring `docs/tracking/workflow.sop.qmd.md` after its other two were
fixed — not by classification and not by writing a regression. Rust is the outlier.

- category: parser
- priority: medium
- affected_files: [qmdc-rs/src/parser.rs]
- affected_functions: [parse, "Event::End(TagEnd::Paragraph)"]
- solution: Align Python and TypeScript with Rust, which matches the written format. Both already had a comment-heading handler that groups correctly; the paragraph run swallowed the heading and advanced the token index past it, so that handler never ran.
- test_plan: pinned by `tests/parser/238-comment-split-at-subheading`

### The shape [[qmd71_finding_split_detail: text]]

- about: [[#qmd71_finding_split]]

```markdown example
## S [[s: S]]

- f: 1

First para.

### Sub heading

Second para.
```

All three produce two comments, and both anchor them on `f`, but they cut in different places:

| impl | comment 1 | comment 2 |
| --- | --- | --- |
| rs | `First para.` | `### Sub heading` + `Second para.` |
| py, ts | `First para.` + `### Sub heading` | `Second para.` |

Nothing is lost either way, so this one looked like the mildest of the seven and the one most likely to
be settled by "two out of three".

It was settled by the specification instead, and it went AGAINST the majority.

`docs/format/comments.qmd.md` already says what a comment heading does: *"comment headings — they and
all content below them (until the next structural boundary) become part of `__comments`"*. Below them.
That is Rust's reading, and the same page's boundary list does not include a deeper bare heading at
all — the listed boundaries are a heading at the same or higher level, a heading with `[[field_id]]`, a
field list, and end of document.

The confirmation is that Python and TypeScript contradicted THEMSELVES. Raise the same heading by one
level, to the object's own level, and all three group it with the text below:

| heading level | rs | py | ts |
| --- | --- | --- | --- |
| same as the object | heading + text below | same | same |
| deeper (this case) | heading + text below | heading joined to text ABOVE | same as py |

One construct, two different answers inside one parser, decided by a level that changes nothing about
what the heading means. So this was a defect in two parsers, not a dialect, and the majority was wrong.

The operator's principle settled the same way from the other end: a heading becomes a field or an
object when it can, and comments are what is left over. A bare heading has no id to key on, so it can
never be a field or an object — verified in all three parsers, inside an object and at top level
alike — which leaves it a comment, and a heading introduces what follows it.

### Why it took three passes to see [[qmd71_finding_split_why: text]]

- about: [[#qmd71_finding_split]]

Worth recording as a method finding, because it is the same lesson for the third time.

`docs/tracking/workflow.sop.qmd.md` was originally classified as ONE divergent document with ten
differing keys. It turned out to hold three independent causes in three different parsers: Rust
inventing fields from a nested list ([[#qmd71_finding_nested]]), Python renumbering a nested bullet
list ([[#qmd71_finding_renumber]]), and this one. Each became visible only once the previous was
fixed, because an aggregate per-key diff reports that a file differs, not how many independent reasons
it differs for.

The practical rule: after fixing any cause, RE-MEASURE the documents it was supposed to close. A count
that does not drop as predicted means another cause is hiding behind the one just fixed. Here the count
stayed at 25 after two fixes that should have closed a document, which is exactly how this surfaced.

## Open questions [[qmd71_finding_questions: Finding]]

Three decisions are needed before the corresponding fixes, and one of them may move work out of this
task entirely.

- category: parser
- priority: high
- affected_files: []
- solution: Answer before implementing the affected cause; the other two causes can proceed regardless.
- test_plan: [[#qmd71_finding_tests]]

### The questions [[qmd71_finding_questions_detail: text]]

- about: [[#qmd71_finding_questions]]

1. **Should a nested list item's field-like entries become fields?** ([[#qmd71_finding_nested]]) Rust
   says yes, Python and TypeScript say no. The evidence favours the latter: the one real occurrence is
   the SOP's own prose describing a data shape, and Rust turns that description into actual fields.
   But it is a reach decision, and whichever way it goes all three must match.

2. **Where does the float fix belong?** ([[#qmd71_finding_float]]) Parsing `2.0` already yields the
   number 2; the difference appears at serialisation. Fixing it in the parse (keeping the raw string
   when it round-trips as a float) preserves the author's text but makes the value's type less
   obvious. Fixing it in serialisation means a custom JSON writer in TypeScript only. Neither is
   clearly right, and the choice affects whether `- z: 0.0` should read back as `0.0` or `0`.

3. **Do the three divergences recorded in the task belong here?** The YAML block-scalar one
   (chomping indicators `|-`, `|+`, `>`, `>-`, `>+` are broken three different ways) looks like a
   missing FEATURE rather than a divergence to reconcile — no implementation handles them, they just
   fail differently. That may be its own task.

## Test plan [[qmd71_finding_tests: Finding]]

Data-driven fixtures in the shared corpus, one per cause, each written before the fix and each
verified to fail for its own stated reason.

- category: testing
- priority: high
- affected_files: [tests/parser, tests/cli]
- solution: Four regressions now, plus a valid-neighbour fixture for each fix as it lands.
- test_plan: see the detail below

### Existing coverage, and why it missed all five [[qmd71_finding_tests_existing: text]]

- about: [[#qmd71_finding_tests]]

**None of the six causes has any existing test.** That is not an oversight in the corpus so much as a
structural gap, and it is worth stating because it shaped this task's existence.

Parity over `tests/parser/**` and `tests/cli/**` is enforced by construction — all three runners
compare against one shared `expected.json`, so a divergence there fails at least one of them. What had
no check at all was the REAL corpus. `make validate-compare` compared only validation-error lists until
QMD-70 added the parse-output comparison, and that is what surfaced all 32.

So every cause here lives in a shape that no fixture was ever written for, and all six were invisible
to CI until a week ago.

One of them was invisible even to the classification: the renumbering bug shares its document with the
nested-field bug, and an aggregate per-key diff shows only that the file differs, not that it differs
for two independent reasons in two different parsers. Writing the regression is what separated them —
which is the argument for writing a fixture per cause rather than per document.

### New tests [[qmd71_finding_tests_new: text]]

- about: [[#qmd71_finding_tests]]

Four regressions, added during triage. Each is the minimal reproducer from the corresponding finding,
so each is small and non-duplicative:

| fixture | pins | fails in |
| --- | --- | --- |
| `tests/parser/235-comment-anchor-after-text-field` | content following a `text` field anchors on that field | rs |
| `tests/parser/236-text-field-ordered-list-with-fence` | an ordered list plus fences inside a text field keeps everything | ts |
| `tests/parser/237-float-trailing-zero` | `2.0` and `0.0` keep their fractional part | ts |
| `tests/cli/022-nested-list-item-fields` | a nested list item's entries do NOT become fields, and the comment keeps the source verbatim | rs, ts |

The last fails in TWO parsers, for two different causes: Rust extracts the fields
([[#qmd71_finding_nested]]) and Python renumbers the comment ([[#qmd71_finding_renumber]]). It is in
the CLI corpus rather than the parser one because its expected output depends on the answer to open
question 1; putting it in a parse-only corpus keeps it out of the round-trip check,
whose own limitations are unrelated to what is asserted. The other three round-trip cleanly and belong
in the parser corpus.

**Measured at `triage_review`** — per fixture and per parser, not in aggregate:

| fixture | rs | py | ts |
| --- | --- | --- | --- |
| `parser/235-comment-anchor-after-text-field` | RED | ok | ok |
| `parser/236-text-field-ordered-list-with-fence` | ok | ok | RED |
| `parser/237-float-trailing-zero` | ok | ok | RED |
| `cli/022-nested-list-item-fields` | RED | ok | RED |

Each fails with exactly the documented difference and nothing else — the anchor value, the truncated
field content, `2` against `2.0`, and the invented fields plus the renumbered comment. Checked by
diffing the output rather than by reading the pass/fail line.

Two method notes, both learned the hard way here:

- **The aggregated JUnit report is not trustworthy for this.** `make -k test` leaves some targets
  without an XML file when they abort, so the aggregate showed three of the four. Verify per fixture
  against each parser directly.
- **A control value in a fixture can make it fail for the wrong reason.** `237` first included
  `- half: 1.50` as a control that all three agree on, and that line made the fixture fail in PYTHON
  too — on the round-trip check, because `rebuild` normalises `1.50` to `1.5`. The control was removed;
  it belongs in this document's table, not in the fixture. That normalisation is a separate, unrelated
  issue and is deliberately NOT part of this task.

### On verification method [[qmd71_finding_tests_method: text]]

- about: [[#qmd71_finding_tests]]

Do not verify these through `parse | rebuild`. The round trip has its own known limitations — a text
field's heading level shifts, a top-level array gains a wrapper heading — so a round-trip failure here
would say nothing about the divergence. Compare parse output directly, which is what
`make validate-compare` now does and what the fixtures assert.

Each fix should also land a fixture for its nearest VALID neighbour, not only the corrected shape.
QMD-70's evidence for this is direct: four new rules all needed one, and the single rule that lacked
one shipped a false positive that survived to the third review pass.
