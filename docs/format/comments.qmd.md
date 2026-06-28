# Comment [[comment: SyntaxConcept]]

- depends: [[#object]], [[#field]]

## Description [[description: text]]

Comments capture descriptive text and unstructured content within objects. All text and subheadings without `[[field_id]]` that don't become fields are collected into the `__comments` system field with semantic binding to the object or its fields.

## Syntax [[syntax: text]]

Format: array of objects with `after` (anchor) and `content` (raw markdown slice).

- `after: "__self"` — comment on the object itself (between heading and first structural element)
- `after: "field_name"` — comment after a specific field

The parser does not interpret `content` — it extracts the raw markdown fragment between structural boundaries. Code blocks, tables, lists inside comments are preserved as-is.

Boundaries are determined by:

- Headings at the same or higher level
- Headings with `[[field_id]]`
- Field lists with valid QMD.md keys
- End of document

## Examples [[examples: text]]

```markdown example
## User [[user]]

General information about the user.

- name: John Doe

Full name of the user.

- email: john@example.com

Additional notes.
```

Result: `__comments` contains three entries bound to `__self`, `name`, and `email` respectively.

Subheadings without `[[field_id]]` inside an object are comment headings — they and all content below them (until the next structural boundary) become part of `__comments`.

## Rules [[rules: text]]

- Comments always come after their anchor (no `before` concept)
- HTML comments `<!-- ... -->` are ignored completely (not captured in `__comments`)
- If no comments exist, `__comments` is not created
- Comments are preserved for all object types: top-level, nested, array elements
- Field order MUST be strictly preserved (insertion order) — without this, comment anchors lose meaning
- The `example` modifier on code fences prevents reference parsing inside them
