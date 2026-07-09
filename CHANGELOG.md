# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
The whole system is released under a single version following
[Semantic Versioning](https://semver.org/spec/v2.0.0.html); a release ships all
packages (`qmdc`, `qmdc-semantic`, `qmdc-mkdocs`, `qmdc-vscode`) together. This
file is maintained by hand.

## [Unreleased]

### Added

- LSP and MCP now report `ambiguous_field_reference` (QMDC009), matching the CLI (QMD-66).
- Agent guide rewritten around real dot-notation semantics: resolution rules, dot-ID declarations, `__local_id` fallback, id-scoping guidance (QMD-66).

### Fixed

- Rename and find-references now handle field-path refs like `[[#team.config.timeout]]` — rename no longer silently breaks them (QMD-66).
- UTF-8 panic in validation on long non-ASCII field values (QMD-66).
- `ambiguous_reference` severity unified to error across CLI/LSP/MCP (QMD-66).

### Removed

- Undocumented-fiction cleanup: filter/wildcard reference syntax and `type_mismatch` removed from docs; error catalogs now match what the parsers actually emit (QMD-66).

## [1.0.1] - 2026-07-02

- MCP workspace resolution searches down then up, so any path (a repo or container dir) resolves the workspace inside it and a container with several workspaces returns an `ambiguous` error with candidates — bundled-binary bump shipping in [qmdc (PyPI)](https://pypi.org/project/qmdc/), [qmdc (crates.io)](https://crates.io/crates/qmdc), [@qmdc/qmdc (npm)](https://www.npmjs.com/package/@qmdc/qmdc), and [qmdc-vscode](https://marketplace.visualstudio.com/items?itemName=MiKiLabs.qmdc-vscode).

## [1.0.0] - 2026-06-13

Initial release.

[1.0.1]: https://github.com/mikilabs/qmdc/releases/tag/v1.0.1
[1.0.0]: https://github.com/mikilabs/qmdc/releases/tag/v1.0.0
