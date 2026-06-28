# Heading [[heading: SyntaxConcept]]

- depends: [[#object]]

## Description [[description: text]]

All headings in QMD.md follow a unified syntax with optional components. Headings create objects and define document structure.

QMDC parses these Markdown constructs:

- Headings (`#` through `######` and beyond) create objects
- `[[id]]` in a heading defines the object identifier
- `[[id: Kind]]` adds a type
- `[[:Kind]]` auto-generates ID from Title and adds a type
- Title text (without `[[...]]`) is stored in `__label`

Position of `[[id]]` in the heading does not matter: `## Title [[id]]`, `## [[id]] Title`, and `## Title [[id]] More` all produce the same ID.

## Syntax [[syntax: text]]

Basic heading forms:

```markdown example
# Title
# Title [[id]]
# Title [[id: Kind]]
# Title [[:Kind]]
# [[id]] Title
# [[id: Kind]] Title
```

Components (all optional):

- Title — human-readable text, stored in `__label`
- `[[id]]` — object/field identifier
- `[[id: Kind]]` — identifier with type
- `[[:Kind]]` — auto-generated ID with type

## Auto ID Generation [[auto_id: text]]

When `[[id]]` is not specified, the ID is auto-generated from Title.

Algorithm:

1. Take the Title text (excluding `[[...]]` parts)
2. Convert to lowercase
3. Replace non-alphanumeric characters with a single space
4. Collapse consecutive spaces into one
5. Replace spaces with `_`
6. Strip leading and trailing `_`
7. If it starts with a digit, prepend `_`

Examples:

| Title           | Auto-generated ID |
| --------------- | ----------------- |
| `Configuration` | `configuration`   |
| `John Doe`      | `john_doe`        |
| `My Team #1`    | `my_team_1`       |
| `API Service`   | `api_service`     |
| `2024 Report`   | `_2024_report`    |

## Kinds [[kinds: text]]

Built-in kinds (lowercase) — parser hints:

- `text` — multiline text field
- `object` — nested object
- `array` — object array
- `yaml` — embedded YAML block
- `json` — embedded JSON block
- `map` — flat string dictionary (str→str)

System kinds (prefixed with `__`) — created automatically by the parser:

- `__Document` — document container
- `__TextBlock` — unstructured text block
- `__Object` — object without explicit Kind
- `__Workspace` — workspace root
- `__Namespace` — namespace grouping

User kinds (PascalCase) — from schemas:

- `User`, `Stage`, `Config` — object types for validation
- `[User]`, `[Stage]` — array of typed objects

Filtering system types: `objects.filter(obj => !obj.__kind?.startsWith("__"))`

## Nesting Levels [[nesting_levels: text]]

Heading levels H1–H6 and beyond (unlimited `#` count). Nesting is relative — a child element is one level below its parent.

Note: Standard Markdown supports only H1–H6. QMD.md extends this by allowing arbitrary `#` counts for deep nesting (H7, H8, etc.).

## Rules [[rules: text]]

- Each heading may contain at most one `[[...]]` definition. Multiple definitions (`## Title [[a: array]] [[b: Kind]]`) produce a `multiple_definitions` error.
- If `[[id]]` is not specified and the heading has no fields, it becomes a `__TextBlock` (not an object).
- If `[[id]]` is not specified but the heading has fields or heading-syntax children, the ID is auto-generated from Title.
- `__has_explicit_id: false` is set when the ID was auto-generated (absent when explicit).
- `__level` records the heading level (1–6+) for lossless rebuild.
