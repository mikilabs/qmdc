# LSP Microtests [[suite_lsp_microtests: TestSuite]]

- location: tests/lsp/microtests/
- format: NNN-name/ with input.qmd.md, request.json, expected.json
- test_count: 100
- implementations: [lsp.rs]
- is_data_driven: true
- tests: [[#lsp]]

## Description [[description: text]]

Tests for LSP capabilities: completion, diagnostics, hover, definition, references, document symbols, rename, and more.

Use LSP microtests when:

- Adding a new LSP capability
- Fixing a bug in LSP functionality
- Verifying edge cases (broken links, ambiguous refs, etc.)
- Testing LSP on a multi-file workspace

Do not use for QMD.md syntax parsing (use parser microtests), SQL queries via LSP command (use LSP SQL integration tests), or SQL queries without LSP (use SQL workspace tests).

## How to Add a New Test

### Step 1: Create the test directory

Create `tests/lsp/microtests/{category}/NNN-name/`:

```text
completion/015-fuzzy-match/
├── input.qmd.md
├── request.json
└── expected.json
```

For multi-file tests, create a `workspace/` directory instead of `input.qmd.md`.

### Step 2: Create request.json

```json
{
  "method": "textDocument/completion",
  "position": { "line": 6, "character": 13 }
}
```

For multi-file tests, add `"uri": "workspace/api/routes.qmd.md"`.

### Step 3: Create expected.json

Format depends on the capability (completion items, diagnostics array, hover contents, definition location, references list).

### Step 4: Run tests

```bash
cd qmdc-rs && cargo test --test lsp
```

## LSP Capability Categories

### Tier 1: Core

- **Diagnostics** (QMDC001–QMDC006): broken links, duplicate IDs, ambiguous refs, invalid syntax, orphan definitions, circular refs. 15 tests.
- **Completion**: ID completion after `[[`, Kind completion, cross-file, namespace, hash-local, fuzzy match, case-insensitive. 15 tests.
- **Hover**: hover on refs, definitions, broken refs, cross-file, namespace. Shows Kind and properties. 10 tests.

### Tier 2: Navigation

- **Definition**: go to definition, cross-file, namespace-qualified, from property. 10 tests.
- **References**: find all references, include definition, cross-file. 5 tests.
- **Document Symbol**: all objects, nested, with kinds, properties as children. 5 tests.

### Tier 3: Refactoring

- **Prepare Rename**: check if rename is possible at cursor position.
- **Rename**: rename object across all references in the workspace.
- **Workspace Symbol**: search objects across the entire workspace.

## Diagnostic Codes

| Code | Severity | Description |
|------|----------|-------------|
| QMDC001 | Error | Object not found |
| QMDC002 | Warning | Ambiguous reference |
| QMDC003 | Error | Duplicate ID |
| QMDC004 | Error | Invalid reference syntax |
| QMDC005 | Hint | Orphan definition |
| QMDC006 | Warning | Circular reference |

## Multi-File Tests

Create a `workspace/` directory with multiple `.qmd.md` files. In `request.json`, specify `uri` relative to the workspace. In `expected.json`, URIs are also relative.
