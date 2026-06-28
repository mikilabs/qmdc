# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
Each package (`qmdc`, `qmdc-semantic`, `qmdc-mkdocs`, `qmdc-vscode`) is versioned
independently following [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

This file is maintained by hand; entries are collected under `## [Unreleased]`
and curated into a versioned section at release time.

## [Unreleased]

_Nothing yet._

## [1.0.0] - 2026-06-13

Initial public release of the QMD.md format and the QMDC toolchain.

### Added

- **QMD.md format** — Markdown convention for structured data: headings as objects,
  list items as fields, `[[#references]]` as typed edges, stored in `.qmd.md` files.
- **Three parser implementations** at byte-for-byte parity, sharing one conformance
  test corpus:
  - `qmdc-py` — Python reference implementation (workspace + SQL).
  - `qmdc-rs` — high-performance Rust implementation with LSP and MCP servers.
  - `qmdc-ts` — TypeScript implementation for Node.js and the browser.
- **`qmdc` CLI** — `parse`, `rebuild`, `workspace parse`/`validate`, and SQL `query`,
  published to PyPI, npm, and crates.io with a bundled native binary.
- **`qmdc-semantic`** — semantic search over a QMDC workspace (hybrid search, graph
  walk, inferred edges).
- **`qmdc-mkdocs`** — MkDocs integration to build a documentation site from a QMDC
  workspace.
- **`qmdc-vscode`** — VS Code extension with LSP-powered diagnostics, completion,
  navigation, and refactoring.
- Documentation site at <https://qmdc.mikilabs.io/>.

[1.0.0]: https://github.com/mikilabs/qmdc/releases/tag/v1.0.0
