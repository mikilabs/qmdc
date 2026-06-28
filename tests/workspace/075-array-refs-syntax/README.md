# Array Refs Syntax Test

## Problem (BUG)

The parser does not correctly handle the array-of-references syntax via a subheading with type `[string]`.

**Per the documentation (MARKDOWN_SYNTAX.md, line 1283):**

- `[[field_id: [type]]]` → **array of primitives** (section 5) or **array of objects** (section 6)
  - For primitives: a list without colons (`- value`)
  - For objects: child subheadings WITH `[[field_id]]`

The parser should distinguish these cases by content, but it creates a separate object instead of an array on the parent.

## Syntax

```markdown
### Examples [[examples: [string]]]

- [[#ex_basic_object]]
- [[#ex_data_types]]
- [[#ex_text_field]]
```

## Expected behavior

The parser should create an array of references in the `examples` field:

- Value: `["[[#ex_basic_object]]", "[[#ex_data_types]]", "[[#ex_text_field]]"]`
- Syntax: `markdown_list`
- Type: `array`

## Current behavior

The parser creates an empty array with `headers` syntax:

- Value: `[]`
- Syntax: `headers`
- Problem: the parser interprets this as an array of objects (headers) rather than an array of references (markdown_list)

## Status

**FAILING** — the test fails, showing the problem in the parser.

## Fix

The parser needs to recognize a list of references under a subheading with type `[string]` as an array of primitives (reference strings) rather than an array of objects.
