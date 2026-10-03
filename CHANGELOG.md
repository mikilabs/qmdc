# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
The whole system is released under a single version following
[Semantic Versioning](https://semver.org/spec/v2.0.0.html); a release ships all
packages (`qmdc`, `qmdc-semantic`, `qmdc-mkdocs`, `qmdc-vscode`) together. This
file is maintained by hand.

## [Unreleased]

### Added

- **`qmdc mcp -w` / `--with`: serve an explicit composition.** `qmdc mcp -w <A> -w <B>` answers
  every tool and resource over the composition of exactly those workspaces — the graph
  `qmdc query -w` sees — wherever they are on disk. A tool's `path` must lie inside one of them
  (`out-of-root` otherwise); the base is virtual as in `workspace parse -w` (`__file` under each
  workspace's id, `root: null` in `qmdc_dump_index`). The set is checked at startup with the CLI's
  rules and a refused set exits 2: zero or several workspaces behind one path, a path twice, a
  workspace id twice, and with `--force-root` any `-w` path or workspace outside the boundary
  ([#10](https://github.com/mikilabs/qmdc/issues/10)).

### Fixed

- **`qmdc parse` exits 1 when the result holds a `__ParsingError`**, in all three parsers, and says
  so on stderr. The JSON is still written in full. It used to exit 0, so the syntax check the agent
  guide teaches (`qmdc parse -i f.qmd.md > /dev/null || exit 1`) passed on a document that had lost
  a value ([#11](https://github.com/mikilabs/qmdc/issues/11)).
- **The LSP reloads `.qmdcignore`.** A change to any `.qmdcignore` now rescans the workspaces, from
  both the editor's file events and the server's own watcher, and the VS Code extension watches
  `**/.qmdcignore`; before, the ignore rules read at start-up held until a restart
  ([#6](https://github.com/mikilabs/qmdc/issues/6)).
- **Explorer items in a multi-root window open the right file.** Every workspace in
  `qmdc.getWorkspaceTree` carries `projectRoot` even when its id is declared in several places
  (the lookup was by id alone and gave up on a duplicate), and the extension no longer falls back
  to the first editor folder when it is missing — the guess that opened a nonexistent file
  ([#7](https://github.com/mikilabs/qmdc/issues/7)).

## [2.0.0] - 2026-10-01

Major release: four breaking changes below (`workspaces` envelope, `objectId`/`fieldName`
error keys, path-derived synthesised ids, no Kind segment in references). Every package
ships as 2.0.0: `qmdc` (crates, PyPI, npm), `qmdc-vscode`, `qmdc-mkdocs` and
`qmdc-semantic`, the last two now requiring `qmdc>=2.0.0`.

### Added

- New `wrapped_field_value` parsing error: a field value — or an element of a `[[field: array]]` list —
  continued on an indented second line. A value is written on ONE line. The continuation used to be
  read three ways (Rust joined the lines with a space, Python with a newline, TypeScript kept the
  first line and dropped the rest in silence), so the same document carried three different values;
  the value is now the authored first line everywhere and the continuation is reported. The two legal
  multiline forms are untouched, because each announces itself on the first line: YAML pipe
  (`- key: |`) and a YAML array whose bracket opens the value (`- key: [`) (QMD-77).

- References can be qualified with a workspace: `[[#workspace:namespace:id.field]]`, a
  right-aligned suffix of the object's global id. `[[#ws::id]]` elides the namespace and matches
  any namespace of that workspace. A reference that crosses a workspace boundary MUST now be
  qualified; an ambiguous qualified reference produces no edge rather than an arbitrary one
  (QMD-69).
- New `table_in_array` parsing error: a Markdown table under a primitive array field
  (`[[field: array]]`) is now reported instead of being silently dropped. A primitive array holds
  scalars and a table has columns, so there is no defined mapping; the table content is preserved in
  `__comments` for a lossless round trip, as with `ordered_list_in_array`. Tables remain valid under
  an object array (`[[field: [Kind]]]`), where one row becomes one object. Surfaced by the CLI, the
  LSP and MCP (QMD-70).

- New `extra_table_in_array` parsing error: only the FIRST Markdown table under an object-array
  heading feeds the array. A second one cannot extend it — its rows would collide on the generated
  positional ids — so it is now reported instead of silently becoming the container's prose. This also
  removes a construct that could not be rebuilt faithfully under any anchor (QMD-70).

- New `block_in_inline_field` parsing error: an indented table, list, paragraph, quote or fence under
  an inline field that already has a value. An inline field holds a scalar and has no content of its
  own, so the block belonged to nothing — and each parser mangled it differently: Rust glued prose
  and quote text into the field's VALUE and lost the field entirely for an indented list, while
  Python and TypeScript reduced every block to bullet items. The content is now preserved in
  `__comments`, dedented and anchored on the field. YAML multiline (`- key: |`) is unaffected — there
  the indented block IS the value (QMD-70).
- New `mixed_array` parsing error: an object array is written as a table OR as subheadings, not both.
  A heading element after the array's table used to leave the array silently — it became a plain field
  on the parent and its declared Kind was degraded to `__Object` — with nothing reported. A table
  AFTER an element heading is still that element's own content (QMD-70).

- `make validate-compare` now also compares the FULL `parse` output of the three parsers over the
  real `docs/` corpus, not just their validation-error lists. The first run found that 32 of 108
  documents parse differently between parsers — a surface the old comparison could not see, and the
  reason two divergences in this release were found by review rather than by CI. It runs as a ratchet
  against `scripts/parse-parity-baseline.json`: new divergences fail, known ones may only shrink, and
  `make parse-parity-baseline` re-records the count (QMD-70).
- New `unsupported_number_format` parsing error: a field value that looks like a number QMD.md cannot
  carry. Two groups — a spelling the format does not define (`1e5`, `1.5e-3`, `2E3`, `.5`, `5.`, `+1`,
  `1_000`, `0x1f`, `0o17`) and a supported spelling outside the representable range (above 2^53-1 an
  integer cannot survive a round trip through a double; below `1e-4` a decimal can only be written with
  an exponent, which the grammar has no form for). The field keeps the text the author wrote, as a
  String, so nothing is lost and the document still round-trips; quoting is the escape hatch
  (`- max: "9223372036854775807"` raises nothing). The error carries a `hint` field naming the way
  out for its group — rewrite the spelling, or quote it when the magnitude leaves no other option —
  so the reader is told what to write and not only what is wrong; it is the only error that carries
  one. Previously every one of these became a String in
  silence, and which of them did so differed per parser (QMD-71).
- **`-w` / `--with`: compose workspaces at unrelated paths.** `query`, `workspace parse`,
  `workspace validate` and `workspace files` (the last in Python and TypeScript only — the Rust CLI
  has no `files` subcommand) take one or more `-w` paths, each naming a workspace
  anywhere on disk, composed in place. Every path is a peer — the first is not primary — and a
  single `-w` is valid. Discovery answers "what is near this path"; composition answers "which
  workspaces make up this project", and the second is a property of the project rather than of one
  developer's disk, so it has to be supplied by the caller. The positional form is unchanged and
  mutually exclusive with `-w`. Five invocations are refused as usage errors with **exit 2** — a
  positional path together with `-w`, a `-w` path resolving to zero workspaces, one resolving to
  several, the same path twice, and two paths declaring the same workspace id (which no composed
  graph can represent, since every id in one would collide with the other's) (QMD-72).

### Changed

- **Breaking:** the Kind segment is gone from the reference grammar. A reference is
  `[[#workspace:namespace:id.field]]` — the same segments as `__global_id` — and a segment before the
  id is always a qualifier, never a Kind. `[[#Table:users]]` used to resolve to `users` by Kind; it now
  reads `Table` as a namespace and reports `broken_link` in all three parsers, and the mkdocs plugin
  renders it as a broken link. `[[#namespace:Kind:id]]` likewise no longer means what the agent guide
  described. Kind never took part in identity: the graph ignored it, so `[[#storage:Table:users]]`
  could get an edge to an `Entity` with the same id, and any Kind segment — even one naming no Kind —
  switched off the ambiguity check. In `parse --format full` such a reference's `type` in
  `__references` changes from `"kind"` to `"namespace"`; the value `"kind"` no longer occurs. To migrate, drop the Kind segment: `[[#Table:users]]` → `[[#users]]` (QMD-69).
- **Breaking:** a synthesised `__Document` / `__TextBlock` id is now derived from the file's path
  instead of being the same literal in every document. Every document used to get `doc_ry4ljv` (an
  LCG reseeded per parse) and every file's first text block `text_0`, and the graph keys on
  `<workspace>:<namespace>:<id>` — so files sharing a namespace collided on that key and the last one
  read won. In our own `docs/` the query layer returned 14 of the 125 synthesised objects; it now
  returns every one, and the sample workspace's edge count rose from 74 to 79 as the containment edges
  came back with them. The id is `doc_<stem>` / `text_<stem>_<n>`, where the stem is the path relative
  to the directory that declared the namespace (`format/deep/commands.qmd.md` under namespace `format`
  becomes `doc_deep_commands`). The stem folds case and punctuation, so `a-b`, `a_b` and `a/b` share
  one, and a name without an ASCII letter or digit folds to nothing (`file`): the files of a namespace
  are taken in path byte order and a later file whose ids are already taken — by another file or by
  an id an author wrote in that namespace — gets the next free `_1`, `_2`, … suffix. A single-file
  `parse` cannot collide with itself and keeps the counter form. Anything storing these ids — the
  semantic index does — must be rebuilt (QMD-77 C2, review round 1).
- **Breaking:** `workspace parse` names an error's owning object `objectId` and adds `fieldName` —
  the same keys `workspace validate` already used. The envelope's own spellings `object` and `field`
  are gone, so one error reads the same whichever command produced it. A consumer reading `object`
  from `workspace parse` must be updated (QMD-77 C3).
- A declaration inside a declared collection no longer creates an object: `[[field: array]]` and
  `[[field: map]]` decide what their section is, the rule QMD-75 settled for `text`. Such a heading
  closes the collection's content, exactly as prose between two lists does, and is reported once at
  its own line — `mixed_array` for an array, `invalid_map_content` for a map. Rust used to swallow it
  into the collection as a value; Python and TypeScript used to create the object (QMD-77 A3).
- A map is populated only from its FIRST bullet list, as the spec said all along: an additional list
  in the section is reported once instead of being merged (Rust) or reported twice (Python,
  TypeScript). A list item that is not a valid `key: value` pair is `invalid_map_entry` in all three,
  including the first item of the list. An indented sub-item under an entry is `nested_subitems` when
  the entry has no value and `block_in_inline_field` when it has one, instead of becoming an entry the
  author never wrote (QMD-77 A3).
- **Breaking:** a `__TextBlock`'s `content` is now the verbatim source of its region — from its
  first line to the line before the heading that ends it, blank lines at either end dropped — instead
  of text each parser rebuilt from Markdown tokens. The rebuild lost constructs, differently in each
  parser: Rust dropped a blockquote, joined the two lines of a setext heading (`## alphabeta`) and
  stripped the markup from an unnamed heading; Python and TypeScript dropped the `>` of a quote, the
  bullets of a list and the closing `#`s of a heading, and turned a setext heading into `## alpha` +
  `beta`, which re-parses as a heading and a paragraph; all three dropped an HTML block and an HTML
  comment inside a block, and rewrote a `~~~` fence with backticks. Any content above the first
  heading now opens the leading block, except HTML comments alone, which are still ignored: a table,
  a `---` or an HTML block opened none in any parser, a list or a blockquote none in Rust, and an
  indented code block none in Python and TypeScript. `__code_fences` offsets are counted against the
  new content, and only fenced code is a code fence (Rust reported an indented code block as one).
  No pinned output changed: every text block in `docs/` and in the parser fixtures already equalled
  its source region (QMD-77 B1, review round 1).
- Rust no longer treats a `---` or `+++` fence at the top of a file as front matter — QMD.md
  defines none, so it is ordinary Markdown, and a file holding nothing else now yields a `__Document`
  and `__TextBlock`s instead of nothing (QMD-77 C1).
- A duplicate id keeps both objects in document order with one error each, in all three. TypeScript
  used to drop the first colliding NESTED object and report nothing; Rust ordered the duplicated
  subtrees by an (id, label) look-up that collides when both match (QMD-77 A1).
- An object-array element written as a bare `### Alice` carries `__has_explicit_id: false` in all
  three, as the spec requires for an auto-generated id — without it a rebuild printed an id the author
  never wrote (QMD-77 C4).

- **A heading's declaration decides what it is; the content below it no longer does.** A `[[id]]`
  heading whose body was a paragraph, a `---`, a table or a code fence used to be read three
  different ways: Rust kept the object and nested a deeper declaration under it, Python dropped the
  object **and its whole subtree**, and TypeScript dropped the object and **re-parented** the
  declaration onto the grandparent — so the same file produced different `__id` values in each, and a
  reference that resolved in one was broken in another. One rule now decides in all three: a bare
  `[[id]]` is an object when, before the next heading at its own level or shallower, there is either
  a field list of its own or a deeper heading that DECLARES an identifier; otherwise it is an
  implicit text field whose value runs to that same boundary. "Declares" is the definition syntax, so
  brackets written inside a code span, and a reference such as `### See [[#other]]`, no longer count —
  a reference used to make the heading above it an object AND give it a field literally named
  `#other`.
  `[[id: text]]` swallows everything below it, deeper declarations included, as Rust and Python
  already did. Non-field content is preserved in `__comments` whether or not the heading spelled a
  Kind — it used to survive under `[[id: Kind]]` and be dropped under `[[id]]`.

  **Migration.** A document whose objects only ever carried field bodies is unaffected. Otherwise all
  three parsers can change: where a body sat between a heading and its child, Python and TypeScript
  GAIN the object, and in TypeScript an existing id CHANGES (`s.t5` → `s.clo.t5`), which breaks a
  `[[#s.t5]]` reference written against the old reading; where a body sat in front of a heading's own
  field list, Python and TypeScript gain the object and Rust keeps the one it had; and a table
  directly under a bare `[[id]]` is a text field in all three rather than a `table`-tagged string
  (QMD-75).
- **`workspace parse` carries `index` in all three parsers**, and its sub-key is `by_global_id`
  everywhere — Rust omitted the key entirely and TypeScript spelled it `byGlobalId`, so a consumer
  written against one parser read a missing key from another. `workspace parse`'s own `errors` block
  now carries the same keys in all three as well. Note that it is not the same shape as
  `workspace validate`'s: parse names the owning object `object`, validate names it `objectId` and
  adds `fieldName`. That difference is older than this task and unchanged by it (QMD-75).
- **`nested_subitems` is reported on an element of an object array too.** Rust already reported it on
  a file's top-level object and on a plain subobject, but the check was switched off for the whole
  subtree of an object array, so on one real repository 148 array elements were 148 errors in Python
  and TypeScript and none in Rust (QMD-75).
- **A list whose items carry no valid key at all is prose, not a mixed field list.** Python reported
  `mixed_field_keys` for it; the error means "some items have valid keys, some do not", so a list
  with none was never a mixed case. Rust and TypeScript already read it as prose (QMD-75).

- **`workspace parse` output has one shape.** It always carries `workspaces`, a list of
  `{id, root, path}` entries — `root` where the workspace is on disk, `path` where its files sit in
  `__file` — replacing the three shapes a consumer had to tell apart: `"workspace": "<id>"` for one
  workspace, `"workspaces": ["<id>", ...]` for several, and `"workspace": null` for none. The
  top-level `root` is now a canonical absolute path, or `null` when the base is virtual (see the next
  entry). A consumer that read the `workspace` key reads `workspaces[0].id` instead (QMD-72).
- **Under `-w`, `__file` starts with the workspace id**, e.g. `shop/storage/tables.qmd.md`. It was
  relative to the common ancestor of the `-w` paths, which for checkouts in unrelated places is `/`,
  so the host's own directory names ended up in the graph and the same repositories gave different
  `__file` values on different machines. The positional forms are unchanged (QMD-72).
- **`nested_workspace` is no longer reported for a workspace that is composed alongside the one it
  sits in** — `-w repo -w repo/.qmdc`, or a directory holding both. Its files are then in the result
  under its own workspace, so nothing is missing, and the report contradicted the composition the
  caller asked for. It is still reported when the inner workspace is left out, which is what it is
  for (QMD-72).
- **The `nested_workspace` report names the consequence and the remedy** instead of saying
  "Workspaces cannot be nested", which is not what the rule means: nesting is reported because the
  outer scan leaves the inner workspace's files out of the graph, and the container form composes
  both without complaint. The message now reads `Nested workspace '<id>' inside this workspace: its
  files are excluded from this graph. Validate both together: --with <root> --with <root>/<dir>`. The
  old wording sent a user hunting for a defect in the layout `qmdc-model init` itself creates — a
  repository whose root is a workspace and which also holds a `.qmdc` model workspace — where the
  answer was to compose the two (QMD-76).
- **MCP now composes a container of sibling workspaces instead of refusing it.** Given a
  non-workspace directory holding several workspaces, MCP reference validation and the other
  path-taking tools previously answered `ambiguous` with the candidates and asked the caller to pick
  one — while the CLI composed the same directory. Neither candidate alone can see the other's
  objects, so picking one could not answer the question either; that split was the live half of
  upstream issue 9. The `ambiguous` code survives with a sharper meaning: a set that cannot be
  composed because two members declare the same workspace id. A caller that relied on receiving
  `candidates` for a plain multi-workspace container will now receive a composed answer (QMD-72).
- **Four constructs that previously parsed silently are now validation errors**, so
  `qmdc workspace validate` can newly exit non-zero on a document that passed before:
  `table_in_array`, `extra_table_in_array`, `mixed_array` and `block_in_inline_field`. Each was
  measured to LOSE or MANGLE data before — a row dropped, a declared `Kind` degraded to `__Object`,
  a table reduced to its cell texts, or prose concatenated into a field's value — and each was
  silent on every surface, so no document could have depended on the old reading for correct output.
- **The numeric grammar is now `-?\d+(\.\d+)?` with a magnitude between `1e-4` and 2^53-1**, and
  anything else is a String plus an `unsupported_number_format` error. This narrows the documented
  format: `docs/format/types.qmd.md` previously promised "scientific notation" with `1.5e10` as an
  example, a promise only the Rust parser kept — Python accepted `1.5e-3` but not `1e5`, and TypeScript
  accepted neither. Exponent notation appears in no field value anywhere in the repository outside the
  fixture written for this decision, so nothing in tree relied on it. The upper bound also removes a
  class of divergence rather than reconciling it: beyond 2^53-1 the three JSON writers disagree on the
  SPELLING of the same double (QMD-71).
- A bare `~` in a field value is a String, not null. That spelling belongs to YAML, and QMD.md is not
  YAML; Rust was the only parser reading it as null, pinned there by its own unit test (QMD-71).
  The offending content is now preserved in `__comments` in every case. None of the four shapes
  occurs anywhere in this repository outside the fixtures written for them (QMD-70).
- **`.qmdcignore` now follows git's `.gitignore` rules exactly, in all three parsers.** Each
  parser used to hand a line to a different glob engine, so one ignore file hid different files
  in each, and none matched git. The matcher is now a port of git's own, pinned against git's
  answers on 79 line forms (`tests/ignore/`). An ignore file written against the old matching may
  hide a different set of files now, so re-check the `.qmdcignore` of any existing workspace.
  Lines that change meaning (QMD-73):
  - `dir/*` and `docs/*.qmd.md`: a star no longer crosses `/` in Rust and Python, so
    `docs/*.qmd.md` stops hiding `docs/sub/`. TypeScript's `dir/*` now hides the whole tree below
    `dir/` by hiding its subdirectories, as git does.
  - A bare name or plain path (`generated`, `tests/sub`) now hides a directory of that name and
    everything in it, in all three.
  - A line with no slash, or only a trailing one, now matches at any depth in all three:
    `build/` hides `src/build/`. Python already did this for file names.
  - A leading `/` now anchors the line; before, it made the line match nothing.
  - `!` now re-includes a path. In TypeScript a line starting with `!` used to hide every file
    it did not name, the workspace's own `readme.qmd.md` included; in Rust and Python it did
    nothing.
  - Only trailing spaces are trimmed, as in git; leading spaces and tabs are part of the line.
- **A `--with` path's own `.qmdcignore` now steers the search for its workspace in all three
  parsers.** Only Rust read it there, so `-w` on a directory that ignores one of its two
  workspaces composed in Rust and was refused in Python and TypeScript (QMD-73).

### Fixed

- The npm package `@qmdc/qmdc` can be imported as a library. Up to 1.0.6 it shipped only the
  TypeScript sources, with no `main` or `exports`, so `import { parse } from '@qmdc/qmdc'` from the
  README failed with `ERR_MODULE_NOT_FOUND`; only the `qmdc` command worked. The package now builds
  `dist/` with type declarations on pack, and `exports` exposes one entry, `src/index.ts`, which
  mirrors Python's `__all__` plus the query API (`QmdcDatabase`, `executeQuery`). Python's root
  now exports the query API too (`QmdcDatabase`, `execute_query`). New `make package-e2e` builds
  each library the way it is published (npm tarball, wheel, `.crate`), installs it into a clean
  directory and runs the parser, workspace and SQL conformance harnesses against it through the
  package root only, plus every code block of its README. The Python and TypeScript READMEs
  showed a workspace API that does not exist (`query_workspace`, `get_refs_to`, a
  `@qmdc/qmdc/workspace` subpath); they now show the real one.
- All three parsers read a workspace's files in one order: directory, then `readme.qmd.md`, then
  file name, each compared by UTF-8 bytes. TypeScript compared names with locale collation, which
  ignores case and punctuation, so when two files declaring the same id were named `B.qmd.md` and
  `a_b.qmd.md`, or `a-b.qmd.md` and `a_b.qmd.md`, it reported a different occurrence as the
  `duplicate_id` and its query layer kept a different object than Rust and Python did. Files outside
  every workspace, which `query` also loads, were read in directory-listing order, so the object
  kept there could differ between parsers and between machines; they are now read in the same order
  as a workspace's files (QMD-77).
- A field value is now the source text as written, in Rust as it already was in Python and
  TypeScript. Rust rebuilt a list-item value from its Markdown events, so it returned whatever form
  the renderer prefers rather than the author's: `__main__` came back as `**main**`, `_x_` as `*x*`,
  `~s~` as `~~s~~`, an escape lost its backslash, `&amp;` came back decoded, an autolink and a titled
  link were rewritten, an image was reduced to its alt text, and inline HTML was dropped outright —
  `<br>` vanished. A value read through Rust and written back therefore altered the document, which
  is exactly the round trip the format guarantees. The inline arms now take the source slice, as the
  `Event::Code` arm already did, so every construct is covered at once instead of one arm per
  construct. `docs/format/fields.qmd.md` states the rule
  ([issue 12](https://github.com/mikilabs/qmdc/issues/12), QMD-77).
- An indented sub-item is never a field, in all three parsers, and the forbidden construct is always
  reported. Rust promoted a sub-item whose text happened to parse as `key: value` to a real field of
  the parent object — `- AMBIGUOUS: text` under `- issues:` became an `AMBIGUOUS` field — keyed by
  prose and with no diagnostic, which on one external corpus of 240 documents cost 23 of the 148
  reports the other two made. Separately, when the construct was an object's ONLY content, Python and
  TypeScript kept it as comment text and reported nothing where Rust reported it, and Rust anchored a
  following comment on the removed key, so the anchor named a field no parser keeps. All three now
  drop the key and the construct, raise one `nested_subitems` at the key's line, and re-anchor a
  following comment on the last surviving field or on `__self` (QMD-77 A4, C5).
- A parse error is no longer offered as an LSP completion. The candidate filter skipped system
  objects by the `doc_` / `text_` id prefix, which missed `__ParsingError`, so `error_0` appeared in
  the reference completion list of any document holding a parse error. Filtered by kind now, which is
  what makes these three unreferenceable: none carries a `__global_id` (QMD-77).
- A workspace scan that meets a directory it cannot read now skips it and returns everything else, in
  TypeScript as in Rust, Python and `git`. An uncaught `EACCES` from `readdirSync` cost the WHOLE
  result — not one readable file came back — for three of the four scanners in `workspace.ts`
  (QMD-77 D1).
- Every `__ParsingError` in one parse result has its own `__id`, numbered in the order the errors
  were raised — `error_0`, `error_1`, … — in all three parsers. Rust minted ids from two counters, so
  two errors on one object could both be `error_0`, and a `structured_in_textblock` error was named
  `parsing_error_<n>` where Python and TypeScript said `error_<n>` (QMD-77, review round 1).
- **The three parsers now produce identical `parse` output for every document in the repository** —
  0 divergent of 111, measured by `make validate-compare`, which had reported 32 when the check was
  first added. `scripts/parse-parity-baseline.json` is deleted rather than set to 0, so any new
  divergence fails immediately instead of fitting under a cap. Eleven independent causes, where a
  per-key diff had suggested five (QMD-71).
- Comment content is now the raw markdown fragment in every path, as the format has always specified.
  Rust rebuilt it from inline events in the paragraph path, which silently LOST or rewrote content: an
  image kept only its alt text (markup and path gone), an autolink `<url>` was rewritten as
  `[url](url)`, `***` and `___` and `- - -` were all normalised to `---`, a footnote definition lost its
  `[^1]:` label, and a list inside a blockquote was emitted TWICE with the first copy's marker
  stripped. The Rust extension set is now pinned explicitly instead of taking every extension
  `pulldown-cmark` ships, which is what enabled the footnote handling (QMD-71).
- A link reference definition (`[d]: https://…`) inside a comment is no longer dropped by Rust and
  TypeScript. Neither tokenizer emits an event for it, so rebuilding produced a document whose `[d]`
  labels pointed at nothing and would render as literal text. Rust now slices a comment to the next
  event's start rather than to the paragraph's end, and TypeScript extends to end of document when no
  boundary follows (QMD-71).
- A comment anchored on a `text` field no longer attaches to the preceding OBJECT in Rust — the single
  largest group, 13 of the divergent documents — and a `text` field declared on an already-finalized
  parent now gets its `__types` and `__syntax` entries. In TypeScript a `text` field whose content
  begins with a bullet list now records the comment anchor, instead of leaving it on the parent's last
  scalar field (QMD-71).
- An ordered list followed by a fence inside a `text` field keeps both in TypeScript; a float's
  fractional part survives serialisation (`2.0` no longer emits as `2`); Python no longer renumbers a
  nested bullet list into the outer ordered list; Rust no longer extracts `key: value` entries from a
  list nested under an ordered item as real fields; and Python no longer reads `1_000` as 1000, which
  was Python's own numeric literal syntax leaking through (QMD-71).
- A Markdown table written inside an object-array *element* is now that element's own content,
  carried in `__comments`, instead of being converted into extra rows of the parent array. In
  Rust some shapes also lost the element entirely — its explicit id became an empty array and its
  fields were dropped — which turned references to it into false `broken_link` diagnostics. All
  three parsers now agree on every shape an element can take, and on what follows the array's own
  table. **Behaviour change:** a document that relied on the old reading to add rows parses to a
  different graph, silently (QMD-70).
- A second Markdown table under one object-array heading no longer loses a row. Rust converted both
  tables, both children took the same local id, and the second overwrote the first; the construct is
  now an `extra_table_in_array` error with the content preserved in `__comments` (QMD-70).
- Rust no longer destroys a `yaml` / `json` field's value with the text that follows its fence. The
  field stayed open after the fence, so a following paragraph was appended into it and overwrote the
  parsed object (`conf: {a: 1}` became the paragraph's text); the field now closes on its fence, as in
  Python, and the paragraph is kept as a comment on the parent (QMD-70).
- Rust no longer discards content that follows an object array's own table. A trailing paragraph was
  dropped and a trailing `- field: value` was lost entirely, because the array's parent object was
  closed at the array heading and the paths that write fields and comments had nothing left to write
  to; the parent now stays open for the whole array (QMD-70).
- Prose between an object-array heading and its table no longer breaks the connection in Python and
  TypeScript: the prose is the container's comment and the table still feeds the array, matching what
  all three already did when a heading element follows the prose instead of a table (QMD-70).
- Rust preserves blockquote comments verbatim instead of reconstructing the `>` prefixes, which had
  dropped an empty blockquote entirely, lost a leading blank quoted line, and collapsed a nested
  `> >` to one level (QMD-70).
- TypeScript no longer cuts the separator row off a Markdown table that has no data rows when the
  table is carried as comment content (QMD-70).
- Table-fed array children now compose their hierarchical id through the same rule as the array's
  heading elements in all three parsers. Rust doubled the segment when the array field name
  equalled the parent's id (`items.items.items_0` instead of `items.items_0`) and prefixed the
  parent id under a `__Workspace` / `__Namespace` parent (`ns_rows_0` instead of `rows_0`)
  (QMD-70).

## [1.0.2] - 2026-07-20

### Added

- LSP and MCP now report `ambiguous_field_reference` (QMDC009), matching the CLI (QMD-66).
- Agent guide rewritten around real dot-notation semantics: resolution rules, dot-ID declarations, `__local_id` fallback, id-scoping guidance (QMD-66).
- Duplicate-ID detection is namespace-scoped: two objects sharing an `__id` in different namespaces are distinct and no longer flagged as duplicates (QMD-67).
- Cross-file / namespace-scoped `duplicate_id` (QMDC003) is now surfaced by the LSP and MCP, not just the CLI (QMD-68).
- Structural parser diagnostics (`dangling_field`, `mixed_field_keys`, `multiple_definitions`, `structured_in_textblock`, `broken_parent`, `nested_workspace`, `workspace_in_wrong_file`, …) are now surfaced by MCP, and single-document structural errors by the LSP (QMD-68).
- MCP now reports same-file duplicate ids (QMD-68).

### Changed

- CLI, LSP, and MCP now share a single reference-validation engine (`core::reference_scan`) and a single duplicate detector, so the three surfaces can no longer drift. A surface-coverage matrix is documented in `docs/lsp/diagnostics.qmd.md` (QMD-68).

### Fixed

- Rename and find-references now handle field-path refs like `[[#team.config.timeout]]` — rename no longer silently breaks them (QMD-66).
- UTF-8 panic in validation on long non-ASCII field values (QMD-66).
- `ambiguous_reference` severity unified to error across CLI/LSP/MCP (QMD-66).
- `ambiguous_field_reference` (QMDC009) messages now include the conflicting object/field candidates on all surfaces (QMD-68).
- LSP duplicate-ID highlighting underlines the definition marker for both `[[id]]` and `[[id: Kind]]` forms (QMD-68).

### Removed

- Undocumented-fiction cleanup: filter/wildcard reference syntax and `type_mismatch` removed from docs; error catalogs now match what the parsers actually emit (QMD-66).

## [1.0.1] - 2026-07-02

- MCP workspace resolution searches down then up, so any path (a repo or container dir) resolves the workspace inside it and a container with several workspaces returns an `ambiguous` error with candidates — bundled-binary bump shipping in [qmdc (PyPI)](https://pypi.org/project/qmdc/), [qmdc (crates.io)](https://crates.io/crates/qmdc), [@qmdc/qmdc (npm)](https://www.npmjs.com/package/@qmdc/qmdc), and [qmdc-vscode](https://marketplace.visualstudio.com/items?itemName=MiKiLabs.qmdc-vscode).

## [1.0.0] - 2026-06-13

Initial release.

[Unreleased]: https://github.com/mikilabs/qmdc/compare/v2.0.0...HEAD
[2.0.0]: https://github.com/mikilabs/qmdc/releases/tag/v2.0.0
[1.0.2]: https://github.com/mikilabs/qmdc/releases/tag/v1.0.2
[1.0.1]: https://github.com/mikilabs/qmdc/releases/tag/v1.0.1
[1.0.0]: https://github.com/mikilabs/qmdc/releases/tag/v1.0.0
