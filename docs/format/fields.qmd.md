# Field [[field: SyntaxConcept]]

- depends: [[#object]], [[#heading]]

## Description [[description: text]]

Fields are the data content of objects, defined via Markdown lists with `- key: value` syntax. Fields can also be defined via heading-syntax for multiline content.

## Syntax [[syntax: text]]

Inline fields use bullet lists:

```markdown example
## User [[user]]

- name: Alice
- age: 30
- active: true
- score: 95.5
```

Heading-syntax fields use subheadings with `[[field_id]]`:

```markdown example
## Article [[article]]

### Content [[content: text]]

This is multiline content with **Markdown** formatting.

### Config [[config]]

- host: localhost
- port: 8080
```

## Field Keys [[field_keys: text]]

A valid QMD.md field key must match `[a-zA-Z_][a-zA-Z0-9_]*`:

- Starts with a letter (a-z, A-Z) or underscore `_`
- May contain letters, digits, underscore `_`
- Must NOT contain spaces, hyphens, special characters, or Markdown formatting

Examples of valid keys: `name`, `firstName`, `first_name`, `user2`, `DBHos`

Examples of invalid keys: `First Name` (space), `my-key` (hyphen), `**Name**` (formatting), `2name` (starts with digit)

Lines with invalid keys are treated as markdown text (not fields) and go into `__comments`. If a list mixes valid and invalid keys, the parser generates a `mixed_field_keys` error.

## Heading-Syntax Fields [[heading_syntax_fields: text]]

Subheadings with `[[field_id]]` or `[[field_id: Kind]]` create structured data. The type is determined by explicit declaration or auto-detection.

Explicit type (strict validation):

- `[[field_id: text]]` — multiline text field. Expects text content. Error if structural subheadings with `[[id]]` appear.
- `[[field_id: Kind]]` or `[[field_id: object]]` — nested object. Expects field lists `- key: value`.
- `[[field_id: [Kind]]]` or `[[field_id: array]]` — object array. Expects child subheadings with `[[id]]`.

Auto-detection (when no type hint):

1. Has child subheadings with `[[field_id]]`? → object array
2. Has lists with `- key: value` (valid keys)? → nested object
3. Has lists without colons `- value`? → primitive array
4. Has text but no valid lists? → text field

## Extraction Rules [[extraction_rules: text]]

Inside an object, content is processed block by block:

1. Bullet list with all valid keys → fields
2. Bullet list with mixed valid/invalid keys → `mixed_field_keys` error; valid keys extracted, invalid go to `__comments`
3. Bullet list with no valid keys → `__comments`
4. Non-bullet content (paragraphs, code blocks, tables, comment headings) → `__comments`
5. Each subsequent bullet list is evaluated independently

Subheadings without `[[field_id]]` inside an object are comment headings — their content goes to `__comments`.

Subheadings with `[[field_id]]` are structural elements that create fields, nested objects, or arrays.

## YAML Multiline Strings [[yaml_multiline: text]]

Fields support YAML pipe syntax for multiline strings:

```markdown example
## Config [[config]]

- description: |
    This is a multiline string
    using YAML pipe syntax.
    References like [[#something]] are NOT parsed here.
```

References `[[#id]]` inside YAML pipe `|` blocks are NOT parsed — content remains plain text. They won't create edges in the graph or trigger broken_link errors.

## Dangling Fields [[dangling_fields: text]]

Heading-syntax fields (`text`, `array`, `yaml`, `json`, `object_array`) require a parent object at a higher heading level. If no parent exists, the parser generates a `dangling_field` error. The object is created for lossless round-trip but flagged as an error.

## Rules [[rules: text]]

- Fields work only inside objects (not at document top level without a parent)
- References `[[#id]]` in text fields are validated for existence but remain as text (not resolved into objects)
- References in YAML pipe blocks are not parsed at all
- References inside inline code and `example`-modified code fences are not parsed
- Nested lists `- key:\n  - item` are forbidden (`nested_subitems` error). Use YAML arrays or heading-syntax instead.
- Map fields (`[[field: map]]`) accept only bullet lists with `- key: value` pairs. All values are strings (no type auto-detection).
