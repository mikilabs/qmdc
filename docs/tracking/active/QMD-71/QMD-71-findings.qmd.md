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

## The last four documents held four more causes [[qmd71_finding_last_four: Finding]]

Causes eight through eleven, all found the same way: by re-measuring after each fix instead of
trusting the original classification. The triage counted five causes in 32 documents; the real number
was ELEVEN.

- category: parser
- priority: high
- affected_files: [qmdc-rs/src/parser.rs, qmdc-ts/src/parser.ts]
- affected_functions: [parse]
- solution: Three defects in Rust and one in TypeScript, described below.
- test_plan: pinned by `tests/parser/239`, `tests/parser/240`, `tests/parser/241`

### Rust rebuilt comment text instead of slicing it [[qmd71_finding_rebuild_inline: text]]

- about: [[#qmd71_finding_last_four]]

The worst of the four, because it LOST data. The format is explicit that comment content is
"the raw markdown fragment between structural boundaries" and that "the parser does not interpret"
it. Rust's paragraph path rebuilt the text from inline events, handling text, code spans, strong, em,
strikethrough and links — and nothing else.

Two consequences, one of them silent data loss:

| input | Rust produced | py, ts |
| --- | --- | --- |
| `![Alt](../img/p.png)` | `Alt` | the image, unchanged |
| `<https://example.com/x>` | `[https://example.com/x](https://example.com/x)` | the autolink, unchanged |

An image in a comment lost its markup and its path entirely, keeping only the alt text — which is why
`docs/tracking/screenshots-temp.qmd.md` diverged: every one of its screenshots was reduced to a
caption. The autolink case merely rewrote the author's text, and it is what `docs/tracking/done/QMD-69/QMD-69-task.qmd.md`
hit when citing the upstream issue.

Fixed by slicing the source, which the text-field and blockquote paths already did — the blockquote one
since QMD-70. Patching the reconstruction instead would have meant chasing every inline construct
forever: footnotes, inline HTML, hard breaks, reference links.

### Rust did not treat a fence as a block for comment merging [[qmd71_finding_fence_block: text]]

- about: [[#qmd71_finding_last_four]]

Rust already had the right concept — a `last_comment_was_block` flag — and set it for `---` and
blockquotes but not for fences. So prose after a code block started a SECOND comment entry where the
other two continue the first.

Getting this right needed the rule measured rather than guessed, because the first attempt overshot and
merged a case that must stay split:

| shape | py, ts | rs before | rs after first attempt |
| --- | --- | --- | --- |
| field, fence, prose | 1 comment | 2 | 1 |
| field, prose, fence, prose | 2 comments | 2 | 1 (wrong) |
| field, prose, fence | 1 comment | 1 | 1 |

The distinction is what STARTED the comment. A comment started by a fence continues into the prose
after it; a comment started by prose does not, even once a fence has been appended to it. So the flag
belongs only in the branch where the fence CREATES the comment, not in the branch where it appends to
one. Rust's flag already encoded exactly this — it was simply never set for fences.

### TypeScript forgot the anchor for a text field fed by a list [[qmd71_finding_ts_anchor: text]]

- about: [[#qmd71_finding_last_four]]

The one document where TypeScript was the outlier, and the same class of defect as [[#qmd71_finding_split]]
and A1: the mechanism existed and one path skipped it.

When a `text` field's content begins with a bullet list, TypeScript writes the field, its `__syntax`
and its `__labels` — and never records the field as the comment anchor. Following content then fell back
to whatever was anchored before the heading, which is the last SCALAR field of the parent object. In
`QMD-71-task.qmd.md` that put a comment belonging to the goals section onto `result`, a field about
forty lines earlier.

The branch immediately below it, for `pendingArrayField`, has always set the anchor. One line, and the
sibling two lines down showed what it should be.

### The method, stated once more [[qmd71_finding_remeasure: text]]

- about: [[#qmd71_finding_last_four]]

Eleven causes were found where classification predicted five, and every one of the six extras came
from the same step: after fixing a cause, re-measure the documents it was supposed to close.

Three of them hid inside `docs/tracking/workflow.sop.qmd.md` alone. An aggregate per-key diff reports
THAT a document differs, never how many independent reasons it differs for, so the count not dropping
as predicted is the only signal that another cause is behind the one just fixed.

## A construct sweep found five more, and one option flag behind them [[qmd71_finding_sweep: Finding]]

The `docs/` corpus only exercises shapes the project's own documents happen to use. An 82-probe sweep
over Markdown constructs — inline, block, field values, structure — found **15 divergent shapes**, of
which five were outright defects and eight are the numeric-literal question below. All five are fixed.

- category: parser
- priority: high
- affected_files: [qmdc-rs/src/parser.rs, qmdc-rs/src/parser_modules/value_parser.rs, qmdc-py/qmdc/parsers/field.py]
- affected_functions: [parse, parse_field_value]
- solution: Four fixes in Rust, one in Python. One remaining divergence needs a decision — the link reference definitions, in the last section below.
- test_plan: pinned by `tests/parser/242` through `247`

### What the sweep found [[qmd71_finding_sweep_table: text]]

- about: [[#qmd71_finding_sweep]]

| shape | outlier | what it did | fixture |
| --- | --- | --- | --- |
| `***`, `___`, `- - -` | rs | rewrote every thematic break as `---`, and merged the paragraph after it | `242` |
| a list inside a blockquote | rs | emitted the quoted list TWICE, one copy missing its first marker | `243` |
| `[^1]: note` | rs | dropped the `[^1]:` label | `244` |
| `[d]: url` | rs, ts | dropped the definition entirely | `245`, unfixed |
| `- k: ~` | rs | read YAML's null | `246` |
| `- k: 1_000` | py | read 1000, via Python's own numeric literal syntax | `247` |

The thematic break was a literal `"---".to_string()` in the source, which is the same class of defect as
the autolink rewrite in [[#qmd71_finding_last_four]] — content asserted rather than sliced. Its
merge behaviour needed the same narrowing the fence path had just received: the flag belongs only where
the rule STARTS the comment.

The duplicated blockquote is the only true data CORRUPTION found in this task. Two of the three list
accumulation sites in Rust appended quoted items to the comment list, while the blockquote handler
already emitted the whole quote verbatim — so the content appeared twice, and because the accumulator's
raw-slice start points past the quote prefix, the first copy had lost its quote marker.

### The option flag behind the footnote [[qmd71_finding_options: text]]

- about: [[#qmd71_finding_sweep]]

Worth more attention than the footnote itself. Rust built its Markdown parser with:

```rust
let md_options = Options::all() - Options::ENABLE_SMART_PUNCTUATION;
```

`Options::all()` is every extension `pulldown-cmark` happens to ship — a set that GROWS on a dependency
bump. So Rust's idea of Markdown was defined by the library's feature list rather than by QMD.md, and it
could change without a single line changing here. Footnotes are the harm that surfaced: with them on,
`[^1]: text` becomes a `FootnoteDefinition` whose inner paragraph begins AFTER the label, which is how
the label and the space after it were lost. QMD.md defines no footnote syntax, so the construct must stay ordinary text.

Now pinned explicitly. That version of `Options::all()` also carries wikilinks (`[[...]]`, which
collides with QMD.md's own reference syntax), math, definition lists, superscript and subscript — none
defined by the format. The sweep found no divergence from those, so they are left alone, but the flag
choice is recorded here because "all extensions the library offers" is not a specification.

### Link reference definitions, closed [[qmd71_finding_reflink: text]]

- about: [[#qmd71_finding_sweep]]

The last one fixed, and it turned out far smaller than the architectural change I had described. The
operator chose to align with Python — keep the definitions — and measuring the two failing parsers
separately showed they were failing for DIFFERENT reasons, each with a contained fix.

Neither `pulldown-cmark` nor `markdown-it` emits an event for `[d]: https://example.com/d`: both consume
the definition into a link map. Python keeps it anyway because Python slices a comment by LINE RANGE, to
the next structural boundary, so anything between is preserved whether or not the tokenizer reported it.
Rust and TypeScript slice per EVENT, so an unreported construct simply vanishes.

Python's behaviour is the one the format describes — "the raw markdown fragment", parser "does not
interpret" — and the other two lost the definition, which made every reference link in that comment
unresolvable. The cost was visible in a round trip: rebuilding from Rust's output produced a document
whose `[d]` and `[m]` labels pointed at nothing, so they would render as literal text.

**Measuring first is what made this cheap.** I had assumed both parsers needed the same broad change.
They did not:

| shape | py | rs before | ts before |
| --- | --- | --- | --- |
| para, definition, then more content | keeps it | drops it | **keeps it** |
| para, definition, end of document | keeps it | drops it | drops it |

TypeScript was already right whenever anything followed the definition. Its comment scan extends the end
line token by token, so it only failed when the scan ran out of tokens — at which point it stopped at the
last token's line instead of the end of the document. One `if` after the loop, and the comment-heading
path above it already defaulted to `lineCount` for exactly this reason.

Rust failed always, because its paragraph comment sliced to the paragraph's own `range.end`. It now
slices to the NEXT EVENT's start, which is the precise statement of the rule: anything the event stream
does not account for is content the tokenizer swallowed, and Python keeps it. A gap holding only a blank
line is removed by the existing `trim`, so nothing else changed — and that is why this did not disturb
the fence and thematic-break boundary rules fixed earlier in the same arm.

Pinned by `tests/parser/245` (definitions at end of document, two of them) and
`tests/parser/252` (a definition followed by a paragraph, and again inside a nested object, so both
branches are covered). The round trip is now byte-identical to the source in all three.

## The numeric grammar, decided [[qmd71_finding_numbers: Finding]]

Eight of the sweep's fifteen divergences were one family, and no two parsers agreed. The operator
decided the rule: a number is an integer or a decimal, nothing else.

- category: parser
- priority: high
- affected_files: [qmdc-rs/src/parser_modules/value_parser.rs, qmdc-py/qmdc/parsers/field.py, docs/format/types.qmd.md]
- affected_functions: [parse_field_value, is_integer_or_decimal]
- solution: All three now gate on `-?\d+(\.\d+)?`. TypeScript already did, so it became the reference.
- test_plan: pinned by `tests/parser/248`

### What each one accepted [[qmd71_finding_numbers_before: text]]

- about: [[#qmd71_finding_numbers]]

| value | rs | py | ts |
| --- | --- | --- | --- |
| `1e5` | number | string | string |
| `1.5e-3` | number | **number** | string |
| `2E3` | number | string | string |
| `.5`, `5.` | number | number | string |
| `+1`, `+1.5` | number | number | string |
| `1_000` | string | **number** | string |

Every cell that is not "string" is a host language showing through. Rust's `parse::<f64>` accepts
exponents and a bare dot and `parse::<i64>` accepts a unary plus; Python's `int()` accepts digit
separators and its `float()` accepts the rest. Python was also internally inconsistent — `1e5` was a
string but `1.5e-3` a number, because the code only reached `float()` when the value contained a `.`.
TypeScript alone had an explicit grammar, `^-?\d+(\.\d+)?$`, which is why it became the reference.

Rust's check is hand-written rather than a regex: `parse_field_value` runs once per field, and
compiling a regex there would too.

### The specification promised the opposite [[qmd71_finding_numbers_spec: text]]

- about: [[#qmd71_finding_numbers]]

Worth recording, because this is a format CHANGE and not only an alignment. `docs/format/types.qmd.md`
said "Integer, float, or scientific notation → Number" and gave `1.5e10` as an example. So the written
format promised exponent support that **only Rust delivered** — Python delivered half of it and
TypeScript none.

Checked before changing it: scientific notation appears in exactly three field values across `docs/`
and `tests/`, and all three are inside `tests/parser/248`, the fixture written for this decision. No
real document relied on the promise, so nothing breaks. The spec now states the grammar it actually
has, and lists every rejected form by name so the next reader does not have to rediscover which host
language accepted what.

### Decided: nothing beyond 2^53-1 is a number [[qmd71_finding_numbers_big: text]]

- about: [[#qmd71_finding_numbers]]

The grammar decision did not settle this, because a long run of digits IS an integer and the
disagreement was about REPRESENTATION:

| value | rs | py | ts |
| --- | --- | --- | --- |
| `9007199254740991` (2^53-1) | exact | exact | exact |
| `9223372036854775807` (i64 max) | exact | exact | 9223372036854776000 |
| `123456789012345678901234567890` | 1.2345678901234568e+29 | exact | 1.2345678901234568e+29 |

TypeScript cannot hold an integer above 2^53-1 in a JSON number at all, so no choice made all three
agree AND keep the value. The operator chose exactness: anything past the bound is a String, so the
digits the author wrote survive untouched instead of being rounded into a different number.

The bound applies to decimals too, and that is where it paid for itself twice. Beyond it the three
JSON writers disagree on the SPELLING of the same double — `12345678901234567890.5` came back as
`1.2345678901234567e+19` from Rust and Python and as all its digits from JavaScript — so drawing the
line at 2^53-1 removed that whole class of divergence instead of leaving it to be chased later.

Two defects in this task's own earlier work surfaced while pinning it. The `__raw_values` spelling added
for [[#qmd71_finding_float]] echoed the AUTHOR's text whenever a decimal's value was integral, so
`100.000` came back as `100.000` where the other two canonicalise to `100.0`, and a decimal whose
fraction had been rounded away reprinted digits the double no longer held. The spelling is now derived
from the value. It also dropped the sign of negative zero, because `${-0}` is `"0"` in JavaScript —
`Object.is` is what tells that case apart.

One more thing this pinned, worth stating because it is a property of the format rather than a bug: a
decimal carries double semantics, so authored precision beyond a double's is lost.
`0.12345678901234567890` reads back as `0.12345678901234568`, and the text round-trip test correctly
reports that as content loss. The fixture uses a clean decimal instead, and the spec now says so
outright.

## Closed by refusing the value: unsupported number formats [[qmd71_finding_spelling: Finding]]

The last divergence found, isolated to decimals with a magnitude below `1e-4`. Not a type question —
all three agreed it was a number and agreed on its VALUE. They disagreed on how to write it down.

The operator declined both options I put up and asked for a third: raise an error saying the format is
not supported. That turned out to be the best of the three by a wide margin, and it closed more than the
case it was asked about.

- category: parser
- priority: high
- affected_files: [qmdc-py/qmdc/parsers/field.py, qmdc-rs/src/parser_modules/value_parser.rs, qmdc-ts/src/parsers/field.ts, docs/format/validation-errors.qmd.md]
- affected_functions: [is_unsupported_number, parse_field_value]
- solution: New error `unsupported_number_format`. The value keeps its authored text; the error names the spelling.
- test_plan: pinned by `tests/parser/251-unsupported-number-format`, and `250` went green

### The measured boundary [[qmd71_finding_spelling_table: text]]

- about: [[#qmd71_finding_spelling]]

| authored | rs | py | ts |
| --- | --- | --- | --- |
| `0.0001` | `0.0001` | `0.0001` | `0.0001` |
| `0.00001` | `0.00001` | `1e-05` | `0.00001` |
| `0.000001` | `1e-6` | `1e-06` | `0.000001` |
| `0.0000001` | `1e-7` | `1e-07` | `1e-7` |

Three different switch points into exponent form and two different exponent paddings. Python switches
below `1e-4` and pads the exponent to two digits; Rust switches below `1e-5` with minimal digits;
JavaScript switches below `1e-6` with minimal digits. Above `1e-4` all three agree, and everything else
about decimals now agrees too — `-0.0`, `100.000`, `0.30000000000000004` and the rest are identical in
all three.

Each parser is emitting its host language's default float formatting, which is the same root cause as
the numeric grammar: the format inherited whatever the language did. Fixing it means choosing which
spelling is canonical and then writing floats by hand in at least two of the three, since none of the
three defaults matches another.

### It breaks the round trip, so it is not cosmetic [[qmd71_finding_spelling_rt: text]]

- about: [[#qmd71_finding_spelling]]

The decided grammar has no exponent form, so a parser that PRINTS one emits JSON its own parser will not
read back as a number. `- g: 0.00001` becomes `1e-05` in Python, and re-parsing that yields the string
`"1e-05"`.

In Python this was already true before this task: its old numeric path only reached `float()` when the
value contained a `.`, so `1e-05` was already a string on the way back in. The bug was simply never
exercised, because no fixture carried a decimal that small. In Rust the round trip DID work before and
the grammar decision broke it — Rust used to accept `1e-6` on re-read and now does not. The red fixture
records that honestly rather than hiding it.

### Why the third option beat both of mine [[qmd71_finding_spelling_options: text]]

- about: [[#qmd71_finding_spelling]]

I offered two, and both were worse than what was asked for.

**Never print an exponent** would have kept every value a number, at the cost of a hand-written float
formatter in all three — no two host defaults agree — and a very small double spells out long (`5e-324`
becomes 324 digits).

**Bound the magnitude from below** was one line per parser but silently demoted `- rate: 0.00001` to a
string with no explanation, and its threshold was borrowed from Python's `repr` switch point rather
than derived from anything in the format.

**Raise an error** does the same demotion but SAYS SO, which is the part both of mine were missing. It
also generalises: the same error covers every numeric spelling the format does not define, so `1e5`,
`.5`, `+1`, `1_000` and `0x1f` stopped being silent strings too. One rule replaced a list of special
cases, and `tests/parser/250` went green on its own — the spelling divergence disappeared rather than
being reconciled, because a value that is never a number is never spelled as one.

Nothing is lost: the field keeps the text the author wrote, so the document still round-trips. Quoting
is the escape hatch — `- max: "9223372036854775807"` raises no error, because the quotes state that the
text is the value.

### What the detector must not match [[qmd71_finding_spelling_detector: text]]

- about: [[#qmd71_finding_spelling]]

The predicate is two questions, and the second is the one worth care. If the value matches the numeric
grammar, only the magnitude bounds can reject it, and that check delegates to the same function
`parse_field_value` uses — so the bounds live in ONE place in each parser and cannot drift. Otherwise the
value is matched against an enumerated list of unsupported numeric shapes.

Enumerated, not heuristic, because "looks like a number" is a trap. These must all stay plain strings
with NO error: `2026-09-24`, `12:30:00`, `1.0.2`, `1 000`. A version number and a date are values, not
failed numbers, and a detector loose enough to flag them would make the error worse than the silence it
replaced. `tests/parser/251` pins all four alongside the eleven shapes that do raise it.

Two more deliberate non-errors: a bare `~` is a String because that spelling belongs to YAML, and
`Infinity` and `NaN` are Strings for the same reason — nobody mis-spelled a number, they wrote a word.

### A regression this refactor introduced and the test caught [[qmd71_finding_spelling_regression: text]]

- about: [[#qmd71_finding_spelling]]

Extracting the shared bounds into one function in TypeScript moved the integer and decimal paths behind a
single call — and the spelling logic after it then fired for plain integers too, so `42` came back as
`42.0`. The `value.includes('.')` guard is what separates "a decimal whose value is integral, which needs
its fraction written back" from "an integer, which never did". Worth recording because the earlier
trailing-zero work had the same shape of bug twice: the spelling mechanism is easy to apply too widely.

## Open questions [[qmd71_finding_questions: Finding]]

Three decisions were needed before the corresponding fixes. Two were answered by measurement rather
than preference, as recorded below. The third, plus two divergences found while closing the corpus,
remain OPEN and are follow-up work rather than part of this task.

- category: parser
- priority: high
- affected_files: []
- solution: Questions 1 and 2 answered during implementation. Question 3 and the two carried-over divergences need an operator decision and belong in their own task.
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
   fail differently. That may be its own task. **Still open** — none of these appear in the `docs/`
   corpus, so they did not block the acceptance criterion.

### Carried forward, found while closing the corpus [[qmd71_finding_carried: text]]

- about: [[#qmd71_finding_questions]]

Two divergences outside the `docs/` corpus, so neither blocked the parity gate. Both are recorded here
rather than fixed, because each needs a decision about what the construct MEANS rather than a
reconciliation between implementations.

**A non-field bullet nested under an empty-valued field.** For `- items:` followed by an indented
`- ![Alt](p.png)`, Rust emits a `nested_subitems` parsing error while Python and TypeScript emit a
comment holding the raw list. This is the same neighbourhood as [[#qmd71_finding_nested]] and it is what
the parity gate was verified against, so it is definitely real. `nested_subitems` exists in all three
parsers and in `docs/format/`, so the question is which shapes should raise it — not whether it exists.

**`rebuild` cannot place a comment anchored on a field that sits after a later heading.** A comment
anchored on a `text` field but written after a following object-array heading is re-emitted directly
after the field, which moves it above that heading; re-parsing then reads it as the field's content.
This is a limitation of the anchor model itself, not a parser divergence — all three do the same thing —
and QMD-70 hit the same wall from a different direction. Any fix means giving a comment a position as
well as an anchor, which is a format decision.

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
