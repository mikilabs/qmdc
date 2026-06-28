# QMD-62: Offline documentation search in the qmdc MCP server

## Offline embedded doc-search tools for the MCP [[qmd62: Feature]]

Give the `qmdc` MCP server a built-in way to search the product documentation and fetch document content, fully offline and with (almost) zero runtime cost: the docs corpus is compiled into the binary and queried with a lexical engine (BM25), no embeddings, no external services, no extra processes. This is the "search the manual" capability an agent needs to answer "how do I do X in qmdc" without the user having to set up the Python `qmdc-semantic` stack or an embedding provider.

- status: planned
- priority: medium
- category: docs
- related_task: [[#qmd61]]
- requires_changes: []
- findings: []
- result: null

### Scope [[qmd62_scope: text]]

Two new MCP tools over a **version-locked, build-embedded** docs corpus (separate from the user's workspace index):

- `qmdc_search_docs(query, limit?)` — ranked lexical search; returns `doc` (logical path), `heading`, line-span, snippet, score.
- `qmdc_get_doc(doc, start_line?, end_line?)` — fetch the embedded document content, with an optional line range; bounded output (`total_lines` + `truncated`). Reads ONLY from the embedded corpus — not an arbitrary file reader (keeps the fail-closed `force_root` boundary intact).

Both reuse the shared `{success, ...}` envelope and need no `path` argument (like `qmdc_get_guide`).

### Constraints / non-goals [[qmd62_constraints: text]]

- **No embeddings, no synonym/alias dictionaries, no query-rewriting cheats** — pure lexical IR only. Semantic recall (synonyms/paraphrase) is explicitly out of scope; it stays in the optional Python `qmdc-semantic` server.
- **Offline & self-contained** — the Rust binary must keep working with no Python, no provider, no network. The docs corpus is vendored into the crate and embedded at build time (same pattern as `core::guide`).
- **Version-locked** — the embedded docs match the binary version; a `make docs-sync` mirror + a CI drift check keep the vendored snapshot in sync with `docs/`.
- **Bounded output** — honour the existing MCP output cap; large docs require a range or are truncated with a flag.

### Decisions already taken (see findings) [[qmd62_decisions: text]]

- Engine: **tantivy** (pure-Rust, Lucene-like, BM25). Validated on our docs — see [[#qmd62_finding_eval]].
- Retrieval quality comes from: per-heading chunking + title boost + char-trigram field (typo tolerance) + analyzer-consistent fuzzy + edismax-style query assembly. All legitimate IR, no synonyms.
- The one inherent lexical gap (vocabulary mismatch, e.g. "website" vs "SSG") is closed by **editing the docs content**, not the engine — already demonstrated on `docs/ssg/readme.qmd.md`.

### Open questions [[qmd62_open_questions: text]]

To resolve during triage:

1. Embed raw markdown + build the index at startup, vs embed a prebuilt tantivy/SQLite index blob. (Findings lean toward embed-markdown + build-on-first-use — corpus is ~300 KB / ~1k chunks, build is sub-second.)
2. tantivy as a new `qmdc-rs` dependency vs native FTS5 over the already-bundled SQLite. (tantivy is stronger lexically — fuzzy + tokenization — but adds binary size; FTS5 is zero-dep. Hide the engine behind a core op so it is swappable.)
3. Result locator shape: line-range vs heading/anchor for `qmdc_get_doc`.
4. Whether to also mirror docs as `qmdc://doc/<path>` resources (some clients bridge only tools — see the note in `core::guide`).

## Checklist

- [ ] Understood the task
- [ ] Studied the code (MCP tool surface, `core::guide` embedding, index seam)
- [ ] Created a plan and prototypes in `artifacts/`
- [ ] Tested the solution
- [ ] Moved the code into the project
- [ ] Created Result.md and Findings.md
