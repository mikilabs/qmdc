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
4. Add a qualifier to the reference: `[[#namespace:id]]` or `[[#workspace:namespace:id]]`
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

## Table In Array [[err_table_in_array: ValidationError]]

A Markdown table is used under a primitive array field (`[[field: array]]`).

- code: table_in_array
- severity: error

### Cause [[cause: text]]

A primitive array holds scalar values, and a table has columns — there is no defined mapping from
one onto the other, so the parser cannot decide what a row should become.

This differs from an OBJECT array (`[[field: [Kind]]]`), where a table IS valid: each data row
becomes one object and the column names become its fields. The distinction is the declared field
type, not the table.

The table content is preserved in `__comments` for lossless round-trip, but the parser generates an
error.

### Examples [[examples: text]]

```markdown example
## Doc [[doc]]

### Tags [[tags: array]]

| a |
|---|
| 1 |
```

### Solution [[solution: text]]

If the values are scalars, use a bullet list:

```markdown example
### Tags [[tags: array]]

- one
- two
```

If each row is meant to be an object, declare an object array and give it a Kind:

```markdown example
### Tags [[tags: [Tag]]]

| name | colour |
| ---- | ------ |
| one  | red    |
```

## Extra Table In Array [[err_extra_table_in_array: ValidationError]]

A second Markdown table appears under one object-array heading.

- code: extra_table_in_array
- severity: error

### Cause [[cause: text]]

The array heading's own table is what feeds the array — each data row becomes one object, with a
positional id derived from the row's index. A second table under the same heading cannot extend the
array, because its rows would generate the same ids as the first table's, and the heading declares an
array rather than prose, so the table describes nothing.

The table content is preserved in `__comments` for lossless round-trip, but the parser generates an
error.

Note this applies only to the array CONTAINER's own content. A table inside an array ELEMENT is that
element's content and is perfectly valid — see the Arrays section.

### Examples [[examples: text]]

```markdown example
## Team [[team: Group]]

### Members [[members: [User]]]

| name  |
| ----- |
| Alice |

| name |
| ---- |
| Bob  |
```

### Solution [[solution: text]]

Put every row in one table:

```markdown example
### Members [[members: [User]]]

| name  |
| ----- |
| Alice |
| Bob   |
```

## Mixed Array [[err_mixed_array: ValidationError]]

An object array is fed by a table AND also has heading elements.

- code: mixed_array
- severity: error

### Cause [[cause: text]]

An object array is written in exactly one of two forms — a table, where each data row becomes an
object, or subheadings, where each subheading becomes an object. Mixing them is not supported: once
the array has been built from the table, a following element heading cannot join it, so it silently
becomes a plain field on the parent and its declared Kind is lost.

Note this fires only when the TABLE comes first. A table AFTER an element heading is that element's
own content, which is valid — see the Arrays section.

### Examples [[examples: text]]

```markdown example
## Team [[team: Group]]

### Members [[members: [User]]]

| name  |
| ----- |
| Alice |

#### Bob [[bob]]

- role: dev
```

Here `bob` does not join `members`: it becomes `team.bob`, a field on `team`, with its Kind degraded
from `User` to `__Object`.

### Solution [[solution: text]]

Use one form for the whole array. Either put every element in the table:

```markdown example
### Members [[members: [User]]]

| name  | role  |
| ----- | ----- |
| Alice |       |
| Bob   | dev   |
```

Or write every element as a subheading:

```markdown example
### Members [[members: [User]]]

#### Alice [[alice]]

#### Bob [[bob]]

- role: dev
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

**Exemption:** the error is NOT raised when the parent's field value is exactly the parser-generated child link `[[#<full dotted id>]]`. Every nested child produces such a field on its parent (e.g. `svc` gets `config: "[[#svc.config]]"`), so without this exemption every hierarchical parent→child reference would be flagged. Only a field whose value is anything else (a scalar, a different reference) makes the dot-path genuinely ambiguous.

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

## Block In Inline Field [[err_block_in_inline_field: ValidationError]]

An indented block follows an inline field that already has a value.

- code: block_in_inline_field
- severity: error

### Cause [[cause: text]]

An inline field (`- key: value`) holds a scalar. It has no content of its own, so an indented table,
list, paragraph, quote or fence placed under it belongs to nothing — the format defines no meaning
for it.

This is the sibling of `nested_subitems`, which covers the EMPTY-value form (`- key:` followed by
indented items). Here the value is present and the block follows it.

Not to be confused with YAML multiline (`- key: |` or `- key: >`), where the indented block IS the
field's value. That is valid and is not reported.

The block content is preserved in `__comments`, anchored on the field and with its indentation
removed, for lossless round-trip; the parser generates an error.

### Examples [[examples: text]]

```markdown example
## P [[p: G]]

- lead: Ann

- note: something

  | ic |
  |----|
  | x |
```

### Solution [[solution: text]]

If the block belongs to the object, put it outside the field list:

```markdown example
## P [[p: G]]

- lead: Ann
- note: something

| ic |
|----|
| x  |
```

If it belongs to the field, declare a heading-syntax text field instead:

```markdown example
## P [[p: G]]

- lead: Ann

### Note [[note: text]]

something

| ic |
|----|
| x  |
```

## Nested Subitems [[err_nested_subitems: ValidationError]]

A nested list under an inline field (`- key:` followed by indented `- item` lines).

- code: nested_subitems
- severity: error

### Cause [[cause: text]]

Inline fields are flat — a list item cannot carry its own sub-list. The pattern `- key:` with indented `- item` children is rejected.

### Solution [[solution: text]]

Use a YAML array (`- items: [first, second]`) or heading-syntax (`### Items [[items: array]]` with a bullet list).

## Mixed Field Keys [[err_mixed_field_keys: ValidationError]]

An object's field list mixes valid and invalid keys.

- code: mixed_field_keys
- severity: error

### Cause [[cause: text]]

Valid keys match `[a-zA-Z][a-zA-Z0-9_]*`. When SOME items in one object's list are valid fields and others are not (spaces, hyphens, leading digits, markdown formatting in the key), the parser cannot decide whether the list is a field list or plain text.

### Solution [[solution: text]]

Fix the invalid keys (`First Name:` → `first_name:`), or convert the list to a text field if it is prose.

## Invalid ID Character [[err_invalid_id_character: ValidationError]]

A dot in a NESTED heading's explicit ID.

- code: invalid_id_character
- severity: error

### Cause [[cause: text]]

Dot-ID declarations (`[[parent.child]]`) are legal only on top-level headings. A nested child's hierarchical ID is composed automatically from its parent; an explicit dot in a nested `[[id]]` is a parsing error.

```markdown example
## Parent [[parent]]

### Child [[child.invalid]]   ← error: dot in nested explicit ID
```

### Solution [[solution: text]]

Use a simple local id for the nested heading (`[[child]]`); the dot-path (`parent.child`) is composed automatically.
