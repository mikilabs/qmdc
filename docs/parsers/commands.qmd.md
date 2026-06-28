# CLI Commands

Commands for working with QMD.md files via the `qmdc` CLI.

## Parse [[cmd_parse: Command]]

Converts QMD.md to JSON.

- parser: [[#python_parser]], [[#typescript_parser]], [[#rust_parser]]

### Description [[description: text]]

Reads QMD.md from a file or stdin and outputs JSON objects to a file or stdout. Supports different output formats (minimal, standard, full) and optional metadata exclusion (__comments,__syntax).

**Output formats:**

- **minimal** — user fields only
- **standard** — system fields + user fields (default)
- **full** — standard + __references,__comments, __types

**Round-trip guarantee:** parse → rebuild restores the original document.

### Syntax [[syntax: text]]

```bash
qmdc parse -i <file>
qmdc parse -i <file> -o <output>
echo '## Test [[test]]' | qmdc parse
```

### Options [[options: text]]

| Option | Short | Description | Type | Required |
|--------|-------|-------------|------|----------|
| `--input` | `-i` | Input file (reads from stdin if omitted) | path | no |
| `--output` | `-o` | Output file (writes to stdout if omitted) | path | no |
| `--format` | | Output format: minimal, standard (default), full (Rust only) | enum | no |
| `--no-comments` | | Exclude `__comments` field from output | boolean | no |
| `--no-syntax` | | Exclude `__syntax` field from output | boolean | no |
| `--no-pretty` | | Compact JSON without formatting | boolean | no |

### Examples [[examples: text]]

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

# Rust: choose format (minimal, standard, full)
qmdc parse -i doc.qmd.md --format full
```

## Rebuild [[cmd_rebuild: Command]]

Converts JSON back to QMD.md.

- parser: [[#python_parser]], [[#typescript_parser]], [[#rust_parser]]

### Description [[description: text]]

Reads JSON objects from a file or stdin and outputs QMD.md to a file or stdout. Ensures lossless round-trip: parse → rebuild restores the original document. Preserves object and field order. Restores hierarchy from __parent and__level.

### Syntax [[syntax: text]]

```bash
qmdc rebuild -i <file>
qmdc rebuild -i <file> -o <output>
```

### Options [[options: text]]

| Option | Short | Description | Type | Required |
|--------|-------|-------------|------|----------|
| `--input` | `-i` | Input JSON file (reads from stdin if omitted) | path | no |
| `--output` | `-o` | Output QMD.md file (writes to stdout if omitted) | path | no |

### Examples [[examples: text]]

```bash
# File → stdout
qmdc rebuild -i data.json

# Stdin → stdout
echo '[{"__id":"test","name":"Test"}]' | qmdc rebuild

# File → file
qmdc rebuild -i data.json -o doc.qmd.md
```

### Tip: canonical formatting [[cmd_rebuild_formatting: text]]

There is no separate `lint` command. To format a QMD.md file to canonical form (the equivalent of `ruff`/`prettier`), pipe it through parse → rebuild — a lossless round-trip that normalizes whitespace and field/array formatting:

```bash
qmdc parse -i doc.qmd.md | qmdc rebuild
```

## Query [[cmd_query: Command]]

Executes SQL queries against workspace via SQLite.

- parser: [[#python_parser]], [[#typescript_parser]], [[#rust_parser]]

### Description [[description: text]]

Automatically:

- Recursively finds all QMDC workspaces in the specified directory
- Parses all files and loads objects into SQLite
- Extracts graph edges from references between objects
- Executes the SQL query against the database

**Available tables:**

- `objects` — all objects with fields `__id`, `__kind`, `__label`, `__file`, `__line`, `data` (JSON)
- `edges` — all graph edges with fields `source_id`, `source_field`, `target_id`, `edge_type`, `__workspace`

### Syntax [[syntax: text]]

```bash
qmdc query <path> "<sql>"
qmdc query <path> "#query_id"
```

### Options [[options: text]]

| Option | Short | Description | Type | Required |
|--------|-------|-------------|------|----------|
| `<path>` | | Path to workspace directory | path | yes |
| `<query>` | | SQL query or reference to a Query object (`#query_id`) | string | yes |
| `--format` | | Output format: table (default), json | enum | no |

### Examples [[examples: text]]

```bash
# SQL query against workspace
qmdc query ./my-project "SELECT __id, __kind, __label FROM objects WHERE __kind = 'Service'"

# JSON output format
qmdc query ./my-project "SELECT * FROM objects LIMIT 10" --format json

# Query via Query object (reference to [[id:Query]] in workspace)
qmdc query ./my-project "#all_services"

# Count objects and edges
qmdc query ./my-project "SELECT COUNT(*) as total FROM objects"
qmdc query ./my-project "SELECT COUNT(*) as edges FROM edges"
```

## Workspace Parse [[cmd_workspace_parse: Command]]

Parses an entire workspace (multiple linked QMD.md files) to JSON.

- parser: [[#python_parser]], [[#typescript_parser]], [[#rust_parser]]

### Description [[description: text]]

Finds the workspace root by locating a file with `[[id:__Workspace]]`. Recursively finds all `.qmd.md` files, respects `.qmdcignore` for exclusions. Parses each file and adds metadata (__file,__workspace, __namespace). Returns all objects + file list + validation errors.

### Syntax [[syntax: text]]

```bash
qmdc workspace parse <path>
qmdc workspace parse <path> -o <output>
```

### Options [[options: text]]

| Option | Short | Description | Type | Required |
|--------|-------|-------------|------|----------|
| `<path>` | | Path to workspace directory | path | yes |
| `--output` | `-o` | Output JSON file (Python/TypeScript) | path | no |
| `--format` | | Output format: minimal, standard, full (Rust only) | enum | no |

### Examples [[examples: text]]

```bash
# Python/TypeScript
qmdc workspace parse ./my-project -o workspace.json

# Rust (output to stdout)
qmdc workspace parse ./my-project > workspace.json

# With format selection (Rust only)
qmdc workspace parse ./my-project --format full
```

## Workspace Validate [[cmd_workspace_validate: Command]]

Validates workspace: checks for broken links, duplicate IDs, ambiguous references.

- parser: [[#python_parser]], [[#typescript_parser]], [[#rust_parser]]

### Description [[description: text]]

Returns **only a JSON array of errors** (empty array `[]` if no errors).

Available in all three parsers (Python, TypeScript, Rust).

**Error types:**

- `broken_link` — reference `[[#id]]` to a non-existent object (after both `__id` and `__local_id` fallback lookups fail)
- `duplicate_id` — two objects with the same ID in one namespace
- `ambiguous_reference` — reference that could point to multiple objects (by `__id` Kind collision or by multiple `__local_id` matches)
- `nested_workspace` — workspace inside another workspace (forbidden)
- `workspace_in_wrong_file` — workspace declaration in wrong file

**Resolution order:** for each reference, the validator tries: (1) exact `__id` match, (2) `__local_id` fallback. A `broken_link` is only produced when both fail. An `ambiguous_reference` is produced when multiple candidates match at any step.

**Error object fields:** `type`, `message`, `file`, `line`, `objectId`, `fieldName`, `reference`, `candidates`, `severity`

**Exit code:** 0 if no errors, 1 if errors exist.

### Syntax [[syntax: text]]

```bash
qmdc workspace validate <path>
```

### Options [[options: text]]

| Option | Short | Description | Type | Required |
|--------|-------|-------------|------|----------|
| `<path>` | | Path to workspace directory | path | yes |

### Examples [[examples: text]]

```bash example
# Validate workspace (returns JSON array of errors)
qmdc workspace validate ./my-project

# If no errors — returns empty array
[]

# If errors exist — returns array of error objects
[
  {
    "type": "broken_link",
    "message": "Object 'xyz' not found",
    "file": "file.qmd.md",
    "line": 5,
    "objectId": "abc",
    "reference": "[[#xyz]]",
    "severity": "error"
  }
]
```
