# Text Field Validation Test

## Goal

Verify that the parser raises an error when structured fields are nested inside a text field with an explicit `:text` type.

## Current behavior (BUG)

The parser **does not raise an error** and simply creates the fields:

- `[[step1:text]]` → text field `step1`
- `[[step2:object]]` → nested object `step2`

## Expected behavior (per the specification)

Per MARKDOWN_SYNTAX.md, section 3.4:

> ⚠️ **Strict validation (only with an explicit `text` type):**
> - Child subheadings WITH `[[field_id]]` → **parsing error**

The parser **should** raise an `invalid_structure` error with a clear message:

```json
{
  "type": "invalid_structure",
  "object": "algorithm",
  "field": "step1",
  "message": "Cannot nest structured field [[step1:text]] inside text field. Remove :text or use plain heading."
}
```

## Required fix

1. Add a check when parsing a field with an explicit `:text` type.
2. If subheadings with `[[field_id]]` are found inside, raise an error.
3. Update `_expected.json` with the correct errors.
4. After the parser fix, this test should pass.
