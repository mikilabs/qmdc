# Query [[mcp_query: Category]]

Tools that read the in-memory index directly — SQL over the workspace and a raw index dump.

## Query SQL [[tool_query_sql: McpTool]]

Run a read-only SQL SELECT over the workspace index.

- tool_name: qmdc_query_sql
- status: implemented
- args: path, sql

### Description [[description: text]]

Runs `sql` against the in-memory SQLite index built from the workspace. Two tables are available:

- `objects(__id, __kind, __label, __namespace, __file, __line, __parent, __level, data)`
- `edges(source_id, target_id, edge_type, source_field)`

Call [[#tool_describe_metamodel]] first to learn the kinds and fields. Passing `#query_id` runs the `sql` field of a stored `Query` object (parity with the LSP [[#lsp:cmd_run_sql_query]] command).

### Read-Only Enforcement [[query_read_only: text]]

Only SELECT-class statements are admitted (INV-2). The statement is parsed and checked against an allowlist before execution; anything else is rejected with a stable, content-free `not-read-only` code. As defense-in-depth the connection runs under SQLite `PRAGMA query_only` for the duration of the read. See [[#mcp_security]].

### Example [[example: text]]

```sql
-- Count objects by kind
SELECT __kind, COUNT(*) AS n FROM objects GROUP BY __kind ORDER BY n DESC

-- Outgoing edges of an object
SELECT target_id, edge_type FROM edges WHERE source_id = 'users'
```

Results are returned as bounded rows (see [[#mcp_bounded]]).

## Dump Index [[tool_dump_index: McpTool]]

Dump the entire parsed index as JSON.

- tool_name: qmdc_dump_index
- status: implemented
- args: path

### Description [[description: text]]

Debug-only: returns all objects and files in the index. Large and token-heavy — prefer [[#tool_query_sql]], [[#tool_get_tree]], or [[#tool_describe_object]] for targeted reads. Output carries truncation metadata (see [[#mcp_bounded]]).
