# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
The whole system is released under a single version following
[Semantic Versioning](https://semver.org/spec/v2.0.0.html); a release ships all
packages (`qmdc`, `qmdc-semantic`, `qmdc-mkdocs`, `qmdc-vscode`) together. This
file is maintained by hand.

## [Unreleased]

### Added

- References can be qualified with a workspace: `[[#workspace:namespace:id.field]]`, a
  right-aligned suffix of the object's global id. `[[#ws::id]]` elides the namespace and matches
  any namespace of that workspace. A reference that crosses a workspace boundary MUST now be
  qualified; an ambiguous qualified reference produces no edge rather than an arbitrary one
  (QMD-69).

### Added

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

### Fixed

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

[1.0.2]: https://github.com/mikilabs/qmdc/releases/tag/v1.0.2
[1.0.1]: https://github.com/mikilabs/qmdc/releases/tag/v1.0.1
[1.0.0]: https://github.com/mikilabs/qmdc/releases/tag/v1.0.0
