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

Two groups, by which parser is the odd one out:

| split | count | first identified cause |
| --- | --- | --- |
| `py+ts vs rs` | 24 | Rust anchors a comment on the enclosing OBJECT where the other two anchor it on the text FIELD the content follows |
| `rs+py vs ts` | 8 | TypeScript DROPS fenced code blocks from inside a `text` field's value |

Both were identified by sampling one document from each group, not by reading all 32, so the counts
are the size of each group and not a claim that one cause explains every member. Confirming that is
the first step of the work.

The TypeScript one is data loss and looks like the more serious of the two. In
`docs/format/validation-errors.qmd.md` a `solution` field whose value contains two
```markdown example``` fences comes out as `"1. Split into two headings:\n\nOr use a single definition
with fields:"` — both fences gone, leaving prose that no longer makes sense.

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

1. Classify all 32 by cause — sample every document, not one per group. Expect a handful of causes,
   not 32.
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
