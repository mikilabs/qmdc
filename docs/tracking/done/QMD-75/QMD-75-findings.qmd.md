# QMD-75: Findings

## A body that is not a field changes which objects exist [[qmd75_finding_body: Finding]]

The divergence needs TWO things: a heading that declares a bare `[[id]]`, and a DEEPER heading that
declares its own identifier below it. What sits between them then decides the tree differently in
each parser. Ten lines reproduce it:

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
| py | `s` — the text field swallows the declaration, dropping `clo` and its whole subtree |
| ts | `s`, `s.t5` — the text field stops early and the child is RE-PARENTED onto the grandparent |

Measured over 48 cases — three heading spellings (`[[clo]]`, `[[clo: text]]`, `[[clo: Closure]]`) x
eight bodies (empty, field list, list without fields, paragraph, `---`, table, fence, blockquote) x
with and without a declared child — the three parsers agreed on 38 and disagreed on 10.

The correction to the first reading of this finding, which mattered: WITHOUT a declared child below,
all three already agreed, and they agreed on the IMPLICIT TEXT FIELD — `### Description [[description]]`
followed by prose is a text field, pinned by fixtures `021-multiline-text`, `030-multiline-with-blank`
and `107-multiline-text-field` plus eight LSP and two workspace fixtures. So "a bare `[[id]]` is
always an object" is NOT the rule and cannot be: removing the implicit text field from Rust broke 15
pinned cases. The rule that holds is `[[#qmd75_finding_kind_decides]]`.

