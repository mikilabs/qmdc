# TypeScript Parser [[typescript_parser: Parser]]

- language: typescript
- source: qmdc-ts/
- implements: [[#format]]

## Description [[description: text]]

Implementation for Node.js and browser with full type safety.

Built on `markdown-it` for tokenization. Ideal for web applications and JavaScript/TypeScript tooling.

**Modules:**

| Module | File | Description |
|--------|------|-------------|
| Tokenizer | `tokenizer.ts` | Markdown tokenization via markdown-it |
| Parser | `parser.ts` | Core QMD.md → JSON parsing logic |
| Header Parser | `parsers/header.ts` | Heading parsing with `[[id]]`, `[[id:Kind]]` |
| Field Parser | `parsers/field.ts` | Field parsing `- key: value` |
| Workspace | `workspace.ts` | Multi-file parsing, reference validation |
| DB | `db.ts` | SQLite integration via sql.js |
| CLI | `cli.ts` | Command line interface (commander.js) |

**Technologies:**

- `markdown-it` — Markdown tokenization
- `js-yaml` — YAML block parsing
- `sql.js` — SQLite integration
- `commander` — CLI framework
- See full dependency list in `qmdc-ts/package.json`

**Data flow:**

```text
QMD.md
    ↓
markdown-it tokenizer
    ↓
Token stream (heading_open, list_item_open, ...)
    ↓
Parser (parser.ts)
    ↓
Object hierarchy (parent-child by heading levels)
    ↓
Field extraction (parsers/field.ts)
    ↓
Typed JSON objects
```

**Installation:**

```bash
cd qmdc-ts
npm install
```

**Performance** (MacBook Pro M1):

- Parsing a 1000-line file — ~8ms
- Workspace with 100 files — ~800ms
- SQL query against workspace — ~40ms

20–30% faster than the Python parser, but 5–10x slower than the Rust parser.

**Limitations:**

- No LSP server (use [[#rust_parser]])
- Workspace/Query commands do not work in the browser
- Requires Node.js for full functionality

## Features [[features: text]]

- parse — QMD.md → JSON conversion
- rebuild — JSON → QMD.md restoration
- workspace — multi-file parsing and validation
- query — SQL queries against workspace via SQLite
- Browser support — parse and rebuild work in the browser via bundler (webpack, vite)
- Full TypeScript typing — interfaces for all data structures, type guards, generics
