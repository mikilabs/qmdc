# LSP SQL Integration Tests [[suite_lsp_sql_tests: TestSuite]]

- location: tests/lsp/sql/
- format: NNN-name/ with workspace/, request.json, expected.json
- test_count: 3
- implementations: [lsp.rs, mcp.rs]
- is_data_driven: true
- tests: [[#lsp:cmd_run_sql_query]]

## Description [[description: text]]

Tests for the LSP command `qmdc.runSqlQuery` with SQL rewrite applied for workspace isolation.

Use LSP SQL integration tests when:

- Verifying that SQL rewrite works through the LSP command `qmdc.runSqlQuery`
- Testing workspace isolation via LSP (different workspaces see different data)
- Checking that `documentUri` and `scope` parameters are handled correctly
- Testing complex SQL queries (JOIN, CTE, subqueries) through LSP

Do not use for QMD.md syntax parsing (use parser microtests), regular LSP capabilities (use LSP microtests), or SQL queries without LSP (use SQL workspace tests).

## How to Add a New Test

### Step 1: Create the test directory

Create `tests/lsp/sql/NNN-name/`:

```text
001-workspace-filter/
├── workspace/
│   ├── readme.qmd.md
│   └── data.qmd.md
├── request.json
└── expected.json
```

### Step 2: Create the workspace

Populate `workspace/` with QMD.md files containing a `__Workspace` declaration and test objects.

### Step 3: Create request.json

```json
{
  "command": "qmdc.runSqlQuery",
  "arguments": [
    "SELECT COUNT(*) as cnt FROM objects WHERE __kind = 'Feature'",
    "workspace/data.qmd.md",
    "workspace"
  ]
}
```

Arguments: `[0]` SQL query, `[1]` documentUri (optional), `[2]` scope — `"workspace"` (default) or `"all"`.

### Step 4: Create expected.json

```json
{
  "success": true,
  "columns": ["cnt"],
  "rows": [[1]]
}
```

### Step 5: Run tests

```bash
cd qmdc-rs && cargo test --test lsp --test mcp
```

## SQL Rewrite

The LSP command `qmdc.runSqlQuery` automatically applies SQL rewrite for workspace isolation:

1. Determines the workspace from `documentUri`
2. Rewrites SQL by adding `__workspace = 'workspace_id'` filters
3. Executes the query with applied filters

## Workspace Isolation

Each workspace sees only its own objects:

- **001-workspace-filter**: workspace `ws1` → 1 Feature
- **002-multi-workspace**: workspace `ws2` → 2 Features

## Scope Parameter

- `"workspace"` (default) — SQL rewrite is applied
- `"all"` — rewrite is not applied, all workspaces are visible
