# Python Parser [[python_parser: Parser]]

- language: python
- source: qmdc-py/
- implements: [[#format]]

## Description [[description: text]]

Reference implementation in Python. Simple and readable code for learning the QMD.md format.

Built on `markdown-it-py` for tokenization. Ideal for prototyping and understanding the format specification.

**Modules:**

| Module | File | Description |
|--------|------|-------------|
| Tokenizer | `tokenizer.py` | Markdown tokenization via markdown-it-py |
| Parser | `parser.py` | Core QMD.md → JSON parsing logic |
| Header Parser | `parsers/header.py` | Heading parsing with `[[id]]`, `[[id:Kind]]` |
| Field Parser | `parsers/field.py` | Field parsing `- key: value` |
| Workspace | `workspace.py` | Multi-file parsing, reference validation |
| DB | `db.py` | SQLite integration for query command |
| CLI | `cli.py` | Command line interface (click) |

**Technologies:**

- `markdown-it-py` — Markdown tokenization
- See full dependency list in `qmdc-py/pyproject.toml`

**Data flow:**

```text
QMD.md
    ↓
markdown-it-py tokenizer
    ↓
Token stream (heading, list_item, paragraph, ...)
    ↓
Parser (parser.py)
    ↓
Object hierarchy (parent-child by heading levels)
    ↓
Field extraction (parsers/field.py)
    ↓
JSON objects
```

**Installation:**

```bash
cd qmdc-py
uv pip install -e .
```

**Performance** (MacBook Pro M1):

- Parsing a 1000-line file — ~10ms
- Workspace with 100 files — ~1s
- SQL query against workspace — ~50ms

For large workspaces (1000+ files), use [[#rust_parser]].

**Limitations:**

- No LSP server (use [[#rust_parser]])
- 10–20x slower than the Rust parser
- Not suitable for real-time IDE editing

## Features [[features: text]]

- parse — QMD.md → JSON conversion
- rebuild — JSON → QMD.md restoration
- workspace — multi-file parsing and validation
- query — SQL queries against workspace via SQLite
