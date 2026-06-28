# Refactoring [[lsp_refactoring: Category]]

Code refactoring features.

## Rename [[rename: LSPFeature]]

Renames an object with automatic update of all references. Trigger: F2.

- lsp_method: textDocument/rename
- status: partial
- depends: [[#parsers:rust_parser]]

### Description [[description: text]]

**Behavior:**

1. Cursor on `## Users [[users]]`
2. F2 → type `customers`
3. Automatically:
   - `[[users]]` → `[[customers]]`
   - All `[[#users]]` in workspace → `[[#customers]]`

**Shared rename planner:** the LSP, the CLI, and the MCP [[#mcp:tool_rename_object]] tool all compute the edit set through one transport-agnostic core planner. The plan is **identity-resolved** (a reference matches by resolved object, not by raw string) and **cascades to descendants** — renaming `team` also rewrites `[[#team.config]]` and `[[#team.members.alice]]`. Descendant *definitions* need no edit (they are anchored by `__local_id`). The planner is **diff-only**: it never writes files, it returns the proposed edits.

**Limitation:** the LSP `textDocument/rename` projects the plan onto currently open buffers, so references in unopened files may be missed. The MCP `qmdc_rename_object` tool runs the same planner across the whole indexed workspace.

**Validation before rename:**

- New id is valid (alphanumeric, dash, underscore, dot)

## Code Action [[code_action: LSPFeature]]

Quick fixes and refactorings. The lightbulb icon in the editor.

- lsp_method: textDocument/codeAction
- status: partial
- depends: [[#parsers:rust_parser]]

### Description [[description: text]]

**Implemented actions:**

**Use full hierarchical ID** — for broken link resolvable via `__local_id`:

```markdown example
[[#child]] → [[#parent.child]]
```

When a broken reference matches exactly one object's `__local_id`, the quick fix replaces it with the full `__id`.

**Planned actions:**

**Add namespace qualifier** — for ambiguous reference:

```markdown example
[[#users]] → [[#storage:users]]
```

**Add Kind qualifier** — for ambiguous reference:

```markdown example
[[#users]] → [[#Table:users]]
```

**Create missing object** — for broken link:

```markdown example
- ref: [[#missing]]
       ^^^^^^^^^^^  ← Quick fix: Create object 'missing'
```

Creates a heading `## Missing [[missing]]` at the end of the file.

**Extract to file** — for a selected object:

- Create a new file
- Move the object
- Add a reference in the source file
