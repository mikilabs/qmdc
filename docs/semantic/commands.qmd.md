# Semantic CLI Commands [[semantic_commands: NarrativeDoc]]

- about: [[#semantic_commands.cmd_sem_index]], [[#semantic_commands.cmd_sem_search]]

Reference for `qmdc-semantic` CLI commands.

## index [[cmd_sem_index: Command]]

Index a QMDC workspace for semantic search.

- parser: qmdc-semantic

### Description [[description: text]]

Parses all QMD.md files in the workspace, chunks objects, computes embeddings, and stores them in `.qmdc-semantic/embeddings.db`. Uses hash-based diffing for incremental updates — only new or changed chunks are re-embedded.

### Syntax [[syntax: text]]

```bash
qmdc-semantic index [WORKSPACE_PATH] [OPTIONS]
```

| Argument | Description | Default |
|----------|-------------|---------|
| WORKSPACE_PATH | Path to workspace directory | `.` (current) |

### Options [[options: text]]

| Option | Description |
|--------|-------------|
| `--force, -f` | Reindex all chunks (ignore hash cache) |
| `--verbose, -v` | Show detailed progress |

### Examples [[examples: text]]

```bash
# Index current directory
qmdc-semantic index

# Index with verbose output
qmdc-semantic index ./my-workspace -v

# Force full reindex
qmdc-semantic index ./my-workspace --force
```

## search [[cmd_sem_search: Command]]

Search for relevant objects in a QMDC workspace.

- parser: qmdc-semantic

### Description [[description: text]]

Runs hybrid search (dense + FTS5) with graph walk expansion. Supports text queries and file-based queries (impact scan). Returns ranked objects with scores and text previews.

### Syntax [[syntax: text]]

```bash
qmdc-semantic search [WORKSPACE_PATH] [QUERY] [OPTIONS]
```

| Argument | Description | Default |
|----------|-------------|---------|
| WORKSPACE_PATH | Path to workspace directory | `.` (current) |
| QUERY | Text search query | (required unless --from-file) |

### Options [[options: text]]

| Option | Description | Default |
|--------|-------------|---------|
| `--from-file, -f FILE` | Use file content as query | - |
| `--top-k, -k N` | Number of results | 10 |
| `--depth, -d N` | Graph walk depth | 2 |
| `--exclude-ns, -x NS` | Exclude namespace(s) from results (repeatable) | - |
| `--verbose, -v` | Show detailed output | - |

### Examples [[examples: text]]

```bash
# Simple search
qmdc-semantic search . "how to test LSP"

# Impact scan from task file
qmdc-semantic search . --from-file tasks/QMD-41/QMD-41-task.qmd.md

# More results, deeper graph walk
qmdc-semantic search . "chunking algorithm" -k 20 --depth 3

# Exclude noisy namespaces from results
qmdc-semantic search . "validation rules" -x tracking -x ideas

# Verbose output with text preview
qmdc-semantic search . "storage schema" -v
```
