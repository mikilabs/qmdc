# QMDC Parser (Python)

QMDC ↔ JSON parser for Markdown with lossless round-trip support.

## Installation

```bash
cd qmdc-py
uv pip install -e .
```

## CLI Usage

### Parse (QMD.md → JSON)

```bash
# File → stdout
qmdc parse -i doc.qmd.md

# Stdin → stdout
echo "## Test [[test]]" | qmdc parse

# File → file
qmdc parse -i doc.qmd.md -o output.json

# Without metadata
qmdc parse -i doc.qmd.md --no-comments --no-syntax

# Compact JSON
qmdc parse -i doc.qmd.md --no-pretty
```

### Rebuild (JSON → QMD.md)

```bash
# File → stdout
qmdc rebuild -i data.json

# Stdin → stdout
echo '[{"__id":"test","name":"Test"}]' | qmdc rebuild

# File → file
qmdc rebuild -i data.json -o doc.qmd.md
```

### Formatting (parse → rebuild)

There is no separate `lint` command. Canonical formatting (like `ruff`/`prettier`)
is the lossless round-trip through parse → rebuild:

```bash
# Canonical formatting
qmdc parse -i doc.qmd.md | qmdc rebuild
```

## Programmatic API

```python
from qmdc.parser import parse, rebuild

# Parse QMD.md → JSON
markdown = """
## User [[user]]

- name: Alice
- age: 30
"""
result = parse(markdown)
# [{"__id": "user", "__label": "User", "name": "Alice", "age": 30, ...}]

# Rebuild JSON → QMD.md
qmdc = rebuild(result)
# "## User [[user]]\n\n- name: Alice\n- age: 30\n"
```

## Testing

```bash
# From the project root
make test

# Python tests only
make py-test

# pytest directly
cd qmdc-py
uv run pytest tests/ -v
```

## Workspace (multi-file parsing)

QMDC Parser supports workspaces — a set of related QMD.md files with cross-file
reference validation.

### Workspace CLI

```bash
# Parse a workspace into JSON
qmdc workspace parse ./my-project -o workspace.json

# Validate a workspace
qmdc workspace validate ./my-project

# Rebuild files from JSON
qmdc workspace rebuild workspace.json -o ./output
```

### Query (SQL queries against a workspace)

```bash
# SQL query against a workspace (loads all workspaces recursively)
qmdc query ./my-project "SELECT __id, __kind, __label FROM objects WHERE __kind = 'Service'"

# With JSON output
qmdc query ./my-project "SELECT * FROM objects LIMIT 10" --format json

# Query via a Query object (reference to a [[id:Query]] object in the workspace)
qmdc query ./my-project "#all_services"

# Count objects and edges
qmdc query ./my-project "SELECT COUNT(*) as total FROM objects"
qmdc query ./my-project "SELECT COUNT(*) as edges FROM edges"
```

The `query` command automatically:

- Recursively finds every QMDC workspace under the given folder
- Parses all files and loads objects into SQLite
- Extracts graph edges from references between objects
- Runs the SQL query against the database

Available tables:

- `objects` — all objects with `__id`, `__kind`, `__label`, `__file`, `__line`, `data` (JSON)
- `edges` — all graph edges with `source_id`, `target_id`, `field_name`

### Workspace API

```python
from qmdc.workspace import (
    scan_workspace,
    parse_workspace,
    validate_workspace,
    query_workspace,
    get_refs_to,
)

# Scan files
files = scan_workspace("/path/to/workspace")
# ['readme.qmd.md', 'users.qmd.md', 'database/tables.qmd.md']

# Parse the whole workspace
result = parse_workspace("/path/to/workspace")
# WorkspaceResult with objects, index, errors

# Query objects
tables = query_workspace(result, kind="Table")
users = query_workspace(result, object_id="users")

# Find references to an object
refs = get_refs_to(result, "database/tables.qmd.md#users")
# [("api.qmd.md", "get_users", "returns"), ...]
```

See the [format specification](https://qmdc.mikilabs.io/) for the full spec.

## Status

✅ **Fully implemented:**

- Tokenizer (markdown-it-py)
- Header parser (all variants: `[[id]]`, `[[id:Kind]]`, `[[:Kind]]`)
- Field parser (primitives: string, number, boolean, null)
- Nested objects (H1–H6)
- Arrays (YAML notation, Markdown lists, object arrays)
- Tables
- Comments (`__comments`)
- Syntax metadata (`__syntax`)
- Types metadata (`__types`)
- YAML multiline syntax (`|`)
- Rebuild (canonical form)
- YAML blocks
- JSON blocks
- Document title (H1 without `[[id]]`)
- Lossless round-trip (`__level`, `__has_explicit_id`)
- CLI: `parse`, `rebuild`
- **Workspace**: multi-file parsing with reference validation

## License

[AGPL-3.0-or-later](LICENSE) © mikilabs
