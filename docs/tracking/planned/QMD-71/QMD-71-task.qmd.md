# QMD-71: Close the parse-output divergences between the three parsers

## The three parsers disagree on 32 of 108 real documents [[qmd71: Feature]]

QMD-70 added `make validate-compare`'s parse-output comparison and it measured, for the first time,
how far the three implementations have actually drifted: **32 of the 108 `.qmd.md` documents under
`docs/` parse differently** between Rust, Python and TypeScript. The count is held as a ratchet in
`scripts/parse-parity-baseline.json` so it cannot grow, but nothing has been fixed yet.

The README's central promise is that the three are "kept at byte-for-byte parity by a shared
conformance test corpus". That is true of the corpus and false of real documents, and it was
unmeasurable before because the old comparison looked only at validation-error lists.

- status: planned
- priority: high
- category: parser
- related_task: [[#qmd70_table_scope]]
- requires_changes: [qmdc-rs/src/parser.rs, qmdc-py/qmdc/parser.py, qmdc-ts/src/parser.ts]
- findings: []
- result: null

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
| 1 | seven keys at once | `docs/tracking/workflow.sop.qmd.md`, probably several causes in one file |

Three causes are already reduced to minimal reproducers. Each is a few lines and each is a genuine
bug in ONE implementation, not an undefined construct.

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

1. ~~Classify all 32 by cause.~~ **Done during analysis** — see the three cause sections above. Five
   causes account for 31 of the 32; `docs/tracking/workflow.sop.qmd.md` differs on seven keys at once
   and is the one file still to be broken down.
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
4. Then the seven-key file, which likely resolves itself once the three causes above are closed.

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

### Acceptance [[qmd71_acceptance: text]]

- about: [[#qmd71]]

- `make validate-compare` reports 0 divergent documents, and the baseline file is deleted rather than
  lowered to zero, so a future divergence fails immediately instead of fitting under a cap.
- Every closed cause has a fixture in the shared corpus, and so does its valid neighbour.
- `make test` green, with no new deliberately-failing cases.
