# Visual Features [[lsp_visual: Category]]

Visual LSP features for improving the editor UX.

## Folding Range [[folding_range: LSPFeature]]

Folding objects and sections in the editor.

- lsp_method: textDocument/foldingRange
- status: planned
- depends: [[#parsers:rust_parser]]

### Description [[description: text]]

**What can be folded:**

- Objects (from heading to the next heading at the same or higher level)
- Array sections `[[field: [Kind]]]`
- Multiline fields `[[field: text]]`
- Code blocks (``` ... ```)

**Example:**

```markdown
## Users [[users: Table]]  ▼
  - name: users
  - columns: [...]
  
## Orders [[orders: Table]]  ▼
  - name: orders
```

After folding:

```markdown
## Users [[users: Table]]  ▶
## Orders [[orders: Table]]  ▶
```

## Document Link [[document_link: LSPFeature]]

Clickable `[[#id]]` links in the document.

- lsp_method: textDocument/documentLink
- status: done
- depends: [[#parsers:rust_parser]]

### Description [[description: text]]

- All `[[#...]]` become clickable
- Ctrl+Click → go to definition
- Hover → shows a tooltip with object information

## Semantic Tokens [[semantic_tokens: LSPFeature]]

Smart syntax highlighting based on semantics.

- lsp_method: textDocument/semanticTokens
- status: planned
- depends: [[#parsers:rust_parser]]

### Description [[description: text]]

**Token types:**

| Token | What it highlights |
|-------|-------------------|
| `namespace` | `[[#namespace:id]]` — the namespace part |
| `type` | Kind in `[[id: Kind]]` |
| `variable` | ID in `[[id]]` and `[[#id]]` |
| `property` | Field keys `- key: value` |
| `string` | String values |
| `number` | Numeric values |
| `keyword` | `true`, `false`, `null` |

**Example:**

```markdown
## User [[user: User]]
     ^^^^  ^^^^  ^^^^
     var   var   type

- name: "John"
  ^^^^  ^^^^^^
  prop  string
```

## Inlay Hint [[inlay_hint: LSPFeature]]

Inline hints in the code.

- lsp_method: textDocument/inlayHint
- status: planned
- depends: [[#parsers:rust_parser]]

### Description [[description: text]]

**Use cases:**

- Show Kind for objects without an explicit type
- Show element count in an array
- Show resolved value of a reference

**Example:**

```markdown example
## User [[user]]        ← : __Object (inlay hint)
- items: [[#stages]]    ← : Stage[] (3 items) (inlay hint)
```
