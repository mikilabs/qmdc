# Doc

## Parser Diff [[parser_diff: Finding]]

**Problem:**
Different parsers find different error counts:
- Rust: 0 errors for docs
- Python: 45 errors for docs
- TypeScript: 44 errors for docs

**Root Cause:**
This is related to bug QMD-10: text fields create extra objects.

**Verification:**
For workspace docs:
- Rust: 11 objects with text
- Python: 56 objects with text
- TypeScript: 56 objects with text

These extra objects cause additional validation errors.

**Validate command check:**
For workspace microtests/errors:
- Rust: parse=6, validate=6
- Python: parse=6, validate=6
- TypeScript: parse=6, validate=6

The validate command correctly returns the same as parse.

**Solution:**
This is a separate task. The validate command works correctly.

- category: known_issue
- related_to: [[#task1]]
- solution: Validate command works correctly