- category: parser
- related_to: [[#qmd75_body_identity]]
- solution: Decide a bare `[[id]]` by STRUCTURE, never by prose — a declared child makes it an object, otherwise it is the implicit text field. Implemented in all three.

## The declared kind decides; the body never does [[qmd75_finding_kind_decides: Finding]]

Measured on the same document with only the heading spelling varied: `[[clo: Closure]]`, a heading
that declares a kind, already behaves the same way in all three parsers for every one of the eight
bodies — it is an object, and non-field content goes to `s.clo.__comments` with `after: "__self"`.
The target behaviour therefore did not have to be invented; it already existed, three times over,
for the spelling that names a kind.

Stated as one rule: the declared kind decides what a heading is, and the content below it never
changes that.

| spelling | what it is | body that is not a field list |
|---|---|---|
| `[[clo: Kind]]` | object | its `__comments` |
| `[[clo]]` plus a declared child below | object | its `__comments` |
| `[[clo]]` with no declared child | implicit text field | its value |
| `[[clo: text]]` | text field | its value, deeper declarations included |

Two consequences. Prose can no longer change a document's object set: with or without the paragraph,
`## Closure [[clo]]` followed by `### Task [[t5: SessionTask]]` is one object holding another. And
the spelled-out `[[clo: text]]` form was diverging too — TypeScript lifted a nested declaration out
of a declared text field, so the author's explicit choice was not honoured either.

- category: parser
- related_to: [[#qmd75_body_identity]]
- solution: Written into `docs/format/headings.qmd.md` Rules, then applied to all three parsers.

## Content survived only when the author spelled a kind [[qmd75_finding_content_gate: Finding]]

Three parsers, two places each, one gate: whether a `---`, a table or a code fence at an object's own
anchor becomes `__comments` was decided by `__kind !== "__Object"` — that is, by whether the author
had written a kind. Under a bare `[[id]]` object the content was DROPPED; under `[[id: Kind]]` the
identical content was kept.

Measured before the fix, `## Closure [[clo]]` with a declared child below:

| body | rs | py | ts |
|---|---|---|---|
| fence | object, content lost | text field | text field |
| table | table-field label on a string value | text field | text field |
| `---` | content kept | text field | text field, content lost |

Rust also carried a lookahead that turned a bare `[[id]]` plus a table into a table FIELD. It never
filled any rows: it produced a string value labelled `__syntax: table` beside `__types: string`, a
combination nothing consumes, where Python and TypeScript both reported `multiline_text` for the
identical value. A bare `[[id]]` declares no array, so the body cannot turn it into one.

- category: parser
- related_to: [[#qmd75_body_identity]]
- solution: Gate on DECLARED (a kind or an explicit id), not on the kind alone, in all six places; drop the Rust table lookahead so the value and its syntax label agree.

## TypeScript's text-field start line was always null [[qmd75_finding_ts_startline: Finding]]

TypeScript recorded a text field's content start from the `heading_close` token
(`tokens[i + 2].map[1]`). markdown-it gives `heading_close` a null map — measured directly: for
headings on lines 1, 5 and 10 the `heading_open` maps are `[0,1]`, `[4,5]`, `[9,10]` and every
`heading_close` map is `null`. So the value was ALWAYS null, which silently disabled the raw-slice
path that closes a text field and left the value to whatever the paragraph handler had appended on
the way. A field whose body was only a `---` therefore came out as the empty string.

Fixing the start line exposed the second half: three separate sites wrote the field by APPENDING a
fragment, so once the slice path came alive the value was duplicated (`(filled later)\n\n(filled
later)`). A text field's value is ONE verbatim slice from just after its heading to its end boundary,
which is what Rust and Python do; every termination path now assigns that slice. Before this,
TypeScript also normalised the author's blank lines — two blank lines before a swallowed heading came
back as one, so the round trip was not lossless.

- category: parser
- related_to: [[#qmd75_body_identity]]
- solution: Take the start line from `heading_open`, and make every write site assign the whole slice instead of appending fragments.

## A `---` above a declared heading cost TypeScript two nesting levels [[qmd75_finding_ts_hr_levels: Finding]]

TypeScript's comment scan for a `---`, table or blockquote ended at a deeper heading only when that
heading declared an id AND carried fields directly after it. A `[[id]]` heading whose own body is a
deeper `[[id]]` heading failed that test and was swallowed into the comment — together with the next
level down. Nine lines reproduce it:

```markdown example
# S [[s: SimSummary]]

- unique_issues: 18

---

## Issues by Severity [[by_sev]]

### CRITICAL [[crit]]

#### C1 [[c1]]
- severity: CRITICAL
```

Rust and Python: `s`, `s.by_sev`, `s.by_sev.crit`, `s.by_sev.crit.c1`. TypeScript: `s`, `s.c1` — two
objects gone and the grandchild re-parented onto the object that owned the `---`. Remove the `---` and
TypeScript agrees, so the prose decided the identity here too. Pre-existing: measured identically on
the pre-change parser. Live instance on `tg-acp`, `sim/runs/2026-05-15T16-40-36/summary.qmd.md`.

- category: parser
- related_to: [[#qmd75_body_identity]]
- solution: A heading that declares an identifier ends the comment, whatever follows it.

## Python reported mixed_field_keys about its own prose [[qmd75_finding_py_mixed: Finding]]

`mixed_field_keys` means a list MIXES valid fields with invalid keys. Python also emitted it from the
branch where there are NO valid fields at all — the branch whose whole job is to store the list as
prose. Two lines reproduce it, and the objects are byte-identical in all three parsers:

```markdown example
# Legal Docs Analysis [[legal_docs_research: NarrativeDoc]]
   - Transfer mechanisms: SCCs, adequacy decisions
```

Rust and TypeScript report nothing; Python reports `mixed_field_keys` at line 2 about content it had
itself read as a comment. Found by shrinking a 938-line real document down to these two lines.

- category: parser
- related_to: [[#qmd75_body_identity]]
- solution: Emit the diagnostic only where fields and invalid keys actually coexist.

## nested_subitems fired on mirror-image shapes [[qmd75_finding_subitems: Finding]]

The shape is a field with indented sub-items — `- issues:` followed by an indented item. Where it sat
decided who reported it:

| position of the field | rs | py | ts |
|---|---|---|---|
| on the file's top-level object | reports | reports | reports |
| on a plain subobject (`#### X [[x: K]]`) | reports | reports | reports |
| on an element of an object array | silent | reports | reports |

Cause: `pending_object_array` stays set for the whole array SUBTREE, and Rust gated the sub-item
accumulation on it, so the branch that feeds `nested_subitems` was switched off inside every array
ELEMENT as well. Inside an element the ordinary field rules apply, exactly as on a plain subobject;
only the array's OWN content is special. This is the same over-broad scoping QMD-70 had to narrow for
the element-versus-table reading.

Effect on `tg-acp`: Rust went from 1 error to 126 on the same tree, gaining 125 `nested_subitems` and
losing nothing.

- category: parser
- related_to: [[#qmd75_body_identity]]
- solution: Scope the array guard to the array's own content by testing `is_array_element`.

## The parse contract differed in three ways [[qmd75_finding_shape: Finding]]

`workspace parse` did not return one shape:

1. Rust omitted the `index` key that Python and TypeScript both emitted, so a consumer written
   against either of them read a missing key.
2. Where both emitted it, the block itself disagreed: TypeScript published its INTERNAL camelCase
   names (`byGlobalId`, `byKind`, `byFile`) while Python emitted snake_case, which is what every
   other key in the output uses.
3. The error objects had three different key sets: Rust `field_name`, Python `field` plus explicit
   `reference: null` and `candidates: null`, TypeScript `field` and neither. `workspace validate`
   already agreed across all three, so only `parse` was inconsistent.

Python also dropped `__positions` from the FIRST of two objects colliding on one id: the duplicate is
moved out of the object map so the second can parse in its place, and the post-parse position pass
never visited it. Rust and TypeScript both keep them.

- category: parser
- related_to: [[#qmd75_body_identity]]
- solution: Emit `index` in Rust, snake_case on the wire in TypeScript, one error-object key set (absent values omitted), and fill positions for displaced duplicates in Python.

## Resolved questions [[qmd75_finding_questions: Finding]]

**Q1 — what is a heading's body when it is not a field list? ANSWERED by the operator: the declared
kind decides, the body never does.** Written into `docs/format/headings.qmd.md`; see
`[[#qmd75_finding_kind_decides]]` for the four-row table it reduces to. The answer is the one under
which a document's objects do not depend on its prose, and it keeps every pinned fixture, which the
stronger reading ("a bare `[[id]]` is always an object") did not — that one broke 15.

**Q2 — does `index` belong to the parse contract? ANSWERED: yes.** Two of three parsers already
emitted it and nothing external consumes the difference, so adding it to Rust is the direction that
breaks no consumer. Key names are snake_case on the wire, matching every other key.

- category: parser
- related_to: [[#qmd75_body_identity]]
- solution: Both answers are implemented and pinned by fixtures.

## What the suite could not express [[qmd75_finding_tests: Finding]]

Parity is measured over `docs/` — 125 documents in which every object's body is a field list, no
object array carries a field with sub-items, and no prose bullet sits under an id-less heading. The
corpus could not express any of these divergences, and each survived every release while the suite
stayed green.

Eight fixtures now pin the shapes, `254`-`261`. Each was verified to FAIL against the pre-change
parsers, which is what makes them regressions rather than decoration:

| fixture | shape | red before the fix in |
|---|---|---|
| 254 | bare id, prose body, declared child | py, ts |
| 255 | bare id, `---` body, declared child | py, ts |
| 256 | bare id, table body, declared child | rs, py, ts |
| 257 | bare id, fence body, declared child | rs, py, ts |
| 258 | `[[id: text]]` swallows a declared child, blank lines verbatim | ts |
| 259 | bare id, table body, NO child — implicit text field | rs |
| 260 | nested_subitems on an array element | rs |
| 261 | indented orphan bullet is prose, no diagnostic | py |

`tg-acp` cost nothing to obtain and exposed five divergences in one run. A corpus of real documents
written by someone who was not thinking about the parser is worth more than the same volume of
fixtures written by someone who was.

- category: testing
- related_to: [[#qmd75_body_identity]]
- solution: Fixtures 254-261 added and proven red before the fix; keep one external corpus in the loop as a periodic check.

## Residuals, measured and left open [[qmd75_finding_residuals: Finding]]

On `tg-acp` at `36f37c5` — 250 documents, 240 scanned — Python and TypeScript now agree exactly:
1138 objects over 1136 distinct ids each. Per-file object-id sets agree on 239 of 240 files. Three
divergences remain, all measured against the PRE-CHANGE parsers and unchanged by this task:

1. **A field-like nested sub-item.** `- issues:` followed by an indented `- AMBIGUOUS: text` — a
   sub-item whose
   own key is valid. Rust turns the first into a field `AMBIGUOUS`, treats the element's whole list as
   comment content and loses `status: ok`; Python and TypeScript report `nested_subitems`, drop
   `issues` and keep the rest. This is the case QMD-71 deliberately left open (see the comment at the
   Rust list-item field branch): suppressing the field for bullet-nested items traded one divergence
   for another. Accounts for the whole remaining error gap — 24 reports Rust misses and 1 it adds —
   and every one of the 10 files involved contains the shape.
2. **Document and TextBlock objects.** In `.kiro/steering/qmd-guide.qmd.md` Rust emits 10 objects
   where the other two emit 12, missing the `__Document` container and one `__TextBlock`.
3. **A mixed_field_keys line number.** On a list holding one valid field and one invalid key, Rust
   points at the invalid item and Python and TypeScript point at the item above it, which has no
   colon at all. Rust looks right; four lines reproduce it.

- category: parser
- related_to: [[#qmd75_body_identity]]
- solution: Each needs its own decision and its own task; none is caused by this one.

## The first rule was incomplete, and one step outside the matrix proved it [[qmd75_finding_review: Finding]]

The blind review (`reviews/06-cr-qmd75-heading-body.md`) returned 0 blocker, 2 high, 3 medium, 5 low,
3 info, and its highest-value question was the right one: the 48-case matrix only proves the shapes it
generates. Three one-step neighbours still let a body decide identity, and one of them was a parity
break this task INTRODUCED. Each was reproduced here and then measured against the pre-change
binaries — a real HEAD Rust build, not a reading of the diff.

**The rule as first written was too narrow.** It said a bare `[[id]]` is an object "when a deeper
heading declares its own `[[...]]` below it, and an implicit text field when none does" — and omitted
that a field list of its own also makes it an object, which all three parsers have always done and
the matrix itself shows (`bare/list_fields/nochild`). The complete rule is one sentence: an object
when, before the next heading at its own level or shallower, there is either a field list of its own
or a deeper heading that DECLARES an identifier; otherwise an implicit text field whose value runs to
that same boundary.

Under the complete rule the three neighbours were all wrong, and in every case Rust's reading was the
correct one:

1. **An undeclared deeper heading.** `## Closure [[clo]]` followed by `### Note` and prose: Rust a
   text field, Python and TypeScript an object. One paragraph inserted between them and all three gave
   the text field — prose deciding identity, the exact class this task is about. Pre-existing, and the
   first round KEPT it on purpose as one of "three readings". Fixed by deleting that reading.
2. **A body in front of a field list.** `## Closure [[clo]]`, a body, then `- note: y`: Rust an object
   with the fields, Python and TypeScript a text field that swallowed the object's own fields. Their
   test required the list to be the very NEXT token; Rust walks past non-field blocks. Measured on
   four bodies — prose, `---`, table, fence. Prose, `---` and fence diverge at HEAD too; the **table
   variant was introduced by this task**: at HEAD all three agreed on "no object", and removing the
   Rust table lookahead let Rust alone reach the field list behind the table. Fixed by giving Python
   and TypeScript the same walk.
3. **A bracket that declares nothing.** The "declared child" lookahead matched the regex
   `\[\[[^\]]+\]\]` anywhere in a deeper heading, so brackets inside a code span counted, and so did a
   reference: `### See [[#s]]` made the heading above it an object AND gave it a field literally
   named `#s`. All three were wrong. Fixed in all three by stripping code spans and excluding a
   leading `#`.

**A fourth, from the same family.** TypeScript's comment-end scan existed in two copies — one for the
table/`---`/quote capture, one for the fence capture — and both of this task's corrections went into
the first only. A fence body followed by `### Note` and `- Table: x` therefore still had TypeScript
invent a field `Table` where Rust and Python produced two comments. The two copies are now one helper,
`findCommentEnd`, so a future correction cannot land on one and miss the other.

**A fifth, found by the fix itself.** Deleting the deeper-heading reading in Python exposed a branch
whose comment and condition had disagreed all along: "sibling or shallower heading follows — empty
text field" tested only that a heading followed, not its level. It had been unreachable for a deeper
heading; once it was reachable, the field came out empty and its content became a comment on the
PARENT. Found because the new fixture 264 diverged, not by reading.

- category: parser
- related_to: [[#qmd75_body_identity]]
- solution: One predicate in all three — object when a field list of its own or a declaring deeper heading follows before the next heading at its level, implicit text field otherwise. "Declares" is the definition syntax, not any pair of brackets. Six fixtures (264-269) pin all five, each verified red at HEAD.
