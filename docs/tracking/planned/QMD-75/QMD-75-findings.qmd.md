# QMD-75: Findings

## A body that is not a field changes which objects exist [[qmd75_finding_body: Finding]]

Ten lines reproduce it. An object heading, a field, a deeper heading whose body is a prose
paragraph, then a deeper heading still:

```markdown example
# S [[s: Session]]

- status: done

## Closure [[clo]]

(filled later)

### Task [[t5: SessionTask]]

- status: x
```

| parser | objects produced |
|---|---|
| rs | `s`, `s.clo`, `s.clo.t5` |
| py | `s` — drops `clo` and its whole subtree |
| ts | `s`, `s.t5` — drops `clo` and re-parents the child onto the grandparent |

A `---` thematic break in place of the paragraph triggers it identically. With an empty body, or a
body that is a field (`- note: y`), all three agree — which is why the repository's own docs never
caught it: they always use fields.

Live instance on `tg-acp`:
`adocs/sessions/telegram-bot_2026-04-12b/telegram-bot_2026-04-12b.qmd.md:86` —
`## Closure [[telegram_bot_2026_04_12b_closure]]` with the body `(filled during Step 3)`. Rust keeps
`...closure` and `...closure.tbt1_t5`, Python loses both, TypeScript keeps `tbt1_t5` attached to the
session instead of the closure. Corpus-wide that is 1136 ids over 1136 distinct in Rust, 1136 over
1134 in Python, 1137 over 1135 in TypeScript, with no `duplicate_id` reported by either of the two
that collide.

- category: parser
- related_to: [[#qmd75_body_identity]]
- solution: Answer Q1 in the spec, then make the three parsers agree on it, and add the shape to the parity corpus so a future divergence fails the suite.

## TypeScript promotes a prose bullet to a field [[qmd75_finding_prose_bullet: Finding]]

`adocs/research/legal-docs-analysis.qmd.md` under the id-less heading
`#### AI-Specific Concerns:` at line 118:

```markdown example
- Explicit: "We may use your Inputs and Outputs to train ..."
- Even with opt-out, data used for: (i) safety review/flagged conversations, ...
```

Rust and Python read both bullets as prose and report no fields on the enclosing object.
TypeScript reads the first as the field `Explicit` and then reports `mixed_field_keys` at line 120 —
the single `mixed_field_keys` in the whole corpus. Bisected to exactly these two lines: a five-line
file reproduces it, and putting the id-less `#### AI-Specific Concerns:` heading back suppresses it in
TypeScript too.

With the two bullets placed directly on an object and no id-less heading above them, all three agree
and all three report `mixed_field_keys` — so the rule itself is shared and only the field-or-prose
decision about the body differs. Same root cause as `[[#qmd75_finding_body]]`.

- category: parser
- related_to: [[#qmd75_body_identity]]
- solution: Falls out of Q1: once a non-field body has one definition, the id-less heading stops changing how the bullets below it are read.

## nested_subitems fires on mirror-image shapes [[qmd75_finding_subitems: Finding]]

The shape is a field with indented sub-items — `- issues:` followed by an indented `- one`. Where it
sits decides who reports it:

| position of the field | rs | py | ts |
|---|---|---|---|
| on the file's top-level object | reports | silent | silent |
| on a plain subobject (`#### X [[x: K]]`) | reports | reports | reports |
| on an element of an object array (`### S [[s: [K]]]` then `#### E [[e]]`) | silent | reports | reports |

`tg-acp`'s 148 hits are all array elements (`sim/runs/*/trace.qmd.md`, under
`### Steps [[r5_s01_steps: [StepTrace]]]`), which is why Rust sees none of them and why the totals
come out 1 / 149 / 150. A colon inside the sub-item and a blank line after the heading are both
irrelevant — only the position matters.

Reproduction of the array-element case, Rust silent and the other two reporting:

```markdown example
# T [[t: SimTrace]]

- status: PASSED

### Steps [[steps: [StepTrace]]]

#### Step 1 [[s1]]
- status: ok
- issues:
  - AMBIGUOUS (medium): text
```

The top-level case is the same `- issues:` block placed directly under `# Thing [[thing: Thing]]`,
where only Rust reports.

- category: parser
- related_to: [[#qmd75_body_identity]]
- solution: Decide whether sub-items under a field are legal, then apply the check at one place that every position reaches, rather than at three call sites that each miss a different one.

## Rust omits the index key from parse output [[qmd75_finding_shape: Finding]]

`workspace parse` top-level keys: Rust returns `errors, files, objects, root, workspaces`; Python and
TypeScript also return `index`. A consumer written against either of the other two reads a missing key
from Rust.

- category: parser
- related_to: [[#qmd75_body_identity]]
- solution: Decide whether index belongs in the contract; either emit it in Rust or drop it in the other two, and pin the key set in the CLI conformance fixtures.

## Open questions [[qmd75_finding_questions: Finding]]

**Q1 — what is a heading's body when it is not a field list?** The format defines a field list and it
defines nesting by heading depth, but not what happens to a heading whose body is prose or a `---`.
The three implementations each answered differently and none of them is written down. The candidate
answers, in the order that keeps the most existing documents working:

1. Prose is ignored and the object exists like any other — Rust's behaviour. A document may then mix
   description and structure freely, which is how `tg-acp` and this repository's own tracking files
   are written.
2. A non-field body ends the object — Python's behaviour. Then a prose line silently deletes a
   subtree, and the only way to keep an object is to give it a field.
3. A non-field body ends the object but its children survive at the parent's level — TypeScript's
   behaviour. This is the one answer that changes an object's parent, so identity depends on prose.

Answer 1 is the only one under which a document's objects do not depend on prose, and it is the
behaviour a reader expects from Markdown. The decision belongs to the operator because it is a format
statement, not an implementation detail.

**Q2 — does `index` belong to the parse contract?** See `[[#qmd75_finding_shape]]`; it is a
one-line answer either way, but it must be answered once rather than per parser.

- category: parser
- related_to: [[#qmd75_body_identity]]
- solution: Operator answers Q1 and Q2; the rest of the task is mechanical once they are settled.

## Why the suite never caught any of this [[qmd75_finding_tests: Finding]]

Parity across the three parsers is measured over `docs/` — 119 documents in which every object's body
is a field list, no object array carries a field with sub-items, and no prose bullet sits under an
id-less heading. So the corpus cannot express any of the five divergences above, and each of them
survived every release while the suite stayed green: 4072 cases, 0 failures, divergent 0.

`tg-acp` cost nothing to obtain and exposed five divergences in one run, three of which change object
identity. A corpus of real documents written by someone who was not thinking about the parser is
worth more than the same volume of fixtures written by someone who was.

- category: testing
- related_to: [[#qmd75_body_identity]]
- solution: Add the four shapes as workspace fixtures so parity covers them, and keep one external corpus in the loop as a periodic check rather than a one-off.
