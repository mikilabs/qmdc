# Information [[lsp_information: Category]]

Features for displaying information about objects.

## Hover [[hover: LSPFeature]]

Shows object information when hovering over a reference.

- lsp_method: textDocument/hover
- status: implemented
- depends: [[#parsers:rust_parser]]

### Description [[description: text]]

**Output format:**

```markdown
**{__label}** `{__kind}`

🔍 `{__global_id}`
📁 {__file}

- {field1}: {value1}
- {field2}: {value2}
```

**Contexts:**

- Object reference: `[[#id]]`, `[[#namespace:id]]`, `[[#Kind:id]]`
- Kind in heading: `[[id: Kind]]` (if a schema exists)

**Example:**

When hovering over `[[#users]]`:

```text
**Users** `Table`

🔍 `storage::users`
📁 storage/tables.qmd.md

- name: users
- schema: public
```
