# ✅ Reference validation bug — FIXED

## Problem

The parser validated `[[id:Kind]]` (without `#`) as references inside text fields, even though they are **not references** but syntax examples or definitions.

## Before

**70 errors** in the documentation:

- 60+ `broken_link` with the message "Object 'text' not found"
- 9 `ambiguous_reference`
- 1 `duplicate_id`

## Fix

In `qmdc-rs/src/parser.rs`:

**Before:**

```rust
if inner.contains(':') && !inner.starts_with('#') {
    // Only check if there is a space after the colon
    let parts: Vec<&str> = inner.splitn(2, ':').collect();
    if parts.len() == 2 && parts[1].trim_start() != parts[1] {
        continue; // skip
    }
}
```

**After:**

```rust
// Only [[#...]] with a # are references
if !inner.starts_with('#') {
    continue; // skip definitions
}
```

## Result

**10 errors** (only real problems):

- 9 `ambiguous_reference` — ambiguous references `#workspace_module`
- 1 `duplicate_id` — duplicate ID

## Tests

- ✅ `reference-validation` workspace — passes
- ✅ All other workspace tests — pass
- ✅ Documentation — no more false errors
