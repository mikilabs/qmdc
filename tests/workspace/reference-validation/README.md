# Reference Validation Bug Test

## Goal

Verify that the parser does NOT validate `[[id:Kind]]` (without `#`) as references inside text fields.

## Problem (BUG)

The parser sees constructs like `[[user:User]]`, `[[config:Config]]`, `[[description:text]]` **inside a text field** and tries to **validate them as references**, even though they are not references.

Per the specification:

- `[[#id]]` — a **reference** (with `#`) → must be validated
- `[[id]]` or `[[id:Kind]]` — a **definition** (without `#`) → NOT a reference, no validation needed

## Current behavior (BUG)

```bash
$ qmdc workspace parse reference-validation
```

Raises errors:

```json
{
  "type": "broken_link",
  "reference": "user:User",
  "message": "Object 'User' not found"
}
{
  "type": "broken_link",
  "reference": "config:Config",
  "message": "Object 'Config' not found"
}
{
  "type": "broken_link",
  "reference": "description:text",
  "message": "Object 'text' not found"
}
```

## Expected behavior

**There should be NO errors.**

Inside the text field `[[syntax:text]]` there is TEXT containing syntax examples. Constructs like `[[id:Kind]]` without `#` are **not references** — they are syntax examples for documentation.

Only `[[#user]]` and `[[#config]]` (with `#`) are references, and they are valid because the objects `user` and `config` exist.

## Where the bug is

In the reference-validation code (likely in `workspace.rs` or `validator.rs`) there is a function that searches for the `[[...]]` pattern in text fields and tries to validate everything.

The fix: validate **only** `[[#...]]`, and ignore `[[id]]` and `[[id:Kind]]`.
