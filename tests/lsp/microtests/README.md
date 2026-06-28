# LSP Micro-tests

A single test suite shared by all three LSP implementations (Rust, TypeScript, Python).

## Structure

```
lsp-microtests/
├── completion/       # textDocument/completion
├── diagnostics/      # textDocument/publishDiagnostics
├── hover/            # textDocument/hover
├── definition/       # textDocument/definition
├── references/       # textDocument/references (Tier 2)
├── document-symbol/  # textDocument/documentSymbol (Tier 2)
├── rename/           # textDocument/rename (Tier 2)
└── workspace-symbol/ # workspace/symbol (Tier 3)
```

## Test format

Each test is a folder with these files:

```
NNN-test-name/
├── input.qmd.md      # Input document
├── request.json      # LSP request (method, position)
└── expected.json     # Expected response
```

### request.json

```json
{
  "method": "textDocument/completion",
  "position": { "line": 0, "character": 14 }
}
```

### expected.json — formats by capability

**completion:**

```json
{
  "items": [
    { "label": "users", "kind": 6 },
    { "label": "orders", "kind": 6 }
  ]
}
```

**diagnostics:**

```json
{
  "diagnostics": [
    {
      "range": { "start": {"line": 1, "character": 7}, "end": {"line": 1, "character": 19} },
      "severity": 1,
      "code": "QMD001",
      "message": "Object 'missing' not found"
    }
  ]
}
```

**hover:**

```json
{
  "contents": "**Users** `Table`\n\n- name: users"
}
```

**definition:**

```json
{
  "uri": "input.qmd.md",
  "range": { "start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 20} }
}
```

**references:**

```json
{
  "locations": [
    { "uri": "input.qmd.md", "range": { "start": {"line": 3}, "end": {"line": 3} } }
  ]
}
```

**document-symbol:**

```json
{
  "symbols": [
    { "name": "Users", "kind": 5, "range": { "start": {"line": 0}, "end": {"line": 5} } }
  ]
}
```

## Multi-file tests (workspace)

For tests with several files:

```
NNN-crossfile/
├── workspace/
│   ├── readme.qmd.md
│   ├── models/users.qmd.md
│   └── api/routes.qmd.md
├── request.json       # uri points to a file in the workspace
└── expected.json
```

---

## Test categories

### Diagnostics (QMD001-QMD006)

| # | Test | Description |
|---|------|----------|
| 001 | broken-link | Reference to a nonexistent object |
| 002 | broken-link-with-kind | `[[Kind.missing]]` |
| 003 | duplicate-id | Two objects with the same ID |
| 004 | ambiguous-ref | Ambiguous reference (multiple Kinds) |
| 005 | invalid-syntax | Invalid syntax `[[]]` |
| 006 | orphan-definition | Definition without any use |
| 007 | empty-doc | Empty document — no errors |
| 008 | broken-cross-file | Reference to a nonexistent file |
| 009 | valid-refs | All references valid — no errors |
| 010 | case-sensitive | IDs are case-sensitive |
| 011 | multiple-errors | Several errors in one file |
| 012 | ref-in-code-block | Reference in a code block — not checked |
| 013 | ref-in-inline-code | `[[ref]]` in inline code |
| 014 | self-reference | Reference to itself |
| 015 | circular-ref | Circular reference (A→B→A) |

### Completion

| # | Test | Description |
|---|------|----------|
| 001 | id-completion | Completion after `[[` |
| 002 | kind-completion | Completion after `[[Kind.` |
| 003 | empty-file | Empty file — no completion |
| 004 | partial-id | Partial match `[[us` → users |
| 005 | no-completion-outside | Outside `[[` — no completion |
| 006 | cross-file | Completion from other files |
| 007 | namespace-completion | `[[ns.` → completion by namespace |
| 008 | ref-completion | `- ref: [[#` → completion |
| 009 | hash-completion | `[[#` — local IDs |
| 010 | kind-filter | After a Kind, show only that Kind |
| 011 | case-insensitive | `[[Us` finds `users` |
| 012 | completion-in-text | Completion in paragraph text |
| 013 | completion-in-property | `- ref: [[` |
| 014 | completion-after-close | After `]]` — no completion |
| 015 | fuzzy-match | `[[usr` → `users` |

### Hover

| # | Test | Description |
|---|------|----------|
| 001 | hover-ref | Hover on a reference `[[users]]` |
| 002 | hover-definition | Hover on a definition `# Users [[users]]` |
| 003 | hover-with-kind | `[[Table.users]]` |
| 004 | hover-outside | Hover outside a reference — null |
| 005 | hover-broken-ref | Hover on a broken reference |
| 006 | hover-shows-kind | Shows the Kind in hover |
| 007 | hover-shows-properties | Shows the object's properties |
| 008 | hover-cross-file | Hover on a cross-file reference |
| 009 | hover-namespace | `[[ns.id]]` |
| 010 | hover-partial | Hover in the middle of an ID |

### Definition

| # | Test | Description |
|---|------|----------|
| 001 | basic | Go to definition |
| 002 | on-definition | On the definition — itself |
| 003 | with-kind | `[[Kind.id]]` |
| 004 | cross-file | Definition in another file |
| 005 | not-found | Nonexistent ID — null |
| 006 | namespace-qualified | `[[namespace.id]]` |
| 007 | multiple-definitions | ID defined twice (return all) |
| 008 | definition-from-ref | `- ref: [[#id]]` |
| 009 | hash-local | `[[#localId]]` |
| 010 | definition-from-property | From an object property |

### References

| # | Test | Description |
|---|------|----------|
| 001 | find-all | Find all references to an ID |
| 002 | include-definition | Include the definition |
| 003 | cross-file | References in other files |
| 004 | no-references | No references — empty array |
| 005 | multiple-same-file | Several references in one file |

### Document Symbol

| # | Test | Description |
|---|------|----------|
| 001 | all-objects | All objects in the document |
| 002 | nested | Nested objects |
| 003 | with-kinds | Different Kinds → different SymbolKind |
| 004 | empty-doc | Empty — empty array |
| 005 | properties | Object properties as children |

---

## Running

```bash
# Rust
cargo test --test lsp

# TypeScript
npm test -- --grep "lsp"

# Python
pytest tests/test_lsp_microtests.py
```

## Severity codes

| Code | Meaning |
|------|---------|
| 1 | Error |
| 2 | Warning |
| 3 | Information |
| 4 | Hint |

## QMD diagnostic codes

| Code | Message | Severity |
|------|---------|----------|
| QMD001 | Object not found | Error |
| QMD002 | Ambiguous reference | Warning |
| QMD003 | Duplicate ID | Error |
| QMD004 | Invalid reference syntax | Error |
| QMD005 | Orphan definition | Hint |
| QMD006 | Circular reference | Warning |
