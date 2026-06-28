# Rust Parser [[rust_parser: Parser]]

- language: rust
- source: qmdc-rs/
- implements: [[#format]]

## Description [[description: text]]

High-performance implementation with LSP server and SQLite integration.

Built on `pulldown-cmark` for tokenization. Includes a full Language Server Protocol server for IDE integration. Used in the VS Code extension.

**Modules:**

| Module | File | Description |
|--------|------|-------------|
| Parser | `parser.rs` | Core QMD.md → JSON parsing logic |
| Parser Modules | `parser_modules/` | Modular parser components (block_tree, header, output, references, utils, value_parser) |
| Rebuild | `rebuild.rs` | JSON → QMD.md conversion |
| Workspace | `workspace.rs` | Multi-file parsing, recursive search |
| DB | `db/mod.rs` | SQLite integration via rusqlite |
| LSP Server | `lsp/server.rs` | LSP server (tower-lsp) |
| LSP Document | `lsp/document.rs` | Open document management |
| LSP Commands | `lsp/commands.rs` | Custom LSP commands |
| LSP Tree | `lsp/tree.rs` | Object tree construction |
| LSP Workspace | `lsp/workspace.rs` | Workspace for LSP |
| LSP SQL Rewrite | `lsp/sql_rewrite.rs` | SQL AST rewrite for workspace isolation |
| Main | `main.rs` | CLI entry point |
| Lib | `lib.rs` | Public library API |

**Technologies:**

- `pulldown-cmark` — Markdown tokenization
- `serde` + `serde_json` — JSON serialization
- `rusqlite` — SQLite integration
- `tower-lsp` — LSP server framework
- `tokio` — async runtime
- `clap` — CLI framework
- See full dependency list in `qmdc-rs/Cargo.toml`

**Data flow:**

```text
QMD.md
    ↓
pulldown-cmark parser
    ↓
Event stream (Start(Heading), Text, End(Heading), ...)
    ↓
Parser (parser.rs)
    ↓
Object hierarchy (parent-child by heading levels)
    ↓
Field extraction
    ↓
serde_json::Value objects
```

**Installation:**

```bash
# From source
cd qmdc-rs
cargo build --release
./target/release/qmdc --version

# Via Python package
cd qmdc-rs
make wheel-dev
```

**Performance** (MacBook Pro M1):

- Parsing a 1000-line file — ~0.5ms (20x faster than Python)
- Workspace with 100 files — ~50ms (20x faster than Python)
- SQL query against workspace — ~5ms (10x faster than Python)
- LSP completion — <10ms (real-time)

Optimized for production use and real-time IDE editing.

**Limitations:**

- No `lint` command (use [[#python_parser]] or [[#typescript_parser]])
- Requires compilation (but available via Python package)

## Features [[features: text]]

- parse — QMD.md → JSON conversion
- rebuild — JSON → QMD.md restoration
- workspace — multi-file parsing and validation
- query — SQL queries against workspace via SQLite
- LSP server — completion, hover, go-to-definition, references, diagnostics, rename, document symbols, workspace symbols
- Zero-copy parsing via `pulldown-cmark`
- Parallel file parsing via `rayon`
- Parse result caching in LSP
- Cross-platform Python package via `cargo-zigbuild`
