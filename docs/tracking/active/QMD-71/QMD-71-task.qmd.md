# QMD-71: Close the parse-output divergences between the three parsers

## The three parsers disagree on 32 of 108 real documents [[qmd71: Feature]]

QMD-70 added `make validate-compare`'s parse-output comparison and it measured, for the first time,
how far the three implementations have actually drifted: **32 of the 108 `.qmd.md` documents under
`docs/` parse differently** between Rust, Python and TypeScript. The count is held as a ratchet in
`scripts/parse-parity-baseline.json` so it cannot grow, but nothing has been fixed yet.

The README's central promise is that the three are "kept at byte-for-byte parity by a shared
conformance test corpus". That is true of the corpus and false of real documents, and it was
unmeasurable before because the old comparison looked only at validation-error lists.

- status: done_review
- priority: high
- category: parser
- related_task: [[#qmd70_table_scope]]
- requires_changes: [qmdc-rs/src/parser.rs, qmdc-py/qmdc/parser.py, qmdc-ts/src/parser.ts]
- findings: [[[#qmd71_finding_anchor]], [[#qmd71_finding_textfield]], [[#qmd71_finding_float]], [[#qmd71_finding_nested]], [[#qmd71_finding_renumber]], [[#qmd71_finding_split]], [[#qmd71_finding_last_four]], [[#qmd71_finding_questions]], [[#qmd71_finding_tests]]]
- result: [[#qmd71_result]]

### Why this matters more than its size suggests [[qmd71_why: text]]

- about: [[#qmd71]]

Every divergence is a place where the answer an agent gets over MCP, the diagnostics an author sees
in the editor, and the graph a build produces depend on WHICH parser ran. The LSP and MCP are Rust;
`qmdc-mkdocs` is Python; a browser consumer is TypeScript. So a divergence is not academic — it is
the same document meaning different things in the editor and on the docs site.

QMD-70 also showed the failure mode is not random. Divergences cluster where the FORMAT is undefined,
and each one found there was silent: data lost, a `Kind` degraded, a table reduced to cell texts. Four
constructs were made errors in that task precisely to shrink the undefined space. The 32 here are the
remainder, in defined space, and they are bugs rather than open questions.

### What the 32 actually are [[qmd71_shape: text]]

- about: [[#qmd71]]

All 32 were classified by diffing every `(object-id, key)` pair across the three parsers, so these are
measured counts rather than a sample. Exactly one parser is the outlier in every case — 24 times Rust,
8 times TypeScript — and no document has all three disagreeing.

| # | differing key | cause |
| --- | --- | --- |
| 20 | `__comments` | comment anchor (Rust) |
| 3 | `__comments`, `__syntax`, `__types` | comment anchor plus a field-type effect, needs its own look |
| 3 | `version` | trailing-zero float (TypeScript) |
| 5 | a named `text` field | content lost inside a text field (TypeScript) |
| 1 | ten keys at once | `docs/tracking/workflow.sop.qmd.md` — Rust extracting fields from a nested list item, plus Python renumbering a nested bullet list |

SIX causes, all reduced to minimal reproducers during triage. Each is a few lines and each is a genuine
bug in ONE implementation, not an undefined construct. The last two share a single document, which is
why the first pass counted five: an aggregate per-key diff shows that a file differs, not that it
differs for two independent reasons in two different parsers.

### Cause 1 — Rust anchors a comment on the wrong thing [[qmd71_cause_anchor: text]]

- about: [[#qmd71]]

Twenty documents, and the single biggest group.

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

Rust anchors the comment on `gen` — the preceding OBJECT — where Python and TypeScript anchor it on
`content`, the text FIELD the content actually follows. The `docs/` corpus hits this constantly
because the generated-content pattern (`ContentGenerator` object, then a `content` text field) is used
throughout the guides.

Python and TypeScript agree, and they are right: the content sits under the `content` heading.

### Cause 2 — TypeScript loses content inside a text field [[qmd71_cause_textfield: text]]

- about: [[#qmd71]]

Five documents, and the most damaging of the three: it is silent data loss in a field's VALUE.

```markdown example
# D [[d: G]]

## Solution [[solution: text]]

1. First:

(a fenced json block)

1. Second:

(a second fenced json block)
```

Rust and Python keep the whole thing verbatim. TypeScript yields `"1. First:\n\nSecond:"` — both
fences gone AND the second item's ordered-list marker gone with them. A plain fence inside a text field is
fine in all three; it takes an ORDERED LIST followed by a fence to trigger it, which is why the
earlier probe with a bare fence did not reproduce it.

Visible today in `docs/format/validation-errors.qmd.md`, where the `err_multiple_definitions`
section's `solution` field loses both of its examples and the remaining prose stops making sense.

### Cause 3 — TypeScript drops a trailing zero from a float [[qmd71_cause_float: text]]

- about: [[#qmd71]]

Three documents, and the cheapest to fix.

`- version: 2.0` parses to `2.0` in Rust and Python and to `2` in TypeScript; `- z: 0.0` likewise
becomes `0`. `1.50` and `3.10` agree, because their value is not integral — so this is JavaScript
number formatting, not parsing: a float whose fractional part is zero serialises without it.

The `__types` entry says `number` in all three, so only the value differs. Worth deciding whether the
fix belongs in the value parse or in the JSON serialisation.

### Separately measured divergences, outside the 32 [[qmd71_extra: text]]

- about: [[#qmd71]]

Found by the QMD-70 code review while probing, each reproduced on a minimal input and none caused by
that task's changes. They do not appear in the `docs/` count because no document happens to use them.

**YAML block scalars with a chomping indicator are broken three different ways.** For
`- v: |-` followed by an indented block, Rust stores `'|- onetwo'` (marker kept, newlines lost),
Python `'|-\none'` (marker kept, content truncated), TypeScript `'|-'` (content lost entirely). Only
a bare `|` is handled identically. The same applies to `|+`, `>`, `>-`, `>+`. QMD-70 stopped these
being reported as `block_in_inline_field` errors, but did not make the three agree on the VALUE —
`tests/parser/231-yaml-block-scalar-blank-line` pins only the bare `|` form for that reason.

**An empty inline field differs in type.** `- items:` with nothing after it yields `null` with
`__types: null` in Rust, and `""` with `__types: string` in Python and TypeScript.

**A third was reported and did NOT reproduce**, recorded so nobody re-investigates it: array
elements were said to carry a Rust-only `__has_explicit_id: false`. Measured on the reported shape,
all three agree.

### Scope [[qmd71_scope: text]]

- about: [[#qmd71]]

1. ~~Classify all 32 by cause.~~ **Done during triage** — six causes, every one with a minimal
   reproducer and a failing regression. See the findings document.
2. For each cause, decide which behaviour is correct BEFORE changing code. Python is the reference
   implementation, but QMD-70 showed that deferring to it mechanically is wrong: the prose-gap
   question was settled against Python by looking at what all three already agreed on for the
   neighbouring shape. Look for that kind of precedent each time.
3. Fix, with a fixture per cause pinning both the corrected shape AND its nearest valid neighbour.
   QMD-70's lesson: four new rules all needed a valid-side fixture, and the one rule that lacked one
   shipped a false positive that reached review.
4. Lower `scripts/parse-parity-baseline.json` as each cause is closed, via `make parse-parity-baseline`.
5. Decide separately whether the three extra divergences above belong in this task or their own — the
   YAML block-scalar one is arguably a missing FEATURE (chomping indicators are unimplemented) rather
   than a divergence to reconcile.

### Suggested order [[qmd71_order: text]]

- about: [[#qmd71]]

Not by document count — by risk and by cost.

1. **Cause 2 first** (TypeScript losing text-field content). It is the only one that destroys data an
   author wrote, and it is already visible in a shipped documentation file. Five documents.
2. **Cause 3 next** (trailing-zero float). Almost certainly a one-line serialisation fix and it
   removes three documents from the count for very little risk.
3. **Cause 1 last** (Rust's comment anchor), despite being 20 of the 32. It is the largest group but
   also the only one touching comment anchoring, which QMD-70 showed is delicate — the anchor model
   was behind three separate findings there, including one where no anchor value gave a faithful
   round trip. Expect to need a decision, not just a fix.
4. Then the two causes in `workflow.sop.qmd.md` — Rust's nested-field extraction (blocked on open
   question 1) and Python's renumbering, which is independent of it and can go with the earlier group.

A note carried from QMD-70: `parse | rebuild` is not a safe way to verify these. The round-trip has
its own known limitations (text-field heading levels shift, top-level arrays gain a wrapper), so
compare parse output directly — which `make validate-compare` now does.

### Non-goals [[qmd71_non_goals: text]]

- about: [[#qmd71]]

- Not a parser rewrite. The architectural criticism is real — 4779 lines and 157 mutable variables in
  the Rust event loop, with the meaning of a construct spread across time rather than expressed in
  data — but that is its own decision, and every divergence here is fixable without it.
- Not new format rules. Anything that turns out to be UNDEFINED rather than divergent belongs in a
  separate decision, the way QMD-70's four errors did.

### Goals [[goals: [Goal]]]

Ordered by risk, not by document count — see the suggested order above.

#### A1: TypeScript keeps text-field content intact [[qmd71_goal_a1]]

An ordered list followed by a fence inside a `text` field must keep both the fences and the list
markers, matching Rust and Python. This is the only cause that destroys what an author wrote, and it is
already wrong in a shipped documentation page.

**Done (2026-09-24).** The fix was to DELETE code rather than add any. A raw-slice branch for lists in
text fields already existed; it was simply unreachable for ordered lists, because an earlier arm in the
same chain matched `ordered_list_open` unconditionally and rebuilt the value from `inline` tokens.
Excluding `pendingTextField` from that arm hands ordered lists to the branch bullet lists already used,
and the 60-line reconstruction became dead code and was removed.

Closed 5 of the 32 divergent documents, not the 5 predicted plus nothing — among them
`docs/format/validation-errors.qmd.md`, the shipped page whose `solution` field was wrong. Parity
baseline lowered 32 → 28.

- group: A_dataloss
- done: true

#### A2: Floats keep their fractional part [[qmd71_goal_a2]]

`- version: 2.0` must read back as `2.0` in all three, not `2`. Blocked on open question 2 — parse or
serialisation.

**Done (2026-09-24), and the open question answered by measurement rather than preference.** The raw
authored text was ALREADY recorded: `parseFieldValue` keeps it whenever a value contains a `.` and
parses to an integer, and it is stashed on each object as a non-enumerable `__raw_values` for `rebuild`.
So the machinery existed and simply was not used on the parse-output path — which settles the question
in favour of serialisation, with one source of truth rather than a second parallel mechanism.

Implemented as a post-pass over `JSON.stringify` rather than a replacer or a `toJSON` hook: neither can
emit an unquoted `2.0`, because whatever they return is itself serialised.

Closed 3 more documents. Parity baseline 28 → 25.

- group: A_dataloss
- done: true

#### B1: One comment anchor rule across the three [[qmd71_goal_b1]]

Content following a `text` field heading must anchor on that field, not on the preceding object. Covers
the 20 plain cases and the 3 where Rust additionally loses the field's `__syntax` and `__types`.

**Done (2026-09-24).** Two bugs in one branch, and the second explains the "stronger form" the triage
counted separately.

`comment_anchor` lives on Rust's in-flight `CurrentObject`. When a `text` field is declared on a
parent that has already been finalized into `objects_map` there was nowhere to record it, so following
content fell back to "the last field that references a child" — which picks the preceding OBJECT. Rust
now remembers `(parent_id, field_name)` for that case, and the in-flight branch clears it when it takes
the anchoring back.

The same branch also wrote `__types` and `__syntax` only when those maps ALREADY existed, so a text
field on a parent whose other fields are plain scalars got its value and no metadata. That is the
3-document variant; it was one missing `or_insert_with`, not a separate cause.

Closed 13 documents — parity baseline 25 → 12.

- group: B_anchor
- done: true

#### B2: One reach rule for nested list items [[qmd71_goal_b2]]

All three must agree on whether a nested list item's field-like entries become fields. Blocked on open
question 1.

**Done (2026-09-24), and the open question answered by what happened when the guard was too broad.**
Rust no longer extracts fields from a list nested under an ORDERED item, matching the other two.

The first attempt gated on nesting depth alone, and that traded one divergence for another: for a
bullet list under an EMPTY-valued bullet item (`- items:` then `- product: x`) suppressing the field
made Rust emit `nested_subitems` where the other two emit nothing — and it also changed LSP completion
output, because completion consults the parsed fields. Narrowing the guard to "an ancestor list is
ordered" fixes the real divergence and leaves both alone.

That also answers question 1 in the direction the evidence pointed: the one real occurrence is the
SOP's own prose describing what a Finding contains, and Rust was turning documentation text into
fields.

- group: B_anchor
- done: true

#### B3: Python stops renumbering nested bullets [[qmd71_goal_b3]]

A nested bullet list captured as comment content must keep its indentation and markers, not be folded
into the outer ordered list and renumbered. Independent of B2 despite sharing a document — see
[[#qmd71_finding_renumber]].

**Done (2026-09-24).** Python's "ordered list before fields" case was the only one of its three
siblings without a raw-slice path — the two below it already had one. Added it, so all three now slice.

- group: B_anchor
- done: true

#### B4: One comment boundary at a sub-heading [[qmd71_goal_b4]]

All three must agree where a comment block ends at a nested bare heading — see
[[#qmd71_finding_split]]. Found by re-measuring after B2 and B3, not by classification.

**Done (2026-09-24), and it went against the majority.** `docs/format/comments.qmd.md` already says a
comment heading takes "all content below them", which is Rust's reading, and its boundary list does not
include a deeper bare heading. Python and TypeScript also contradicted themselves: the same heading one
level shallower groups the other way in both.

For the third time in this task the correct code already existed and was unreachable. Both parsers have
a comment-heading handler that slices from the heading — the same shape as Rust's. The paragraph run
swallowed the heading and advanced the token index past it, so the handler never saw the token and the
heading ended up glued to the text ABOVE. One `break` in each makes it reachable.

Closed 8 documents — parity baseline 12 → 4.

- group: B_anchor
- done: true

#### C1: A regression per cause, plus a valid neighbour per fix [[qmd71_goal_c1]]

Four regressions land during triage and must fail for their own documented reason. Each fix then adds a
fixture for its nearest valid neighbour, per [[#qmd71_finding_tests]].

**Done (2026-09-24).** Seven fixtures: the four from triage, plus `239` (image and autolink kept raw in
a comment), `240` (both fence-boundary shapes in one file, so the merge rule cannot be half-satisfied)
and `241` (comment anchor for a text field fed by a list).

Two things worth recording about `241`. Its first shape put the comment after an object-array heading,
and that made the ROUND-TRIP test fail in all three: `rebuild` re-emits a comment directly after the
field it is anchored on, which moved it above the array heading, and re-parsing then read it as text
field content. Not a divergence and not something this task introduced — a pre-existing `rebuild`
limitation of the anchor model, the same shape QMD-70 already hit. Reshaped the fixture to pin the same
anchor without crossing an array heading; the limitation is recorded in [[#qmd71_finding_questions]]
rather than papered over.

Second, `241` was verified the way the operator asks for: the TypeScript fix was temporarily reverted,
the fixture was confirmed to produce `status` instead of `why`, and the fix restored. It fails for its
own reason, not by luck.

- group: C_tests
- done: true

#### D1: The baseline file is deleted, not zeroed [[qmd71_goal_d1]]

`make validate-compare` reports 0 divergent documents and `scripts/parse-parity-baseline.json` is
removed, so a future divergence fails immediately instead of fitting under a cap.

**Done (2026-09-24).** 0 divergent of 110, and the file is deleted. Deleting it needed a change to
`scripts/compare_parse_output.py`, which treated a missing baseline as a hard error: an absent baseline
now means parity must be EXACT, and the ratchet stays available via `--update-baseline` for the case it
was built for. A baseline of `0` was rejected deliberately — it would still invite a future
`--update-baseline` to raise it again.

The gate was verified to FAIL, not just to pass: planting one divergent document made it exit 1 and name
the file. The probe used the `- items:` shape from [[#qmd71_finding_questions]], which also confirms that
divergence is real.

- group: D_gate
- done: true

### Acceptance [[qmd71_acceptance: text]]

- about: [[#qmd71]]

- `make validate-compare` reports 0 divergent documents, and the baseline file is deleted rather than
  lowered to zero, so a future divergence fails immediately instead of fitting under a cap.
- Every closed cause has a fixture in the shared corpus, and so does its valid neighbour.
- `make test` green, with no new deliberately-failing cases.
