# Refactoring [[mcp_refactoring: Category]]

Tools that propose edits to the workspace. They never write files — they return a diff for the agent to apply.

## Rename Object [[tool_rename_object: McpTool]]

Preview a workspace-wide rename as a diff.

- tool_name: qmdc_rename_object
- status: implemented
- args: path, old_id, new_id

### Description [[description: text]]

Returns the exact text edits — `{file, line, old_text, new_text, kind}` for the definition and every reference — to rename `old_id` to `new_id` across the whole indexed workspace. The agent applies the edits itself; the tool **never writes to disk** (INV-3).

The rename **cascades to descendants**: renaming `team` also rewrites `[[#team.config]]` and `[[#team.members.alice]]`. Descendant *definitions* need no edit — they are anchored by `__local_id` and the parser recomposes the hierarchical id from the renamed parent.

This is the same transport-agnostic planner used by the LSP [[#lsp:rename]] feature and the CLI, so all three produce identical edit sets. `new_id` must be alphanumeric plus dash, underscore, or dot. Edits are bounded (see [[#mcp_bounded]]).

### Example [[example: text]]

```json example
{
  "old_id": "user",
  "new_id": "customer",
  "edit_count": 3,
  "truncated": false,
  "edits": [
    { "file": "orders.qmd.md", "line": 3, "old_text": "[[#user]]", "new_text": "[[#customer]]", "kind": "reference" },
    { "file": "orders.qmd.md", "line": 5, "old_text": "[[#user]]", "new_text": "[[#customer]]", "kind": "reference" },
    { "file": "readme.qmd.md", "line": 3, "old_text": "[[user]]", "new_text": "[[customer]]", "kind": "definition" }
  ]
}
```
