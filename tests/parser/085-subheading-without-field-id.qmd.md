## Built-in types [[ctx_builtin_types: CompletionContext]]

Autocompletion of built-in type hints for fields.

- capability: [[#completion]]
- trigger: "[[field: "
- suggestions: [text, object, array, yaml]

### Built-in types

| Type | Description |
|-----|----------|
| `text` | Multiline text |
| `object` | Nested object |
| `array` | Array (with subheadings or a list) |
| `yaml` | YAML block |

### Example

```markdown
- description: [[description: |]]
                              ↑ Ctrl+Space
```
