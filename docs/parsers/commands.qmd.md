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

**Round-trip guarantee:** parse → rebuild restores the original document, and re-parsing the
result always yields the same graph. Text placement has known limits: content that follows a
heading-declared array's own table is re-anchored on the field before that heading, so it can move
above the array heading on rebuild, and a top-level array is rebuilt as a wrapper heading plus a
nested array heading. The graph is unchanged in both cases; only the layout is.

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

Reads JSON objects from a file or stdin and outputs QMD.md to a file or stdout. Round-trips a parsed document back to QMD.md — see the round-trip note above for the two shapes where the layout can shift while the graph stays identical. Preserves object and field order. Restores hierarchy from __parent and__level.

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
qmdc query -w <path> -w <path> "<sql>"
```

### Options [[options: text]]

| Option | Short | Description | Type | Required |
|--------|-------|-------------|------|----------|
| `<path>` | | Path to workspace directory | path | no |
| `<query>` | | SQL query or reference to a Query object (`#query_id`) | string | yes |
| `--with` | `-w` | Compose this workspace explicitly; repeatable. Mutually exclusive with `<path>` | path | no |
| `--format` | | Output format: table (default), json | enum | no |

### Examples [[examples: text]]

```bash
# SQL query against workspace
qmdc query ./my-project "SELECT __id, __kind, __label FROM objects WHERE __kind = 'Service'"

# JSON output format
qmdc query ./my-project "SELECT * FROM objects LIMIT 10" --format json

# Query via Query object (reference to [[id:Query]] in workspace)
qmdc query ./my-project "#all_services"

# Compose workspaces at unrelated paths (the only positional left is the query)
qmdc query -w ~/checkouts/repo_a -w /srv/repo_b "SELECT * FROM edges"

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
qmdc workspace parse -w <path> -w <path>
```

### Options [[options: text]]

| Option | Short | Description | Type | Required |
|--------|-------|-------------|------|----------|
| `<path>` | | Path to workspace directory (defaults to `.`) | path | no |
| `--with` | `-w` | Compose this workspace explicitly; repeatable. Mutually exclusive with `<path>` | path | no |
| `--output` | `-o` | Output JSON file (Python/TypeScript) | path | no |
| `--format` | | Output format: minimal, standard, full (Rust only) | enum | no |

### Examples [[examples: text]]

```bash
# Python/TypeScript
qmdc workspace parse ./my-project -o workspace.json

# Rust (output to stdout)
qmdc workspace parse ./my-project > workspace.json

# Compose two workspaces that do not share a parent directory
qmdc workspace parse -w ~/checkouts/repo_a -w /srv/repo_b

# With format selection (Rust only)
qmdc workspace parse ./my-project --format full
```

### Output [[output: text]]

One JSON object of the same shape for every invocation:

```json
{
  "root": "/home/me/checkouts/shop",
  "workspaces": [
    {"id": "shop", "root": "/home/me/checkouts/shop", "path": ""}
  ],
  "files": ["readme.qmd.md", "storage/tables.qmd.md"],
  "objects": [],
  "errors": []
}
```

- `workspaces` is always present: every workspace in the result, ordered by `path`. `root` is where
  the workspace is on disk, as a canonical absolute path; `path` is where its files sit in `__file`.
- `root` at the top level is the directory every `__file` is relative to, or `null` when the
  invocation composed `-w` paths. Those need not share any directory, so each workspace sits at its
  own id instead: `-w ~/a/shop -w /srv/billing` gives `__file` values like `shop/storage/tables.qmd.md`
  and `billing/readme.qmd.md`, the same wherever the checkouts are.
- To open the file an object came from: take the entry whose `path` is the longest leading directory
  of the object's `__file` (`repo_a` leads `repo_a/x.qmd.md`, not `repo_ab/x.qmd.md`), and join that
  entry's `root` with the rest of `__file`. For a single workspace `path` is `""`, which leads every
  file, so this is simply `root` + `__file`.

Given a directory holding several workspaces, each sits at its directory under that container
(`path: "repo_a"`), and a file outside every workspace has no entry: it is located from the
top-level `root`.

An entry of `errors` carries `type`, `message`, `file`, `line`, `objectId`, `fieldName`, `reference`,
`candidates` and `severity`, omitting whatever is absent — the SAME key names `workspace validate`
uses, so one error reads the same whichever command produced it. The names `object` and `field` were
this envelope's own spelling before QMD-77 and are gone.

## Workspace Validate [[cmd_workspace_validate: Command]]

Validates workspace: checks for broken links, duplicate IDs, ambiguous references.

- parser: [[#python_parser]], [[#typescript_parser]], [[#rust_parser]]

### Description [[description: text]]

Returns **only a JSON array of errors** (empty array `[]` if no errors).

Available in all three parsers (Python, TypeScript, Rust).

**Error types:**

- `broken_link` — reference `[[#id]]` to a non-existent object (after both `__id` and `__local_id` fallback lookups fail)
- `duplicate_id` — two objects with the same full hierarchical `__id`
- `ambiguous_reference` — reference that could point to multiple objects (by `__id` Kind collision or by multiple `__local_id` matches)
- `broken_parent` — dot-ID declaration (`[[parent.child]]`) whose parent object does not exist in the workspace
- `ambiguous_field_reference` — dot-path resolves both as an object `__id` and as a field on the prefix object
- `nested_workspace` — workspace inside another workspace (forbidden)
- `workspace_in_wrong_file` — workspace declaration in wrong file

Parse-stage errors (`invalid_id_character`, `mixed_field_keys`, `nested_subitems`, ...) surface as `__ParsingError` objects — the full catalog is in the validation-errors reference.

**Resolution order:** for each reference, the validator tries: (1) exact `__id` match, (2) `__local_id` fallback. A `broken_link` is only produced when both fail. An `ambiguous_reference` is produced when multiple candidates match at any step.

**Error object fields:** `type`, `message`, `file`, `line`, `objectId`, `fieldName`, `reference`, `candidates`, `severity`

**Exit code:** 0 if no errors, 1 if errors exist, 2 if the invocation itself was refused
(a usage error, e.g. a positional path together with `--with`).

### Syntax [[syntax: text]]

```bash
qmdc workspace validate <path>
qmdc workspace validate -w <path> -w <path>
```

### Options [[options: text]]

| Option | Short | Description | Type | Required |
|--------|-------|-------------|------|----------|
| `<path>` | | Path to workspace directory (defaults to `.`) | path | no |
| `--with` | `-w` | Compose this workspace explicitly; repeatable. Mutually exclusive with `<path>` | path | no |

### Examples [[examples: text]]

```bash example
# Validate workspace (returns JSON array of errors)
qmdc workspace validate ./my-project

# Validate a project made of workspaces at unrelated paths
qmdc workspace validate -w ~/checkouts/repo_a -w /srv/repo_b

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
