# QMD-71: Result

## The three parsers agree on every document in the corpus [[qmd71_result: Result]]

The 32 divergent documents `make validate-compare` measured at the start of this task are now **0 of
111**, and `scripts/parse-parity-baseline.json` is deleted rather than zeroed, so any new divergence
fails immediately instead of fitting under a cap.

All eight goals are complete. `make test` is green end to end: **3691 cases, 0 failures**.

- feature: [[#qmd71]]
- files_changed: [qmdc-rs/src/parser.rs, qmdc-rs/src/parser_modules/value_parser.rs, qmdc-rs/src/parser_modules/mod.rs, qmdc-py/qmdc/parser.py, qmdc-py/qmdc/parsers/field.py, qmdc-ts/src/parser.ts, qmdc-ts/src/parsers/field.ts, qmdc-ts/src/cli.ts, scripts/compare_parse_output.py, docs/format/types.qmd.md, docs/format/validation-errors.qmd.md]
- tests_added: [tests/parser/235-comment-anchor-after-text-field.qmd.md, tests/parser/236-text-field-ordered-list-with-fence.qmd.md, tests/parser/237-float-trailing-zero.qmd.md, tests/parser/238-comment-split-at-subheading.qmd.md, tests/parser/239-comment-raw-image-and-autolink.qmd.md, tests/parser/240-comment-after-fence-boundary.qmd.md, tests/parser/241-text-field-fed-by-list-anchor.qmd.md, tests/parser/242-comment-hr-variants-verbatim.qmd.md, tests/parser/243-comment-blockquote-with-list.qmd.md, tests/parser/244-comment-footnote-definition.qmd.md, tests/parser/245-comment-link-reference-definition.qmd.md, tests/parser/246-value-tilde-is-a-string.qmd.md, tests/parser/247-value-underscore-digits-is-a-string.qmd.md, tests/parser/248-value-numeric-grammar.qmd.md, tests/parser/249-value-number-range.qmd.md, tests/parser/250-value-decimal-spelling.qmd.md, tests/parser/251-unsupported-number-format.qmd.md, tests/parser/252-link-definition-then-boundary.qmd.md, tests/cli/022-nested-list-item-fields]

### The count was wrong, and finding out how was the work [[qmd71_result_count: text]]

- about: [[#qmd71_result]]

Triage classified the 32 documents into five causes. The real number was **eleven**, plus five more
found afterwards by a construct sweep the corpus could not reach.

Every one of the extras came from the same step: after fixing a cause, RE-MEASURE the documents it was
supposed to close. An aggregate per-key diff reports THAT a document differs, never how many
independent reasons it differs for. `docs/tracking/workflow.sop.qmd.md` alone held three causes in
three different parsers, and the third only surfaced because the parity count did not drop after two
fixes that should have closed that file.

This is the finding worth carrying out of the task: the count not moving as predicted is the only
signal that another cause is hiding behind the one just fixed.

### The same defect shape, five times [[qmd71_result_shape: text]]

- about: [[#qmd71_result]]

Five of the fixes were not new code. The correct code already existed and one path could not reach it.

| where | what was unreachable |
| --- | --- |
| ts, text-field content | a raw-slice branch for lists, blocked by an earlier arm matching `ordered_list_open` unconditionally |
| ts, float spelling | `parseFieldValue` already recorded the authored text in `__raw_values`; the parse-output path never used it |
| py, nested bullets | the "ordered list before fields" case was the only one of three siblings without a raw-slice path |
| py and ts, comment boundary | both have a comment-heading handler that groups correctly; the paragraph run swallowed the heading and advanced the token index past it |
| ts, comment anchor | the branch writing a text field fed by a list never recorded the anchor, while its sibling two lines below always had |

A1 is the clearest: the fix was to DELETE about sixty lines of reconstruction, which became dead once
ordered lists were handed to the branch bullet lists already used.

### Rust rebuilt what it should have sliced [[qmd71_result_rebuild: text]]

- about: [[#qmd71_result]]

The largest group of real damage. `docs/format/comments.qmd.md` says comment content is "the raw
markdown fragment between structural boundaries" and that the parser "does not interpret" it. Rust's
paragraph path rebuilt the text from inline events, handling text, code spans, strong, em,
strikethrough and links — and nothing else.

| input | Rust produced |
| --- | --- |
| `![Alt](../p.png)` | `Alt` — markup and path gone |
| `<https://example.com/x>` | `[https://example.com/x](https://example.com/x)` |
| `***`, `___`, `- - -` | `---`, every spelling normalised |
| `[^1]:` note | the label dropped |
| a list inside a blockquote | the list TWICE, one copy missing its marker |

The image case is why every screenshot in `docs/tracking/screenshots-temp.qmd.md` had been reduced to a
caption. The blockquote case is the only true data CORRUPTION found in the task: two of Rust's three
list-accumulation sites appended quoted items while the blockquote handler already emitted the whole
quote verbatim.

All of it is fixed by slicing the source, which the text-field and blockquote paths already did — the
blockquote one since QMD-70. Patching the reconstruction would have meant chasing every inline
construct forever: footnotes, inline HTML, hard breaks, reference links.

### The specification decided twice, both times against the majority [[qmd71_result_spec: text]]

- about: [[#qmd71_result]]

Two questions looked like operator decisions and were not, because the written format already answered
them — and in both cases two parsers were wrong and one was right.

**Where a comment block ends at a nested bare heading.** `comments.qmd.md` says a comment heading takes
"all content below them", and its boundary list does not include a deeper bare heading. That is Rust's
reading. Python and TypeScript also contradicted THEMSELVES: raise the same heading one level, to the
object's own level, and all three group it with the text below.

**What an empty value means.** `types.qmd.md` says "Null: keyword `null` or empty value after colon".
Rust matches; Python and TypeScript return an empty string. Recorded, not fixed — it does not occur in
the corpus and it sits next to the `nested_subitems` question, so it is carried forward rather than
changed alongside eleven other things.

### Rust's Markdown was the library's, not the format's [[qmd71_result_options: text]]

- about: [[#qmd71_result]]

Worth more than the defect that exposed it. Rust built its parser with `Options::all()` minus smart
punctuation — every extension `pulldown-cmark` happens to ship, a set that GROWS on a dependency bump.
So Rust's idea of Markdown was defined by the library's feature list rather than by QMD.md, and it could
change with no code change here.

Footnotes are the harm that surfaced: with them on, `[^1]: text` becomes a `FootnoteDefinition` whose
inner paragraph begins AFTER the label. The extension set is now pinned explicitly. That version of
`Options::all()` also carries wikilinks — `[[...]]`, which collides with QMD.md's own reference
syntax — plus math, definition lists, superscript and subscript. The sweep found no divergence from
those, so they are left alone, but "all extensions the library offers" is not a specification.

### Two format decisions, made by the operator [[qmd71_result_format: text]]

- about: [[#qmd71_result]]

**A number is an integer or a decimal, with a magnitude between `1e-4` and 2^53-1.** Eight sweep
divergences were one family and no two parsers agreed; every disagreement was a host language showing
through. `parse::<f64>` accepts exponents and a bare dot, `parse::<i64>` a unary plus; Python's `int()`
accepts digit separators, and Python was internally inconsistent — `1e5` a string but `1.5e-3` a number,
because it only reached `float()` when the value held a dot. TypeScript alone had an explicit grammar,
so TypeScript became the reference.

This is a format CHANGE: `types.qmd.md` promised scientific notation and gave `1.5e10` as an example, a
promise only Rust delivered. Checked before changing it — exponent notation appears in exactly three
field values across `docs/` and `tests/`, all three inside the fixture written for this decision.

**An unsupported numeric spelling raises `unsupported_number_format`.** The operator declined both
options offered for the tiny-decimal divergence and asked for a third: refuse the value out loud. It
was better than either. The field keeps the text the author wrote, so nothing is lost and the document
still round-trips; quoting is the escape hatch. And it generalised — the same rule covers every
spelling the format does not define, so `1e5`, `.5`, `+1`, `1_000` and `0x1f` stopped being silent
strings too. `tests/parser/250` went green on its own: the spelling divergence disappeared rather than
being reconciled, because a value that is never a number is never spelled as one.

The detector is ENUMERATED, not heuristic. "Looks like a number" is a trap: `2026-09-24`, `12:30:00`,
`1.0.2` and `1 000` must stay plain strings with no error, and a detector loose enough to flag a date or
a version number would be worse than the silence it replaced.

### Verification [[qmd71_result_verification: text]]

- about: [[#qmd71_result]]

- `make test`: **3691 cases, 0 failures**, all 20 JUnit reports written.
- `make validate-compare`: **0 divergent of 111** documents, with no baseline file present, so the gate
  is absolute.
- The gate was verified to FAIL, not only to pass: planting one divergent document made it exit 1 and
  name the file.
- `make md-lint`, `ruff check`, `cargo clippy` and `npm run lint` clean; the five TypeScript `any`
  warnings are pre-existing and unchanged in count.
- Each regression was verified per fixture and per parser with a direct diff, not by reading the
  aggregate line. `tests/parser/241` was verified the harder way: the TypeScript fix was temporarily
  reverted, the fixture confirmed to produce the wrong anchor, and the fix restored.

Two measurement traps met along the way, both recorded because they produce false confidence. `nextest`
aborts on the first failure and never writes several XML reports, so runs that looked like 2146 or 3280
cases were truncated — 3691 is the real count. And the microtest harness compares parsed VALUES, not
bytes, so `1e-05` and `0.00001` look equal to it; a float-spelling divergence is only visible through
the round trip, where the type flips.

### Carried forward [[qmd71_result_carried: text]]

- about: [[#qmd71_result]]

Three items, none in the `docs/` corpus, each needing a decision about what a construct MEANS rather
than a reconciliation between implementations. All recorded in the findings.

- A non-field bullet nested under an empty-valued field (`- items:` then `- ![Alt](p.png)`): Rust
  reports `nested_subitems`, the other two emit a comment. `nested_subitems` exists in all three
  parsers and in `docs/format/`, so the question is which shapes should raise it.
- An empty inline field: `null` in Rust, `""` in the other two. The spec says Rust is right.
- `rebuild` cannot place a comment anchored on a field that sits after a later heading — a limitation of
  the anchor model itself, not a divergence; all three do the same thing. QMD-70 hit the same wall from
  another direction.

YAML block scalars (`|-`, `|+`, `>`) are explicitly out of scope: the operator confirmed they are
deliberately unsupported, which closes the third open question from triage.
