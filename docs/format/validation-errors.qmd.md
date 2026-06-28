# Validation Errors [[validation_errors: SyntaxConcept]]

- depends: [[#reference]], [[#workspace]], [[#object]]

Validation errors detected by the `workspace validate` command.

## Broken Link [[err_broken_link: ValidationError]]

A `[[#id]]` reference points to a non-existent object in the workspace (after both `__id` and `__local_id` lookups fail).

- code: broken_link
- severity: error

### Cause [[cause: text]]

- The object with the specified ID does not exist (neither as `__id` nor as `__local_id`)
- Typo in the reference ID
- The object was deleted but references remain
- Incorrect namespace in the reference

Note: before producing a `broken_link` error, the validator attempts `__local_id` fallback — if the reference matches exactly one object's `__local_id`, it resolves successfully without error.

### Solution [[solution: text]]

1. Verify the target object exists
2. Fix the ID in the reference
3. Create the missing object
4. Add namespace/Kind to the reference: `[[#namespace:id]]`
5. Use the full hierarchical ID: `[[#parent.child]]` instead of `[[#child]]`

## Duplicate ID [[err_duplicate_id: ValidationError]]

Two objects with the same `Kind:Id` in one namespace.

- code: duplicate_id
- severity: error

### Cause [[cause: text]]

- Copying objects without changing the ID
- Auto-generated IDs from identical Titles
- Git merge conflicts

### Solution [[solution: text]]

1. Rename one of the objects (change the ID)
2. Use explicit `[[id]]` instead of auto-generation
3. Move objects into different namespaces
4. Change the Kind of one of the objects

## Ambiguous Reference [[err_ambiguous_ref: ValidationError]]

A `[[#id]]` reference could point to multiple objects (ID collision or multiple `__local_id` matches).

- code: ambiguous_reference
- severity: error

### Cause [[cause: text]]

- Same ID on objects with different Kind: `Table:users` and `Entity:users`
- Same ID in different namespaces
- Reference without Kind or namespace qualifier
- Multiple objects share the same `__local_id` (e.g., several child objects named `[[config]]` under different parents, all with `__local_id: "config"`)

### Solution [[solution: text]]

1. Add Kind to the reference: `[[#Table:users]]`
2. Add namespace to the reference: `[[#storage:users]]`
3. Use the full form: `[[#storage:Table:users]]`
4. Use the full hierarchical ID: `[[#parent.config]]` instead of `[[#config]]`

## Nested Workspace [[err_nested_workspace: ValidationError]]

A workspace inside another workspace (nested workspaces are forbidden).

- code: nested_workspace
- severity: error

### Cause [[cause: text]]

- A `__Workspace` object was created inside an existing workspace
- Incorrect directory structure

### Solution [[solution: text]]

1. Move the nested workspace up one level (make them siblings)
2. Change the Kind to `__Namespace` instead of `__Workspace`
3. Delete the nested workspace

## Workspace In Wrong File [[err_workspace_in_wrong_file: ValidationError]]

A `__Workspace` object is defined in a file other than `readme.qmd.md`.

- code: workspace_in_wrong_file
- severity: error

### Cause [[cause: text]]

`__Workspace` declarations must be in `readme.qmd.md` (the anchor file for the workspace root). If a `__Workspace` object appears in any other file, the parser generates this error.

### Solution [[solution: text]]

1. Move the `__Workspace` declaration to `readme.qmd.md`
2. If the file is a namespace, use `__Namespace` instead of `__Workspace`

## Type Mismatch [[err_type_mismatch: ValidationError]]

The explicitly declared type `[[field: Kind]]` does not match the content structure.

- code: type_mismatch
- severity: error
- status: planned

### Cause [[cause: text]]

- Declared `[[field: text]]`, but valid field lists are present (structure = object)
- Declared `[[field: Kind]]`, but child subheadings with `[[id]]` exist (structure = array)
- Declared `[[field: [Kind]]]`, but no child subheadings exist (structure = object)

### Solution [[solution: text]]

1. Fix the type in the heading: `[[field: text]]` → `[[field: Kind]]`
2. Restructure the content to match the declared type
3. Remove the explicit type and use auto-detection: `[[field]]`

## Structured In TextBlock [[err_structured_in_textblock: ValidationError]]

Attempt to create a structured element (object/field) inside a `__TextBlock`.

- code: structured_in_textblock
- severity: error

### Cause [[cause: text]]

`__TextBlock` is a system object for unstructured content. It is created when:

- The document starts with a heading without `[[id]]`
- A heading without `[[id]]` appears at the top level (not inside an object)
- After a code fence at the top level

Inside a `__TextBlock`, structured elements are forbidden:

- Headings with `[[id]]` or `[[id: Kind]]`
- Field lists `- key: value`

### Examples [[examples: text]]

Error example:

```markdown example
## Documentation Section

This starts a TextBlock (heading without [[id]]).

### Some Object [[my_obj]]

Error! Attempt to create an object inside a TextBlock.
```

Important exception — do not confuse TextBlock with an object's comment section:

A comment section is a heading without `[[id]]` inside an object. It is part of the object (`__comments` field), not a TextBlock.

```markdown example
# My Object [[my_obj: Kind]]

- name: Example

## Comment Section

This is a Comment inside an object, NOT a TextBlock!

### Nested [[nested_obj: NestedKind]]

This is a valid nested object, NOT an error.
```

In an object's comment section, a heading with `[[id: Kind]]` creates a nested object — this is normal behavior.

### Solution [[solution: text]]

1. Add `[[id]]` to the parent heading to create an object instead of a TextBlock
2. Move the structured content to the top level
3. Remove `[[id]]` from the nested heading if it is just text

## Multiple Definitions In Heading [[err_multiple_definitions: ValidationError]]

A heading contains more than one `[[...]]` definition.

- code: multiple_definitions
- severity: error

### Cause [[cause: text]]

Each heading may contain at most one `[[id]]` or `[[id: Kind]]` definition. If a heading has two or more definitions, the parser cannot unambiguously determine which one is the object ID and which is a field. This is a syntax error.

### Examples [[examples: text]]

```markdown example
## Also broken [[items: array]] [[finding_array: Finding]]
```

Here `[[items: array]]` is a field definition and `[[finding_array: Finding]]` is an object definition. The parser does not know which is the heading's ID.

### Solution [[solution: text]]

1. Split into two headings:

```markdown example
## Findings [[finding_array: Finding]]

### Items [[items: array]]

- severity: high
```

1. Or use a single definition with fields:

```markdown example
## Also broken [[finding_array: Finding]]

- severity: high
- category: parser
```

## Ordered List In Array [[err_ordered_list_in_array: ValidationError]]

A numbered list (`1. item`) is used inside a heading-syntax array instead of a bullet list.

- code: ordered_list_in_array
- severity: error

### Cause [[cause: text]]

QMD.md supports only bullet lists (`- item`) for markdown-list arrays. Numbered lists (`1. First`, `2. Second`) are forbidden — numbering is redundant since arrays are ordered by definition.

The numbered list content is preserved in `__comments` for lossless round-trip, but the parser generates an error.

### Examples [[examples: text]]

```markdown example
## Task [[task1: Task]]

- category: implementation

### Steps [[steps: array]]

1. First step
2. Second step
3. Third step
```

### Solution [[solution: text]]

Replace the numbered list with a bullet list:

```markdown example
### Steps [[steps: array]]

- First step
- Second step
- Third step
```

Or use YAML notation:

```markdown example
- steps: [First step, Second step, Third step]
```

## Explicit System Type [[err_explicit_system_type: ValidationError]]

Explicit declaration of a system type `__Document`, `__TextBlock`, or `__Object` in a heading.

- code: explicit_system_type
- severity: error

### Cause [[cause: text]]

System types `__Document`, `__TextBlock`, and `__Object` are created by the parser automatically only. Explicit declaration of `[[id: __Document]]`, `[[id: __TextBlock]]`, or `[[id: __Object]]` in a heading is forbidden.

Types `__Workspace` and `__Namespace` allow explicit declaration in anchor files (`readme.qmd.md`).

### Examples [[examples: text]]

```markdown example
# Test Document [[test_doc: __Document]]

- version: 1.0
```

### Solution [[solution: text]]

1. Remove the system type from the heading: `# Test Document [[test_doc]]`
2. Use a user-defined type: `# Test Document [[test_doc: Document]]`

## Dangling Field [[err_dangling_field: ValidationError]]

A heading-syntax field is declared without a parent object at a higher heading level.

- code: dangling_field
- severity: error

### Cause [[cause: text]]

Heading-syntax field types (`text`, `array`, `yaml`, `json`, `object_array`) imply a parent object at a higher heading level. If no such parent exists (field at the top level or at the same level as a sibling object), this is an error.

The parser creates an object for lossless round-trip but generates a `dangling_field` error.

### Examples [[examples: text]]

```markdown example
## Result [[result1: Finding]]

- status: done

## Summary [[summary: text]]

This is a text field at the same H2 level as Result — no parent object exists.
```

Here `[[summary: text]]` is a heading-syntax field of type `text`, but it is at the same H2 level as `[[result1]]`. There is no parent object at a higher level (H1) that could contain this field.

### Solution [[solution: text]]

1. Add a parent object at a higher level:

```markdown example
# Document [[doc1]]

## Result [[result1: Finding]]

- status: done

## Summary [[summary: text]]

Summary text here.
```

1. Or remove the field type and make it a regular object:

```markdown example
## Summary [[summary]]

- content: Summary text here.
```

## Invalid Map Entry [[err_invalid_map_entry: ValidationError]]

A list item inside `[[field: map]]` is not a valid `key: value` pair.

- code: invalid_map_entry
- severity: error

### Cause [[cause: text]]

A map field expects a bullet list with items in the format `- key: value`, where `key` is a valid QMD.md key (`[a-zA-Z_][a-zA-Z0-9_]*`). The error is generated when:

- The key contains Markdown formatting (`**bold**`, `` `code` ``)
- The list item does not contain a colon (`- just text`)
- The item is a link (`- [link](url)`)

Invalid items are discarded; valid pairs are kept in the map.

### Examples [[examples: text]]

```markdown example
# Service [[svc1]]

### env [[env: map]]

- host: localhost
- **port**: 8080
- path: /api
```

The line `- **port**: 8080` generates an error — bold formatting makes the key invalid.

### Solution [[solution: text]]

Remove Markdown formatting from keys:

```markdown example
### env [[env: map]]

- host: localhost
- port: 8080
- path: /api
```

## Broken Parent [[err_broken_parent: ValidationError]]

Parent object not found for a dot-ID declaration.

- code: broken_parent
- severity: error

### Cause [[cause: text]]

A dot-ID object `[[parent_path.local_id]]` declares a hierarchical ID where `parent_path` does not resolve to an existing object in the workspace. The parent must exist for the child to attach to it.

### Examples [[examples: text]]

```markdown example
## Alice [[team.members.alice]]

- role: admin
```

If no object with `__id` equal to `team` exists (or `team` has no `members` field path), the parser cannot establish the parent relationship.

### Solution [[solution: text]]

1. Create the parent object first: `## Team [[team]]` with a `members` array field
2. Fix the parent path in the dot-ID: ensure each segment resolves to an existing object
3. Use a flat ID instead if hierarchy is not needed: `[[alice]]`

## Ambiguous Field Reference [[err_ambiguous_field_reference: ValidationError]]

A dot-path reference cannot be unequivocally resolved to an object or a field.

- code: ambiguous_field_reference
- severity: error

### Cause [[cause: text]]

A dot-path reference resolves both as an object ID and as a field-path on a parent object. The parser cannot determine whether the reference targets the object or the field.

### Examples [[examples: text]]

```markdown example
## Team [[team]]

- status: active

## Status [[team.status]]

- value: operational
```

Here `[[#team.status]]` is ambiguous — it could reference the field `status` on object `team`, or the object with hierarchical ID `team.status`.

### Solution [[solution: text]]

1. Rename the child object to avoid collision with the field name
2. Remove the field from the parent if the object is the intended target
3. Use a non-hierarchical ID for the object: `[[team_status]]`

## Invalid Map Content [[err_invalid_map_content: ValidationError]]

Content inside `[[field: map]]` that is not a bullet list with `key: value` pairs.

- code: invalid_map_content
- severity: error

### Cause [[cause: text]]

A map field accepts only a single bullet list with `- key: value` items. Any other content between the map heading and the next heading at the same or higher level is an error: paragraphs, code fences, numbered lists, additional bullet lists.

Invalid content is ignored; the map is populated only from the first valid bullet list.

### Examples [[examples: text]]

````markdown example
# Service [[svc1]]

### env [[env: map]]

Some description paragraph.

- host: localhost
- port: 8080

1. numbered item

```yaml
some: code
```
````

The paragraph, numbered list, and code fence generate `invalid_map_content` errors.

### Solution [[solution: text]]

Remove all content except the bullet list with `key: value` pairs:

```markdown example
### env [[env: map]]

- host: localhost
- port: 8080
```
