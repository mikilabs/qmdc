# LSP Commands [[lsp_commands: Category]]

Custom LSP server commands. These commands execute on the LSP server side and can be invoked programmatically via the LSP protocol.

## Dump Index [[cmd_dump_index: Command]]

Outputs the workspace index contents for debugging.

- parser: [[#parsers:rust_parser]]

### Description [[description: text]]

Displays:

- List of all workspaces with their root paths
- Number of files in each workspace
- List of all objects with their Kind and location
- Open documents in the editor

Useful for:

- Debugging workspace indexing issues
- Verifying that objects are correctly recognized
- Diagnosing broken links

### Syntax [[syntax: text]]

```bash
# VS Code Command Palette:
# Ctrl+Shift+P → "QMDC: Dump Index"
# Result displayed in the Output panel
```

### Examples [[examples: text]]

```text
=== QMDC Workspace Index ===

Workspace: 'docs'
  Root: /path/to/project/docs
  Files: 25
  Objects (142):
    - users [Table] in storage/tables.qmd.md
    - orders [Table] in storage/tables.qmd.md
    - completion [LSPFeature] in lsp/completion.qmd.md
    ...

=== Open Documents ===

Document: file:///path/to/file.qmd.md
  Objects: 5
    - my_object [Component]
    - another_object [Service]
```

## Get Workspace Tree [[cmd_get_workspace_tree: Command]]

Returns the workspace object tree for UI display. Used by the VS Code extension for the Objects Explorer.

- parser: [[#parsers:rust_parser]]

### Description [[description: text]]

Returns a tree of workspace objects grouped by the specified mode.

### Syntax [[syntax: text]]

```typescript
const tree = await client.sendRequest('workspace/executeCommand', {
  command: 'qmdc.getWorkspaceTree',
  arguments: ['namespace']  // mode: 'namespace' | 'file' | 'smart'
});
```

### Options [[options: text]]

| Parameter | Type | Description |
|-----------|------|-------------|
| `mode` | string | Grouping mode: `namespace` (by namespace), `file` (by files), `smart` (smart parent-child hierarchy) |

### Examples [[examples: text]]

```json
{
  "nodes": [
    {
      "id": "users",
      "label": "Users",
      "kind": "Table",
      "file": "storage/tables.qmd.md",
      "line": 15,
      "children": []
    }
  ]
}
```

## Run SQL Query [[cmd_run_sql_query: Command]]

Executes a SQL query against the workspace database. Used by the VS Code extension for "Run SQL Query" and "Run Query from Block" commands.

- parser: [[#parsers:rust_parser]]

### Description [[description: text]]

Executes a SQL query against the workspace SQLite database and returns the results.

### Syntax [[syntax: text]]

```bash
# VS Code Command Palette:
# Ctrl+Shift+P → "QMDC: Run SQL Query"
# Enter SQL query
# Result displayed in the Output panel
```

### Options [[options: text]]

| Parameter | Type | Description |
|-----------|------|-------------|
| `query` | string | SQL query to execute |

### Examples [[examples: text]]

```sql
-- All objects of type LSPFeature
SELECT __id, __label FROM objects WHERE __kind = 'LSPFeature'

-- Incoming references to an object
SELECT source_id, source_field FROM edges WHERE target_id = 'users'

-- Statistics by Kind
SELECT __kind, COUNT(*) as count FROM objects GROUP BY __kind
```

Response format:

```json
{
  "columns": ["__id", "__label"],
  "rows": [
    ["completion", "Completion"],
    ["hover", "Hover"],
    ["definition", "Go to Definition"]
  ]
}
```
