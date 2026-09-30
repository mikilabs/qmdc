# QMD-77: Findings

## Identity: four shapes where the object set itself differs [[qmd77_finding_identity: Finding]]

All four measured on the current build (`9d1f63c`) with all three parsers on the same input.

**A nested duplicate id.** Two `## One [[c: Child]]` / `## Two [[c: Child]]` sections, each with a
deeper `### Deep [[d]]`:

| parser | objects | diagnostics |
|---|---|---|
| rs | `s`, `s.c`(n=1), `s.c.d`(q=2), `s.c.d`(q=1), `s.c`(n=2) | two `duplicate_id` |
| py | `s`, `s.c`(n=1), `s.c`(n=2), `s.c.d`(q=2) | one `duplicate_id` |
| ts | `s`, `s.c`(n=2), `s.c.d`(q=2) | none |

TypeScript keeps one of each pair and reports nothing, so two authored objects vanish in silence.
Python keeps both parents but only one child. Rust keeps everything and reports both, but emits the
duplicated children in the wrong order — `q=2` before `q=1`.

**A wrapped field value.** `- note: first line` continuing on an indented `second line`:

| parser | value |
|---|---|
| rs | `first line second line` |
| py | `first line\nsecond line` |
| ts | `first line` |

Three readings of four lines, and TypeScript's drops content silently. Both of the sibling findings in
QMD-74 (`[[#qmd74_finding_wrapped_field]]`, `[[#qmd74_finding_wrapped_comment]]`) describe this pair;
they are carried here for the fix.

**A declared array or map above a deeper declaration.** `## Closure [[clo: array]]` with `- one` /
`- two`, then `### Task [[t5: SessionTask]]` with `- status: x`:

| parser | result |
|---|---|
| rs | `clo = ["one", "two", "status: x"]` — the declared object is gone, its field became a string element of the array |
| py, ts | `clo = ["one", "two"]` plus the object `s.t5` |

With `[[clo: map]]` the same shape gives rs one `invalid_map_content` at the declaration's line and the
entry inside the map; py and ts give TWO at the offending line and a clean map.

**An indented sub-item whose text looks like a field.** `- issues:` followed by an indented
`- AMBIGUOUS: text`, with `- status: ok` after it. All three lose the parent field `issues`. Rust
additionally promotes the sub-item to a real field of the parent object, keyed by whatever text
precedes the colon, and reports nothing; a further sub-item without a colon becomes comment content
anchored on that invented field. Python and TypeScript drop the sub-item and report
`nested_subitems`. A field written after the sub-item survives in all three, so the loss is the
parent field plus, in Rust, a field that the author never wrote. Re-measured on the corpus shape
(`sim/runs/2026-05-17T12-39-34/s05_file-upload/trace.qmd.md`, four fields then the sub-item): Rust
emits `AMBIGUOUS` with the sentence as its value and zero errors. QMD-71 left this open on purpose,
having found that suppressing the field for bullet-nested items traded one divergence for another.
It is the whole remaining error gap on the external corpus: 125 `nested_subitems` in Rust against
148 in the other two.

