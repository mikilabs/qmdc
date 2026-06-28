# Formatting [[lsp_formatting: Category]]

Document formatting features.

## Formatting [[formatting: LSPFeature]]

Formats a document into canonical form. Equivalent to `qmdc lint`.

- lsp_method: textDocument/formatting
- status: planned
- depends: [[#parsers:rust_parser]]

### Description [[description: text]]

**What gets formatted:**

- Consistent headings: `## Label [[id]]` or `## Label [[id: Kind]]`
- Normalized field lists
- Uniform indentation and spacing
- Blank lines between objects

**Before formatting:**

```markdown
##User[[user:User]]
-name:John
- email:  john@example.com
```

**After formatting:**

```markdown
## User [[user: User]]

- name: John
- email: john@example.com
```
