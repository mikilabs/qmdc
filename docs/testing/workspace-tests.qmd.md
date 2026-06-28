# Workspace Tests [[suite_workspace_tests: TestSuite]]

- location: tests/workspace/
- format: Directory with readme.qmd.md + _expected.json
- test_count: 12
- implementations: [test_workspace.py, test-workspace.ts, workspace_conformance.rs]
- is_data_driven: true
- tests: [[#format]], [[#parsers]]

## Description [[description: text]]

Multi-file scenarios — verifying workspace behavior with multiple files, cross-file references, and validation.

Use workspace tests when:

- Verifying multi-file behavior
- Testing cross-file references
- Checking workspace validation (broken links, duplicate IDs)
- Testing nested workspaces
- Verifying `.qmdcignore` functionality

Do not use for single-file parsing (use parser microtests), LSP features (use LSP microtests), or SQL queries (use SQL tests).

## How to Add a New Test

### Step 1: Create a workspace directory

Create `tests/workspace/test-name/`:

```text
simple-workspace/
├── readme.qmd.md
├── users.qmd.md
├── orders.qmd.md
└── _expected.json
```

### Step 2: Create readme.qmd.md with __Workspace

```markdown
# My Workspace [[my_ws:__Workspace]]
```

### Step 3: Create files with objects

### Step 4: Create _expected.json

```json
{
  "workspace_id": "my_ws",
  "files": ["readme.qmd.md", "users.qmd.md", "orders.qmd.md"],
  "objects": {
    "__Workspace": ["my_ws"],
    "": ["alice", "bob", "order_1", "order_2"]
  },
  "errors": []
}
```

### Step 5: Run tests

```bash
make test
```

## _expected.json Format

| Field | Description |
|-------|-------------|
| `workspace_id` | ID of the workspace object (from `[[id:__Workspace]]`) |
| `files` | List of files in the workspace (relative paths) |
| `objects` | Objects grouped by Kind (key = `__kind`, value = array of `__id`) |
| `errors` | Validation errors: `type`, `object`, `reference`, `file`, `line`, `candidates` |
| `nested_workspaces` | Nested workspaces (optional) |

## Error Types

- **broken_link** — reference to a non-existent object
- **duplicate_id** — two objects with the same ID
- **ambiguous_ref** — ambiguous reference (multiple objects with different Kind)

## Nested Workspaces

For testing nested workspaces, create a subdirectory with its own `readme.qmd.md` containing a `__Workspace` declaration. Add `nested_workspaces` to `_expected.json`.

## .qmdcignore

For testing `.qmdcignore`, create a `.qmdcignore` file in the workspace directory. Files matching the patterns will be excluded from `files` in `_expected.json`.
