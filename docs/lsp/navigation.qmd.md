# Navigation [[lsp_navigation: Category]]

Navigation features for moving through the workspace.

## Go to Definition [[definition: LSPFeature]]

Navigates to the definition of an object from a reference. Trigger: F12 or Ctrl+Click.

- lsp_method: textDocument/definition
- status: implemented
- depends: [[#parsers:rust_parser]]

### Description [[description: text]]

**Supported patterns:**

- `[[#id]]` — jump to the object with `__id: id`
- `[[#id]]` via `__local_id` fallback — jump to the object where `__local_id: id` (when no direct `__id` match exists and the match is unambiguous)
- `[[#namespace:id]]` — jump to the object in the specified namespace
- `[[#Kind:id]]` — jump to the object with the specified Kind

**Behavior:**

1. Cursor on reference `[[#users]]`
2. F12 or Ctrl+Click
3. Navigates to heading `## Users [[users]]`

For hierarchical IDs: cursor on `[[#alice]]` where `__id` is `team.members.alice` — navigates to `#### Alice [[alice]]` via `__local_id` fallback.

## Find References [[references: LSPFeature]]

Find All References — finds all places that reference a given object. Trigger: Shift+F12.

- lsp_method: textDocument/references
- status: implemented
- depends: [[#parsers:rust_parser]]

### Description [[description: text]]

**Behavior:**

1. Cursor on object heading `## Users [[users]]`
2. Shift+F12
3. Shows a list of all `[[#users]]` across the entire workspace

Membership is decided by **resolved identity**, not naive string equality: each reference is resolved through the shared `core::resolve` index (handling `ns:id`, hierarchical ids, and `__local_id`), so the LSP and the MCP [[#mcp:tool_find_references]] tool agree on what counts as a reference.

## Document Symbol [[document_symbol: LSPFeature]]

Outline — document structure. Shows the object tree in a file. Used by the standard Outline panel in editors.

- lsp_method: textDocument/documentSymbol
- status: implemented
- depends: [[#parsers:rust_parser]]

### Description [[description: text]]

Each object becomes a DocumentSymbol:

- name: `__label` or `__id`
- kind: SymbolKind.Class (for objects) or .Field (for fields)
- detail: `__kind` if present
- range: from heading to the next object
- children: nested objects and fields

**Example output:**

```text
📦 Users (Table)
  └─ 📄 columns
      ├─ id
      ├─ email
      └─ name
📦 Orders (Table)
  └─ 📄 foreign_keys
```

## Workspace Symbol [[workspace_symbol: LSPFeature]]

Search for objects across the entire workspace. Trigger: Ctrl+T or Ctrl+P with # prefix.

- lsp_method: workspace/symbol
- status: implemented
- depends: [[#parsers:rust_parser]]

### Description [[description: text]]

**Filtering:**

- `#users` — search by id/label
- `#Table:` — all objects with Kind=Table
- `#storage:` — all objects in namespace storage

**Behavior:**

1. Ctrl+T
2. Type `users`
3. Shows all objects containing "users" in id/label
