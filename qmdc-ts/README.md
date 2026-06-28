# QMDC Parser (TypeScript)

QMDC ↔ JSON parser for Markdown with lossless round-trip support.

## Installation

```bash
# Use the CLI directly (bundles the native binary)
npx @qmdc/qmdc --help

# Or install globally / as a project dependency
npm install -g @qmdc/qmdc
npm install @qmdc/qmdc

# From a checkout of this repo (development)
cd qmdc-ts
npm install
```

## CLI Usage

### Parse (QMD.md → JSON)

```bash
# File → stdout
./qmdc parse -i doc.qmd.md

# Stdin → stdout
echo "## Test [[test]]" | ./qmdc parse

# File → file
./qmdc parse -i doc.qmd.md -o output.json

# Without metadata
./qmdc parse -i doc.qmd.md --no-comments --no-syntax

# Compact JSON
./qmdc parse -i doc.qmd.md --no-pretty
```

### Rebuild (JSON → QMD.md)

```bash
# File → stdout
./qmdc rebuild -i data.json

# Stdin → stdout
echo '[{"__id":"test","name":"Test"}]' | ./qmdc rebuild

# File → file
./qmdc rebuild -i data.json -o doc.qmd.md
```

### Formatting (parse → rebuild)

There is no separate `lint` command. Canonical formatting (like `ruff`/`prettier`)
is the lossless round-trip through parse → rebuild:

```bash
# Canonical formatting
./qmdc parse -i doc.qmd.md | ./qmdc rebuild
```

## Programmatic API

```typescript
import { parse, rebuild } from '@qmdc/qmdc';

// Parse QMD.md → JSON
const markdown = `
## User [[user]]

- name: Alice
- age: 30
`;
const result = parse(markdown);
// [{"__id": "user", "__label": "User", "name": "Alice", "age": 30, ...}]

// Rebuild JSON → QMD.md
const qmdc = rebuild(result);
// "## User [[user]]\n\n- name: Alice\n- age: 30\n"
```

## Testing

```bash
# From the project root
make test

# TypeScript tests only
make ts-test

# npm directly
cd qmdc-ts
npm test
```

## Workspace (multi-file parsing)

QMDC Parser supports workspaces — a set of related QMD.md files with cross-file
reference validation.

### Workspace CLI

```bash
# Parse a workspace into JSON
./qmdc workspace parse ./my-project -o workspace.json

# Validate a workspace
./qmdc workspace validate ./my-project

# Rebuild files from JSON
./qmdc workspace rebuild workspace.json -o ./output
```

### Query (SQL queries against a workspace)

```bash
# SQL query against a workspace (loads all workspaces recursively)
./qmdc query ./my-project "SELECT __id, __kind, __label FROM objects WHERE __kind = 'Service'"

# With JSON output
./qmdc query ./my-project "SELECT * FROM objects LIMIT 10" --format json

# Query via a Query object (reference to a [[id:Query]] object in the workspace)
./qmdc query ./my-project "#all_services"

# Count objects and edges
./qmdc query ./my-project "SELECT COUNT(*) as total FROM objects"
./qmdc query ./my-project "SELECT COUNT(*) as edges FROM edges"
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

```typescript
import {
  scanWorkspace,
  parseWorkspace,
  queryWorkspace,
  getRefsTo,
} from '@qmdc/qmdc/workspace';

// Scan files
const files = scanWorkspace('/path/to/workspace');
// ['readme.qmd.md', 'users.qmd.md', 'database/tables.qmd.md']

// Parse the whole workspace
const result = parseWorkspace('/path/to/workspace');
// WorkspaceResult with objects, index, errors

// Query objects
const tables = queryWorkspace(result, { kind: 'Table' });
const users = queryWorkspace(result, { id: 'users' });

// Find references to an object
const refs = getRefsTo(result, 'database/tables.qmd.md#users');
// [{ file: 'api.qmd.md', object: 'get_users', field: 'returns' }, ...]
```

See the [format specification](https://qmdc.mikilabs.io/) for the full spec.

## Status

✅ **Fully implemented:**

- Tokenizer (markdown-it)
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
