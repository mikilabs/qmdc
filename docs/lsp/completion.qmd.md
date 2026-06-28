# Completion [[lsp_completion: Category]]

Autocompletion — a key feature for productive QMDC editing. Groups the completion feature and its trigger contexts.

## Completion [[completion: LSPFeature]]

Autocompletion triggers on specific characters and suggests relevant options.

- lsp_method: textDocument/completion
- status: implemented
- depends: [[#parsers:rust_parser]]

### Description [[description: text]]

**Triggers:**

- `[` — start of a reference or heading anchor
- `#` — start of ID in a reference
- `:` — after Kind or namespace
- `.` — after Kind or namespace (dot notation)

**Usage examples:**

Creating an object with Kind:

```markdown
## My Service [[my_service: |]]
                              ↑ Ctrl+Space
Suggestions: Service, Component, Table, Config...
```

Reference to an object:

```markdown example
- database: [[#|]]
               ↑ Ctrl+Space  
Suggestions: users_db, orders_db, config...
```

Reference with namespace:

```markdown example
- table: [[#storage:|]]
                    ↑ Ctrl+Space
Suggestions: users, orders, products (from storage namespace)
```

See CompletionContext objects below for details on each context.

## Kind in Heading [[ctx_kind_header: CompletionContext]]

Autocompletion of Kind after the colon in an object heading.

- trigger: "[[id: "

### Suggestions [[suggestions: text]]

All known Kind values from the workspace (from existing objects) plus built-in type hints: `text`, `object`, `array`, `yaml`.

**Sorting:**

1. Kind from the current namespace (most relevant)
2. Kind from other namespaces
3. Built-in type hints

### Description [[description: text]]

**Pattern:**

```markdown
## User [[user: |]]
              ↑ cursor here
```

**Example:**

```markdown
## My Service [[my_service: |]]
                              ↑ Ctrl+Space

Suggestions:
  Service      (from current namespace)
  Component    (from architecture namespace)
  Table        (from storage namespace)
  text         (built-in)
```

## Kind in Reference [[ctx_kind_ref: CompletionContext]]

Autocompletion of ID after `Kind:` in a reference.

- trigger: "[[#Kind:"

### Suggestions [[suggestions: text]]

All objects with the specified Kind from the workspace.

**Sorting:**

1. Objects from the current namespace
2. Objects from other namespaces

### Description [[description: text]]

**Pattern:**

```markdown example
- table: [[#Table:|]]
                  ↑ cursor here
```

**Example:**

```markdown example
- table: [[#Table:|]]
                  ↑ Ctrl+Space

Suggestions:
  users        (storage namespace)
  orders       (storage namespace)
  products     (storage namespace)
```

## ID in Reference [[ctx_id_ref: CompletionContext]]

Autocompletion of ID after the hash in a reference.

- trigger: "[[#"

### Suggestions [[suggestions: text]]

All objects from the workspace (all `__id` values). Additionally, `__local_id` values are valid reference targets — short-form references like `[[#child]]` resolve via `__local_id` fallback when the local ID is unambiguous.

**Sorting:**

1. Objects from the current namespace (most relevant)
2. Objects from the current file
3. Objects from other namespaces
4. Alphabetical

**Filtering:** as you type, filters by substring: `[[#us|]]` → users, user_service, status

### Description [[description: text]]

**Pattern:**

```markdown example
- ref: [[#|]]
          ↑ cursor here
```

**Example:**

```markdown example
- ref: [[#|]]
          ↑ Ctrl+Space

Suggestions:
  users           (storage namespace)
  config          (current namespace)
  api_gateway     (architecture namespace)
  ...
```

## Namespace in Reference [[ctx_namespace_ref: CompletionContext]]

Autocompletion of namespace after `[[#` and before `:`.

- trigger: "[[#"

### Suggestions [[suggestions: text]]

All namespaces from the workspace (all unique `__namespace` values).

### Description [[description: text]]

**Behavior:**

1. User types `[[#stor`
2. Autocompletion suggests `storage`
3. After selection, `:` is added and cursor moves to ID autocompletion

**Pattern:**

```markdown example
- ref: [[#storage:|]]
              ↑ cursor here (after typing namespace)
```

**Example:**

```markdown example
- ref: [[#|]]
          ↑ type "stor"

Suggestions:
  storage:     (namespace)
  
After selection:
- ref: [[#storage:|]]
                  ↑ now autocompleting ID from storage namespace
```

## Field Key [[ctx_field_key: CompletionContext]]

Autocompletion of field key at the start of a line after `-`.

- trigger: "- "
- status: planned

### Suggestions [[suggestions: text]]

Fields from the Kind schema (if a schema is defined for the object's Kind).

Requires schema registry to be configured and the object's Kind to be known.

### Description [[description: text]]

**Pattern:**

```markdown
## User [[user: User]]

- |
  ↑ cursor here
```

**Example:**

```markdown
## User [[user: User]]

- |
  ↑ Ctrl+Space

Suggestions:
  name         (string, required)
  email        (string, required)
  age          (number, optional)
  created_at   (string, optional)
```

## Built-in Types [[ctx_builtin_types: CompletionContext]]

Autocompletion of built-in type hints for fields.

- trigger: "[[field: "
- status: planned

### Suggestions [[suggestions: text]]

| Type | Description |
|------|-------------|
| `text` | Multiline text |
| `object` | Nested object |
| `array` | Array (via subheadings or list) |
| `yaml` | YAML block |

### Description [[description: text]]

**Pattern:**

```markdown
- description: [[description: |]]
                              ↑ cursor here
```

**Example:**

```markdown
- description: [[description: |]]
                              ↑ Ctrl+Space

Suggestions:
  text         (multiline text)
  object       (nested object)
  array        (array)
  yaml         (YAML block)
```

## Field Value [[ctx_field_value: CompletionContext]]

Autocompletion of field value based on schema.

- trigger: "- key: "
- status: planned

### Suggestions [[suggestions: text]]

Enum values from the schema (if the field has an enum type).

Requires schema registry and a field with an enum type.

### Description [[description: text]]

**Pattern:**

```markdown
## Feature [[feature: Feature]]

- status: |
          ↑ cursor here
```

**Example:**

```markdown
## Feature [[feature: Feature]]

- status: |
          ↑ Ctrl+Space

Suggestions:
  planned
  in_progress
  implemented
  deprecated
```
