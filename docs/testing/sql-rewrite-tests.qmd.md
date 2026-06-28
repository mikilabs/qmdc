# SQL Rewrite Tests [[suite_sql_rewrite_tests: TestSuite]]

- location: tests/sql/rewrite/
- format: NNN-name.json with input, workspace, expected, expected_result
- test_count: 25
- implementations: [sql_rewrite.rs]
- is_data_driven: true
- tests: [[#lsp:cmd_run_sql_query]]

## Description [[description: text]]

Tests for SQL AST rewrite — automatic injection of `__workspace` filters into SQL queries.

Use SQL rewrite tests when:

- Verifying that SQL is correctly rewritten (AST rewrite)
- Testing complex SQL constructs (JOIN, CTE, subqueries, window functions)
- Checking that rewrite preserves semantics (DISTINCT, LEFT JOIN, aggregates)
- Validating that rewritten SQL executes correctly

Do not use for workspace isolation checks (use SQL workspace tests), LSP integration (use LSP SQL integration tests), or QMDC parsing (use parser microtests).

## Testing Logic

### Two levels of verification

1. **String comparison** — checks that SQL is correctly rewritten. Compares `expected` SQL with the rewrite result.

2. **Execution validation** — checks that rewritten SQL works. If `expected_result` is specified, the SQL is executed against a test database populated from `multi-workspace-isolation/`.

### Test database

Uses `parse_all_workspaces()` to load data from:

- `tests/sql/multi-workspace-isolation/workspace1/` (ws1: 1 Feature)
- `tests/sql/multi-workspace-isolation/workspace2/` (ws2: 2 Features)

## Test Format

```json
{
  "input": "SELECT COUNT(*) FROM objects",
  "workspace": "ws1",
  "expected": "SELECT COUNT(*) FROM objects o WHERE o.__workspace = 'ws1'",
  "expected_result": {
    "columns": ["cnt"],
    "rows": [[1]]
  }
}
```

- `input` — original SQL query
- `workspace` — workspace ID for filtering
- `expected` — expected rewritten SQL
- `expected_result` (optional) — expected execution result

## Critical Checks

### Semantics preservation

Rewrite must NOT change: `DISTINCT` in aggregates, aggregate type, `GROUP BY` logic, `LEFT JOIN` semantics (RLS in ON, not WHERE).

### RLS rules

- All `objects` and `edges` tables get a `alias.__workspace = 'ws1'` filter
- Aliases are mandatory
- `LEFT JOIN` → RLS in `ON` clause
- `INNER JOIN` → RLS in `ON` clause

### Supported constructs

SELECT, UNION, EXCEPT, INTERSECT, subqueries (recursive), CTE (WITH, WITH RECURSIVE), JOIN (INNER, LEFT/RIGHT/FULL OUTER), window functions (OVER), CASE, IN, EXISTS, BETWEEN, LIKE.

## How to Add a Test

1. Create `NNN-name.json` in `tests/sql/rewrite/`
2. Specify `input`, `workspace`, `expected`
3. Optionally add `expected_result` for execution validation
4. Run: `cd qmdc-rs && cargo test --test sql_rewrite`
