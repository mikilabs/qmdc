# Dynamic Block [[dynamic_block: SyntaxConcept]]

- depends: [[#workspace]], [[#object]]

## Description [[description: text]]

Dynamic blocks execute SQL queries against workspace data and display results in documents. They are fenced code blocks with types like `table`, `diagram`, or `chart`, containing SQL queries or references to Query objects.

The parser stores code block metadata in `__code_fences` on `__TextBlock` objects. The workspace is loaded into an in-memory SQLite database for query execution.

## Syntax [[syntax: text]]

Query objects define reusable SQL queries:

```markdown example
## Get Tables [[get_tables: Query]]
- sql: SELECT __id, __label FROM objects WHERE __kind = 'Table'
```

Reference to a Query object (inside a `table` code block):

```example
query: [[#get_tables]]
```

Inline SQL (inside a `table` code block):

```example
sql: SELECT __id, __kind FROM objects
```

Scope parameter:

- `scope: workspace` (default) — auto-filters by current workspace
- `scope: all` — no workspace filtering, returns data from all workspaces

## Renderers [[renderers: text]]

| Type | Description |
|------|-------------|
| `table` | HTML table (implemented) |
| `diagram` | D2/Mermaid (future) |
| `chart` | Charts (future) |

Unknown type → raw YAML output.

## Rules [[rules: text]]

- Dynamic blocks live inside `__TextBlock` objects as fenced code blocks
- Metadata is stored in `__code_fences` (lang, offset_line, length_lines)
- Query objects are regular QMD.md objects with a `sql` field
- The `scope: workspace` default ensures data isolation between workspaces
