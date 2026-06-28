# Diagnostics [[lsp_diagnostics: Category]]

Document validation with errors and warnings displayed in the editor. Groups the diagnostics feature and its rules.

## Diagnostics [[diagnostics: LSPFeature]]

Red and yellow underlines in the editor for errors and warnings.

- lsp_method: textDocument/publishDiagnostics
- status: implemented
- depends: [[#parsers:rust_parser]]

### Description [[description: text]]

**Update triggers:**

- `textDocument/didOpen` — when a file is opened
- `textDocument/didChange` — on changes (with debounce)
- `textDocument/didSave` — on save
- `workspace/didChangeWatchedFiles` — when other files change

**Errors (red):**

- [[#rule_broken_link]] (QMDC001) — reference to a non-existent object
- [[#rule_duplicate_id]] (QMDC003) — duplicate IDs
- [[#rule_workspace_wrong_file]] (QMDC004) — workspace in wrong file

**Planned errors (not yet implemented):**

- [[#rule_ambiguous_ref]] (QMDC002) — ambiguous reference
- [[#rule_invalid_ref]] (QMDC006) — invalid reference syntax

**Planned warnings (not yet implemented, requires schema registry):**

- [[#rule_unknown_kind]] (QMDC005) — unknown Kind
- [[#rule_missing_field]] (QMDC007) — missing required field
- [[#rule_invalid_field_type]] (QMDC008) — incorrect field type

**Related information:**
For some errors, related information is shown:

- Duplicate ID → first definition line number in message text

**Example:**

```markdown example
- user: [[#alice]]
        ^^^^^^^^^^^  ← Error (QMDC001): Object 'alice' not found

## Users [[users: Table]]
...
## Users [[users: Table]]
   ^^^^^^^^^^^^^^^^^^^^^  ← Error (QMDC003): Duplicate 'Table:users'
                             First defined at line 5
```

## Broken Link [[rule_broken_link: DiagnosticRule]]

Reference to a non-existent object.

- code: QMDC001
- severity: error
- validates: [[#format:validation_errors.err_broken_link]]

### Description [[description: text]]

**Pattern:** `[[#id]]` where id does not exist in the workspace (neither as `__id` nor as `__local_id`).

**Message:**

```text
Object '{id}' not found
```

**Range:** the entire reference `[[#id]]` is underlined in red.

**Related information:**
When a broken link's id matches a `__local_id` in another namespace, the diagnostic includes a hint suggesting the full qualified reference. For example, if `[[#config]]` is broken but `storage:parent.config` has `__local_id: "config"`, the hint suggests using `[[#storage:parent.config]]`.

**Example:**

```markdown example
- user: [[#alice]]
        ^^^^^^^^^^^  ← Error: Object 'alice' not found

- config: [[#config]]
          ^^^^^^^^^^^  ← Error: Object 'config' not found
                          Hint: Did you mean '[[#storage:app.config]]'?
```

## Ambiguous Reference [[rule_ambiguous_ref: DiagnosticRule]]

Ambiguous reference — ID exists in multiple namespaces, with different Kinds, or matches multiple `__local_id` values.

- code: QMDC002
- severity: error
- status: planned
- validates: [[#format:validation_errors.err_ambiguous_ref]]

### Description [[description: text]]

**Pattern:** `[[#id]]` where id exists in multiple places:

- Different namespaces: `storage:users` and `auth:users`
- Different Kinds: `Table:users` and `View:users`
- Multiple `__local_id` matches: several child objects with the same local name (e.g., `parent1.config` and `parent2.config` both have `__local_id: "config"`)

**Message:**

```text
Ambiguous reference '{id}', found in: {locations}
```

**Related information:** shows a list of all candidates with their locations.

**Resolution:** add a qualifier:

- `[[#namespace:id]]` — specify namespace
- `[[#Kind:id]]` — specify Kind
- `[[#namespace:Kind:id]]` — specify both
- `[[#parent.child]]` — use full hierarchical ID (for `__local_id` ambiguity)

**Example:**

```markdown example
- table: [[#users]]
         ^^^^^^^^^^^  ← Error: Ambiguous reference 'users'
                         Found in: storage:users, auth:users

- cfg: [[#config]]
       ^^^^^^^^^^^  ← Error: Ambiguous reference 'config'
                       Found in: app.config, server.config
```

## Duplicate ID [[rule_duplicate_id: DiagnosticRule]]

Two objects with the same Kind:Id in one namespace.

- code: QMDC003
- severity: error
- validates: [[#format:validation_errors.err_duplicate_id]]

### Description [[description: text]]

**Pattern:** two headings with the same `[[id]]` or `[[id: Kind]]` in one namespace.

**Message:**

```text
Duplicate '{kind}:{id}' in namespace '{namespace}'
```

**Range:** the heading of the second object is underlined in red.

**Related information:** shows the location of the first object.

**Example:**

```markdown
## Users [[users: Table]]
...

## Users [[users: Table]]
   ^^^^^^^^^^^^^^^^^^^^^  ← Error: Duplicate 'Table:users' in namespace 'storage'
                             First defined at line 5
```

## Unknown Kind [[rule_unknown_kind: DiagnosticRule]]

Kind not found in the workspace schemas.

- code: QMDC005
- severity: warning
- status: planned

### Description [[description: text]]

**Pattern:** `[[id: UnknownKind]]` where UnknownKind is not defined in the workspace.

**Message:**

```text
Unknown Kind '{kind}'
```

**Range:** the Kind in the heading is underlined in yellow.

This is a warning, not an error — QMD.md allows arbitrary Kinds. But if the workspace has a schema registry, unknown Kinds may be typos.

**Example:**

```markdown
## My Service [[my_service: Servise]]
                            ^^^^^^^  ← Warning: Unknown Kind 'Servise'
                                        Did you mean 'Service'?
```

## Invalid Reference Syntax [[rule_invalid_ref: DiagnosticRule]]

Invalid reference syntax.

- code: QMDC006
- severity: error
- status: planned

### Description [[description: text]]

**Patterns:**

- `[[#]]` — empty reference
- `[[#:]]` — colon only
- `[[#::id]]` — double colon
- `[[# id]]` — space after hash
- `[[#id ]]` — space before closing bracket

**Message:**

```text
Invalid reference syntax
```

**Range:** the entire reference is underlined in red.

**Example:**

```markdown example
- ref: [[#]]
       ^^^^^  ← Error: Invalid reference syntax

- ref: [[# users]]
       ^^^^^^^^^^^  ← Error: Invalid reference syntax (space after #)
```

## Missing Required Field [[rule_missing_field: DiagnosticRule]]

A required field is missing for the given Kind (if a schema exists).

- code: QMDC007
- severity: warning
- status: planned

### Description [[description: text]]

**Pattern:** an object with a Kind that has a defined schema, but a required field is missing.

**Message:**

```text
Missing required field '{field}' for Kind '{kind}'
```

**Range:** the object heading is underlined in yellow.

Requires schema registry. If no schema exists, this rule does not apply.

**Example:**

```markdown
## My Table [[my_table: Table]]
   ^^^^^^^^^^^^^^^^^^^^^^^^^^  ← Warning: Missing required field 'columns' for Kind 'Table'

- name: my_table
```

## Invalid Field Type [[rule_invalid_field_type: DiagnosticRule]]

The field value type does not match the schema.

- code: QMDC008
- severity: warning
- status: planned

### Description [[description: text]]

**Pattern:** a field has a value of the wrong type according to the schema.

**Message:**

```text
Invalid type for field '{field}': expected {expected}, got {actual}
```

**Range:** the field value is underlined in yellow.

Requires schema registry.

**Example:**

```markdown
## My Table [[my_table: Table]]

- name: my_table
- row_count: "100"
             ^^^^^  ← Warning: Invalid type for field 'row_count': expected number, got string
```

## Workspace In Wrong File [[rule_workspace_wrong_file: DiagnosticRule]]

Workspace declaration in a file other than `readme.qmd.md`.

- code: QMDC004
- severity: error

### Description [[description: text]]

**Pattern:** a `__Workspace` object is defined in a file that is not `readme.qmd.md`.

**Message:**

```text
Workspace '{id}' must be defined in readme.qmd.md, not here
```

**Range:** the heading of the workspace object is underlined in red.

**Example:**

```markdown
# My Project [[myproject: __Workspace]]
  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^  ← Error: Workspace 'myproject' must be defined in readme.qmd.md
```
