# Validate a Document [[guide_validate: HowTo]]

- goal: find and fix errors in your .qmd.md files
- audience: newcomer
- prerequisites: [[#guide_first_file]]
- outcome: a clean `qmdc workspace validate` with zero errors
- about: [[#validation_errors]], [[#workspace]]
- next: [[#guide_query]]

## Content Generator [[guide_validate_gen: ContentGenerator]]

- target: [[#guide_validate.content]]
- about: [[#validation_errors]], [[#workspace]]
- sources_hash: 1c569ae44efaedc1

### Prompt [[guide_validate_gen_prompt: text]]

Write a guide: "How to validate your QMD.md files and fix common errors."

Cover:

1. Single file validation: `qmdc parse -i file.qmd.md > /dev/null` — exit code tells you if it's valid
2. Workspace validation: `qmdc workspace validate .` — checks cross-file references
3. Common errors and how to fix them:
   - broken_link: you referenced `[[#something]]` that doesn't exist → fix the ID or create the object
   - duplicate_id: two objects with the same ID → rename one
   - mixed_field_keys: some list items have valid keys, some don't → make all keys valid or none
4. Show example error output and the fix

Use the ValidationError objects from the source as reference for error codes and messages.
Keep it practical — show the error, show the fix.

## Content [[content: text]]

QMD.md files are plain Markdown, but the parser enforces structure rules. Validation catches mistakes early — broken references, duplicate IDs, and malformed fields — before they become confusing graph bugs.

## Validate a single file

Parse a file and check for syntax errors:

```bash
qmdc parse -i myfile.qmd.md > /dev/null
```

Exit code `0` means the file is valid. Non-zero means errors — they print to stderr. This catches per-file problems like `structured_in_textblock`, `dangling_field`, `multiple_definitions`, and `ordered_list_in_array`.

## Validate an entire workspace

Cross-file references can only be checked at workspace level:

```bash
qmdc workspace validate .
```

This command:

1. Finds all `.qmd.md` files in the workspace
2. Parses and indexes every object
3. Resolves all `[[#references]]` — first by `__id`, then by `__local_id` fallback
4. Returns a JSON array of errors (empty `[]` if clean)

Exit code `0` means no errors. Exit code `1` means errors exist.

Example output when errors are found:

```json
[
  {
    "type": "broken_link",
    "message": "Object 'user_settings' not found",
    "file": "config/app.qmd.md",
    "line": 12,
    "objectId": "app_config",
    "reference": "[[#user_settings]]",
    "severity": "error"
  }
]
```

## Common errors and fixes

### `broken_link`

A reference `[[#id]]` points to an object that doesn't exist.

**Causes:** typo in the ID, the target object was deleted, or incorrect namespace.

**Fix:**

- Check spelling of the ID
- Create the missing object
- Add namespace: `[[#storage:user_settings]]`
- Use hierarchical ID: `[[#parent.child]]`

### `duplicate_id`

Two objects share the same ID in one namespace.

**Causes:** copy-pasting without renaming, auto-generated IDs from identical headings, merge conflicts.

**Fix:**

- Rename one object — change its `[[id]]`
- Use an explicit ID instead of auto-generation
- Move one object to a different namespace

### `ambiguous_reference`

A reference matches multiple objects and the parser can't pick one (e.g., `Table:users` and `Entity:users` both match `[[#users]]`).

**Fix:**

- Add Kind: `[[#Table:users]]`
- Add namespace: `[[#storage:users]]`
- Use the full form: `[[#storage:Table:users]]`
- Use hierarchical ID: `[[#parent.config]]` instead of `[[#config]]`

### `structured_in_textblock`

You created an object inside a TextBlock (a heading without `[[id]]`).

```qmd
## Documentation Section

Some intro text.

### My Object [[my_obj]]

Error! Objects can't live inside a TextBlock.
```

**Fix:** Add `[[id]]` to the parent heading to make it a proper object, or move the nested object out:

```qmd
## Documentation Section [[docs_section]]

Some intro text.

### My Object [[my_obj]]

Now valid — parent has an ID.
```

### `dangling_field`

A heading-syntax field (`[[field: text]]`, `[[field: array]]`) has no parent object at a higher heading level.

```qmd
## Result [[result1: Finding]]

- status: done

## Summary [[summary: text]]

This field has no parent — it's at the same H2 level as Result.
```

**Fix:** Add a parent object at a higher heading level, or convert the field to a standalone object.

### `ordered_list_in_array`

Numbered lists are forbidden in arrays — use bullet lists instead.

```qmd
### Steps [[steps: array]]

1. First step
2. Second step
```

**Fix:**

```qmd
### Steps [[steps: array]]

- First step
- Second step
```

## Tips

- Run `qmdc workspace validate .` in CI to catch broken references before merge
- Most errors include `file`, `line`, and `reference` fields — jump straight to the problem
- When in doubt, use explicit IDs (`[[my_id]]`) rather than relying on auto-generation
- Use `qmdc parse -i file.qmd.md | qmdc rebuild` to normalize formatting

For the full list of error codes and their details, see the [[#validation_errors]] reference.
