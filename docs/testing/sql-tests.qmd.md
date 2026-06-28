# SQL Workspace Tests [[suite_sql_tests: TestSuite]]

- location: tests/workspace/
- format: Workspace with tests/*.sql + tests/*.expected.json
- test_count: 41
- implementations: [test_sql.py, test-sql.ts, sql.rs]
- is_data_driven: true
- tests: [[#format]], [[#parsers]]

## Description [[description: text]]

SQL queries against workspace — verifying that the SQLite database is correctly populated and queries return expected results.

Use SQL tests when:

- Verifying that a workspace loads correctly into SQLite
- Testing SQL queries against objects
- Checking edges (references between objects)
- Testing filtering by `__kind`, `__namespace`, `__workspace`
- Verifying JSON extraction from the `data` column

Do not use for QMD.md syntax parsing (use parser microtests), LSP features (use LSP microtests), SQL rewrite logic (use SQL rewrite tests), or workspace validation (use workspace tests).

## How to Add a New Test

Tests are automatically discovered by test runners only if they are in scanned directories:

- `tests/workspace/` — primary path
- `tests/workspace/` — alternative path

Do not create tests elsewhere — they will not run automatically.

### Step 1: Create a workspace with a tests/ directory

```text
tests/workspace/my-workspace/
├── readme.qmd.md
├── users.qmd.md
└── tests/
    ├── 001-count-users.sql
    └── 001-count-users.expected.json
```

### Step 2: Create the SQL query

`tests/001-count-users.sql`:

```sql
SELECT COUNT(*) as count
FROM objects
WHERE __kind = 'User'
```

### Step 3: Create the expected result

`tests/001-count-users.expected.json`:

```json
{
  "columns": ["count"],
  "rows": [[2]]
}
```

### Step 4: Run tests

```bash
make test
```

## expected.json Format

```json
{
  "columns": ["__id", "name", "age"],
  "rows": [
    ["alice", "Alice", 30],
    ["bob", "Bob", 25]
  ]
}
```

- `columns` — array of column names (same order as SELECT)
- `rows` — array of rows, each row is an array of values

## SQLite Schema

### Table: objects

| Column | Type | Description |
|--------|------|-------------|
| `__workspace` | TEXT NOT NULL | Workspace ID |
| `__namespace` | TEXT NOT NULL DEFAULT '' | Namespace ID |
| `__id` | TEXT NOT NULL | Unique identifier within workspace/namespace |
| `__global_id` | TEXT GENERATED STORED UNIQUE | Globally unique: `workspace:namespace:id` |
| `__kind` | TEXT | Object type (Kind) |
| `__label` | TEXT | Human-readable name |
| `__file` | TEXT | File path |
| `__parent` | TEXT | Parent object ID |
| `__line` | INTEGER | Line number in file |
| `__level` | INTEGER | Heading level (1–6) |
| `data` | TEXT | JSON with user fields |

Primary key: `(__workspace, __namespace, __id)`.

### Table: edges

| Column | Type | Description |
|--------|------|-------------|
| `source_id` | TEXT NOT NULL | `__global_id` of source object |
| `source_field` | TEXT NOT NULL | Field name containing the reference |
| `target_id` | TEXT NOT NULL | `__global_id` of target object |
| `__workspace` | TEXT | Workspace ID |

## JSON Extraction

Use `json_extract` for the `data` column:

```sql
SELECT json_extract(data, '$.name') as name FROM objects
SELECT json_extract(data, '$.address.city') as city FROM objects
```
