# Array [[array: SyntaxConcept]]

- depends: [[#object]], [[#field]]

## Description [[description: text]]

Arrays of primitives and objects. QMD.md supports two syntaxes for primitive arrays: YAML notation (compact inline `[a, b, c]`) and Markdown lists (expanded via subheading). Object arrays are created via subheadings where each child heading becomes an array element. Tables provide a compact alternative for homogeneous object arrays.

## Syntax [[syntax: text]]

YAML inline notation:

```markdown example
- tags: [react, typescript, nodejs]
- ports: [8080, 8081, 8082]
- enabled: [true, false, true]
- mixed: [1, "two", true, null]
```

Values are wrapped in `[...]`, elements separated by commas. Strings with spaces, commas, or special characters require quotes: `"hello world"`. The `__syntax` field records `yaml_array`.

YAML multiline notation (for long values):

```markdown example
## Result [[result: TaskResult]]

- files_changed: [
    qmdc-rs/src/parser.rs,
    qmdc-py/qmdc/parser.py,
    qmdc-ts/src/parser.ts
  ]
- status: done
```

Opening bracket `[` on the same line as the field name, each element on its own line with 4-space indent, closing bracket `]` on a separate line with 2-space indent. The `__syntax` field records `yaml_multiline_array` (preserves formatting on round-trip).

Markdown lists via subheading:

```markdown example
## Team [[team]]

### Members [[members]]

- Alice
- Bob
- Charlie
```

A subheading with `[[field_id]]` followed by a bullet list without colons (`- value`, not `- key: value`). Each list item is an array element. Supports multiline elements. The `__syntax` field records `markdown_list`.

Explicit type annotation: `[[field_id: [type]]]` where type is `[string]`, `[number]`, `[boolean]`, or a user-defined type.

Comma-separated references (no outer brackets):

```markdown example
- deps: [[#auth]], [[#db]], [[#cache]]
```

Object arrays via subheadings with `[[field: [Kind]]]`:

```markdown example
## My Scenario [[my_scenario]]

- name: Zoe

### Stages [[stages: [Stage]]]

#### After Class [[after_class]]

- duration: 10

#### Sketch [[sketch]]

- duration: 15
```

Each child heading at the next level becomes a separate object in the array. The parent gets `stages: ["[[#after_class]]", "[[#sketch]]"]`, and each child gets `__parent` and `__parent_field` auto-links.

Variants for the section heading:

- `[[field_id]]` — array without type (auto-detection)
- `[[field_id: [Kind]]]` — typed array (strict validation)
- `[[field_id: array]]` — explicit array (strict validation)

Table syntax as compact alternative:

```markdown example
## Team [[team]]

### Members [[members: [User]]]

| name    | role      | email          |
| ------- | --------- | -------------- |
| Alice   | admin     | alice@ex.com   |
| Bob     | developer | bob@ex.com     |
| Charlie | designer  | charlie@ex.com |
```

First row = field names, each data row = one object. Supports primitives and references in cells. The `__syntax` field records `table`.

## Examples [[examples: text]]

Primitive arrays — strings:

```markdown example
## Config [[config]]

- tags: [react, typescript, nodejs]
```

```json example
[
  {
    "__id": "config",
    "__label": "Config",
    "__syntax": { "tags": "yaml_array" },
    "tags": ["react", "typescript", "nodejs"]
  }
]
```

Primitive arrays — numbers:

```markdown example
## Config [[config]]

### Ports [[ports]]

- 8080
- 8081
- 8082
```

```json example
[
  {
    "__id": "config",
    "__label": "Config",
    "__syntax": { "ports": "markdown_list" },
    "ports": [8080, 8081, 8082]
  }
]
```

Primitive arrays — mixed types:

```markdown example
- values: [null, 42, "hello world"]
```

Multiline elements:

```markdown example
## Documentation [[documentation]]

### Notes [[notes]]

- First note with
  multiple lines
  of text
- Second note
- Third note
```

```json example
[
  {
    "__id": "documentation",
    "__label": "Documentation",
    "notes": [
      "First note with\nmultiple lines\nof text",
      "Second note",
      "Third note"
    ]
  }
]
```

Object arrays with parent-child links:

```markdown example
## Company [[acme]]

- name: Acme Corporation

### Teams [[teams: [Team]]]

#### Engineering [[team_eng]]

- members: 45

#### Marketing [[team_marketing]]

- members: 25
```

```json example
[
  {
    "__id": "acme",
    "__label": "Company",
    "name": "Acme Corporation",
    "teams": ["[[#team_eng]]", "[[#team_marketing]]"]
  },
  {
    "__id": "team_eng",
    "__label": "Engineering",
    "__kind": "Team",
    "__parent": "[[#acme]]",
    "__parent_field": "teams",
    "members": 45
  },
  {
    "__id": "team_marketing",
    "__label": "Marketing",
    "__kind": "Team",
    "__parent": "[[#acme]]",
    "__parent_field": "teams",
    "members": 25
  }
]
```

Table syntax with references in cells:

```markdown example
## Organization [[org]]

### Team Members [[members: [Member]]]

| name    | role             | manager    |
| ------- | ---------------- | ---------- |
| Alice   | [[#admin_role]]  | [[#bob]]   |
| Bob     | [[#dev_role]]    | null       |
| Charlie | [[#design_role]] | [[#alice]] |
```

## Rules [[rules: text]]

- Only bullet lists are allowed in heading-syntax primitive arrays. Numbered lists (`1. item`) produce an `ordered_list_in_array` error.
- Parent-child auto-links: objects inside array sections (`[[field: [Kind]]]` or `[[field: array]]`) automatically receive `__parent` (reference to the parent object) and `__parent_field` (field name in the parent). Independent objects outside array sections do not get `__parent`.
- Syntax choice is preserved in `__syntax` for lossless round-trip: `yaml_array`, `yaml_multiline_array`, `markdown_list`, `headers`, `table`.
- Table syntax is equivalent to subheading syntax but more compact. The same data can be expressed either way.
- Tables support only primitives and references in cells — nested objects are not supported.
- In table syntax, objects receive auto-generated `__id` values (e.g. `user_0`, `user_1`) unless an explicit ID column is present.
- Text around tables (before or after) is captured in `__comments`.