- category: parser
- related_to: [[#qmd77_tails]]
- solution: Each needs its rule stated before it is coded; three of the four are questions Q1-Q3.

## Content: two shapes where a round trip is not lossless [[qmd77_finding_content: Finding]]

**Content above the first heading.** A document opening with prose, `---`, a table and a fence before
`# A [[a: Thing]]`. The synthesised `text_0`:

| parser | content |
|---|---|
| rs | the fence only |
| py, ts | prose + table + fence |
| all three | the `---` is dropped |

**A wrapped list item in comment content.** A `## Checklist` heading (declaring nothing) with
`- [ ] wrapped item that continues` and an indented continuation line. Rust and Python keep the two
leading spaces; TypeScript strips them, so rewriting the document through TypeScript changes it. Found
when the QMD-61 checklist gained wrapped bullets and `make validate-compare` reported the document
divergent as `rs+py vs ts`; the bullets were straightened, so the corpus is exact again and the defect
untouched.

**A third shape of the same family, found while writing this file.** A wrapped bullet in the body of a
Finding object — prose, then `- **Name.** text` continuing on an indented line — diverges with a
DIFFERENT grouping again: Rust joins the lines into one, Python and TypeScript keep the wrap. So the
wrapped-line family has at least three shapes and all three groupings appear across them:

| shape | grouping | who differs |
|---|---|---|
| a field value | three-way | all three |
| a list item in text-block content | `rs+py` vs `ts` | ts strips the indent |
| a list item in an object's body | `py+ts` vs `rs` | rs joins the lines |

That matters for the fix: whichever answer Q1 gets has to be applied to every carrier of a wrapped
line, not to the field-value path alone, and a fixture is needed per carrier. Found the same way as the
other two — this file's own bullets wrapped, and `make validate-compare` reported it divergent.

- category: parser
- related_to: [[#qmd77_tails]]
- solution: Q4 decides the first. The second is a TypeScript defect with no question attached.

## Contract: six differences in what the output says [[qmd77_finding_contract: Finding]]

1. **System objects on one file.** `.kiro/steering/qmd-guide.qmd.md` in the external corpus is
   entirely YAML front matter. Rust synthesises neither `__Document` nor `__TextBlock` for it; the
   other two synthesise both — 2 system objects against 4. The authored objects are identical.
2. **A synthesised id repeats.** That file and an unrelated one both get `doc_ry4ljv`, and both get
   `text_0`. No `__global_id`, so unreferenceable, and no diagnostic.
3. **Two names for one thing.** `workspace parse` errors carry `object`; `workspace validate` errors
   carry `objectId` plus `fieldName`. All three parsers agree within each command.
   `docs/parsers/commands.qmd.md:270` documents validate's shape and is correct; parse's is undocumented.
4. **`mixed_field_keys` line.** On a list with one valid field and one invalid key, Rust points at the
   invalid item; Python and TypeScript point at the item above it, which has no colon at all. Rust
   looks right.
5. **`__has_explicit_id` on an array element.** An object-array element with no `[[id]]` carries
   `__has_explicit_id: false` in Rust and nothing in the other two.
6. **The parse envelope is unpinned.** QMD-75 gave Rust the `index` key and renamed TypeScript's
   `byGlobalId` to `by_global_id`; no fixture under `tests/workspace` mentions `index` or those keys,
   nor Python's `__positions` on the first of two duplicates.

- category: parser
- related_to: [[#qmd77_tails]]
- solution: 1, 4, 5, 6 have one right answer each and need no question. 2 and 3 are proposals in the questions finding.

## Robustness: two failures outside the parser proper [[qmd77_finding_robustness: Finding]]

**An unreadable directory.** A workspace holding `readme.qmd.md`, `top.qmd.md` and a `chmod 000`
subdirectory with a file in it:

```text
rs  -> {"files": ["readme.qmd.md", "top.qmd.md"]}
py  -> {"files": ["readme.qmd.md", "top.qmd.md"]}
ts  -> Error: EACCES: permission denied, scandir '.../locked'
git -> warning: could not open directory 'locked/' + both files, exit 0
```

TypeScript returns nothing at all, not even the files it could read. `scanWorkspace` calls `readdirSync`
with no `try`/`catch`; the same file already has the pattern it needs, in the QMD-72 bounded scan a few
hundred lines below. Three lines. Pre-existing, confirmed by running `HEAD`'s scanner during QMD-73's
review with a stub in place of the dependency that task removed.

**Path bytes that are not valid UTF-8.** Of eight byte-level ignore cases, one diverges: Python decodes
with `surrogateescape`, keeps the bytes and matches; TypeScript (`TextEncoder`) and Rust
(`to_string_lossy`) substitute U+FFFD and do not. APFS refuses such a filename (`OSError 92`), so the
case is unreachable on this machine and real on Linux ext4. Closing it therefore needs a Linux run,
which no current test performs — that, not the fix, is the work.

- category: parser
- related_to: [[#qmd77_tails]]
- solution: The first is three lines. The second needs a Linux job before it can be verified at all.

## Fidelity: Rust rewrites the markup in a field value [[qmd77_finding_fidelity: Finding]]

Filed on the public tracker as [issue 12](https://github.com/mikilabs/qmdc/issues/12), open since
2026-09-28 and re-measured live on this build. A list-item field value is returned by Rust rebuilt
from its Markdown events rather than sliced from the source, so every inline construct whose canonical
form differs from the written one comes back changed, and a construct with no arm in the match comes
back missing. One field:

```text
written  - summary: guard __main__, name _x_, tag <b>bold</b>, star *y*, tick `z`
rs    -> guard **main**, name *x*, tag bold, star *y*, tick `z`
py    -> guard __main__, name _x_, tag <b>bold</b>, star *y*, tick `z`
ts    -> guard __main__, name _x_, tag <b>bold</b>, star *y*, tick `z`
```

The issue measured nine constructs that change: `__x__`, `_x_`, inline HTML (dropped), `~s~`, escaped
`\_e\_`, `&amp;`, an autolink, a titled link and an image. `**y**`, `*y*`, `~~d~~` and code spans come
back unchanged only because the rebuilt form happens to equal the written one. The consequence is not
cosmetic: a value read through Rust and written back alters the document, which is the round-trip
guarantee the format sells.

The `Event::Code` arm in the same match already takes `&markdown[range.start..range.end]`, so the
mechanism is present — the fix is to take the source slice for the whole value instead of extending the
rebuild construct by construct. Same shape as the QMD-71 ordered-list fix, where a correct raw-slice
branch existed and a rebuild path was deleted.

- category: parser
- related_to: [[#qmd77_tails]]
- solution: Slice the source for the whole value; pin it with the tracker's own CLI fixture.

## Open questions [[qmd77_finding_questions: Finding]]

Four decisions belong to the operator, because in each the three parsers disagree and no document in
the repository states which is right. Nothing should be coded on these until they are answered — that
is exactly how QMD-75's first rule came to be written too narrow. Two are now ANSWERED (2026-09-30):
Q1 and Q4 below carry the decision; Q2 and Q3 are still open, the operator having asked for a worked
example of each before deciding.

**Q1. What does a wrapped field value mean? ANSWERED: a parsing error, in all three.** `- note: first
line` with an indented continuation. The three current readings — space-join (Rust), newline-join
(Python), first line only (TypeScript) — are each a silent rewrite of the author's text, and "a value
is written on one line" is a rule a reader can hold. Every `.qmd.md` in this repository is affected,
and both defects that exposed it were found because a tracking file of ours wrapped a value. The error
must name the continuation's line, and the rule has to hold in every carrier of a wrapped item, not
only in a field value: `[[#qmd77_goal_b2]]` measured three distinct groupings across the carriers
(field value three-way, text block `rs+py` against `ts`, object body `rs` against `py+ts`).

**Q2. Does a declared `array` or `map` swallow a deeper declaration? ANSWERED: it swallows, and the
mix is reported.** QMD-75 settled that `[[id: text]]` swallows everything below it, deeper declarations
included, because the declared kind decides; `array` and `map` do the same, so the object does not
survive — but the loss is never silent, because mixing an element of another shape into a declared
collection is a type mix and the format already has codes for it. The operator's words: "это и есть
`invalid_map_content`, потому что смешиваются типы в массиве".

The vocabulary needs no new code. `[[clo: map]]` already raises `invalid_map_content`
(`[[#err_invalid_map_content]]`); `[[clo: array]]` raises `mixed_array` (`[[#err_mixed_array]]`), whose
declared cause is this exact consequence — "a following element heading cannot join it, so it silently
becomes a plain field on the parent and its declared Kind is lost". Today that code fires only for a
table-fed object array, so its scope widens to any declared collection meeting a declaring heading.
Two contract details go with it, decided here because they are counting rather than syntax: ONE error
per offending element, and at the ELEMENT's own line, not the declaration's. Rust currently reports
one `invalid_map_content` at the declaration's line; Python and TypeScript report two at the element's
line for a single offender. Both are wrong, in different ways.

**Q3. Can an indented sub-item ever be a field? ANSWERED: never — they are forbidden.** A sub-item is
always content and the parent always reports `nested_subitems` (`[[#err_nested_subitems]]`), which is
what Python and TypeScript already do and what `docs/format/fields.qmd.md` already states: a nested
list under a field key is forbidden. Rust's reading — a sub-item whose text parses as
`key: value` becomes a field of the parent — is the behaviour to remove, along with its silence. The
operator's words: "это и есть `nested_subitems`, так и есть, они запрещены". This closes the whole
remaining error gap on the external corpus, 125 against 148, since every one of those 23 is this shape.

**Q4. What belongs in the text block above the first heading, and does a `---` survive anywhere?
ANSWERED: every block belongs there, and `---` is preserved everywhere.** Rust keeps only a fence
there, the other two keep prose and tables as well, and all three drop a thematic break. The second
half was the larger question: a `---` inside an object's body IS preserved as a comment (QMD-75 pinned
that), so dropping it at the top level was inconsistent with the rest of the format. So the text block
takes prose, tables, fences and thematic breaks alike, and a round trip through any of the three
reproduces the source.

Two more decisions are contract rather than syntax, and are proposed here rather than asked:

- **Synthesised ids.** Make `doc_*` and `text_*` unique within a parse result by deriving them from the file path, keeping them out of `__global_id` as now. The alternative — leaving them file-local — keeps a graph that holds two objects under one id.
- **Error key names.** Give `workspace parse` the same `objectId` / `fieldName` as `workspace validate`, and document both in `docs/parsers/commands.qmd.md`. It breaks a consumer reading `object`, and one name for one thing is worth that once.

- category: parser
- related_to: [[#qmd77_tails]]
- solution: All four answered 2026-09-30; the two proposals stand unless overridden.

## The synthesised id collision costs objects, and the id does resolve [[qmd77_finding_synth_ids: Finding]]

Measured on the current build with all three parsers, which agree on every number below, so this is a
format defect and not a parity one.

`doc_<x>` is not occasionally colliding — it is a CONSTANT. The LCG is seeded per parse (default seed
666), so every document in every workspace gets the literal id `doc_ry4ljv`. `text_<n>` counts from
zero within a file, so every file with content above its first heading holds `text_0`.

The graph keys on `__global_id`, a generated column computed as `<workspace>::<id>`, so the collision
is a key collision and the last file read wins. Two files, each with a fence above its first heading:

| | `workspace parse` | `query ... FROM objects` |
|---|---|---|
| objects | both files' `doc_ry4ljv` + `text_0` | one of each, from `specs/beta.qmd.md` only |

On our own `docs/`: `workspace parse` returns 55 `__Document` and 70 `__TextBlock` under 8 distinct
ids, and the query layer sees 4 and 10. So 111 of 125 synthesised objects are dropped, and which
survivor remains depends on file order. The containment graph collapses with them — one `doc_ry4ljv`
node owns `content` edges to objects from every file at once.

A reference DOES resolve. `- points_to: [[#doc_ry4ljv]]` and `- text_to: [[#text_0]]` validate clean
in all three and produce edges, pointing at whichever document won the merge rather than at the one
the reference was written beside. The external corpus is unaffected for a reason worth keeping: every
file in it opens with a heading, so it synthesises neither object.

25 parser fixtures plus five workspace, sql and lsp fixtures pin these literal ids, and the format
docs never state their shape, so the id scheme is unpinned by the spec and free to choose.

Nothing authored references them. Zero `[[#doc_...]]` or `[[#text_...]]` in our own documents or in
the external corpus, and no product code consumes them either — the mkdocs plugin, the extension and
the semantic chunker all read a `__kind`, never one of these ids. One thing does persist them: the
semantic index stores the global id as its `object_id` (`docs:architecture:text_0` and three more in
the live index), but it is a derived artifact rebuilt by `make semantic-index`, so a scheme change
costs a reindex rather than compatibility.

The collision is per NAMESPACE, not per workspace, which is why it survived releases. The graph key is
`<workspace>:<namespace>:<id>`, so the 125 synthesised objects in `docs/` fall under 14 distinct
(namespace, id) pairs and only 6 of those pairs are shared by more than one file. Almost all of the
damage sits in one namespace: `tracking` holds 52 files that all carry `doc_ry4ljv` and `text_0`. A
repository with one document per namespace never sees any of this.

What each id scheme is actually worth, measured on 368 real files across both corpora:

| scheme | uniqueness | length, median / max |
|---|---|---|
| path hash, 6 chars | probabilistic — 2.3% at 10k documents | 6 |
| path hash, 8+ chars | probabilistic — 1.2% at 10k, the seed is 32 bits | 8 |
| file basename | COLLIDES for real: `readme` in three of our files, `operations` and `schema` once per module in the corpus | 11 / 22 |
| path slug relative to the namespace root | guaranteed — 0 collisions on 368 files | 16 / 90 |
| full relative path slug | guaranteed — 0 collisions on 368 files | 46 / 90 |

Widening the printed id past about 8 characters buys nothing on its own: the LCG state is masked to
32 bits so that all three parsers agree on `doc_ry4ljv` for seed 666, and that mask is the ceiling on
how many distinct ids any seed can produce. Measured on a replica of that LCG: 200 000 distinct seeds
give 15 colliding ids at 6 characters and none at 12, so from roughly 8 characters the id is
injective in the seed and the hash width alone decides.

- category: parser
- related_to: [[#qmd77_goal_c2]]
- solution: The collision is total, not occasional; it costs 111 of 125 objects in our own docs and the ids are addressable.

## How this one has to be verified [[qmd77_finding_tests: Finding]]

Every item on this list survived at least one release, and each survived for the same reason: the
parity corpus is `docs/`, whose documents are written by us and therefore avoid every shape that
breaks. A fix verified only against `docs/` proves nothing about these.

So each item needs, in this order:

1. A data-driven fixture under `tests/parser` or `tests/workspace`, checked to FAIL on the previous
   commit's binaries — built and run, not reasoned about from the diff. For a shape where all three
   parsers currently agree on the wrong answer, the fixture must fail in all three.
2. The 48-case declaration matrix (`reviews/qmd75_matrix.py`) still at 48/48, and the matrix extended
   where this task's rule adds a dimension.
3. The full `make test` green: unified report, three-way validation-error parity, exact parse parity
   across the whole corpus, markdownlint, and `make lint` for clippy and the type checks.
4. The external corpus re-measured — authored object count, distinct global id count and error count
   per parser, on a corpus an order of magnitude larger than `docs/` and not written by us.
5. For `[[#qmd77_goal_d2]]` only: a Linux run, because the divergence is unreachable on APFS. Without
   one that goal cannot be closed and should be split out rather than claimed.

- category: parser
- related_to: [[#qmd77_tails]]
- solution: A goal is not done until its fixture has been seen red on the previous commit and green on this one.
