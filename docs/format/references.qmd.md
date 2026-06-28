# Reference [[reference: SyntaxConcept]]

- depends: [[#object]], [[#field]], [[#workspace]]

## Description [[description: text]]

References connect objects in the QMDC graph. A reference is a link to another object in the workspace, written as `[[#id]]` in field values. References enable building a graph of interconnected objects.

The process has two phases:

1. Parsing — references remain as strings like `"[[#users]]"`
2. Resolve — after loading all objects, references are validated and indexed

## Syntax [[syntax: text]]

A reference is defined via `[[#...]]` inside field values:

```markdown example
- target: [[#users]]
- service: [[#api_gateway]]
```

Full format:

```markdown example
[[#workspace:namespace:Kind:id]]
```

Components (all optional except `id`):

- `workspace` — workspace name (defaults to current)
- `namespace` — architectural slice (deliverables, storage, domain, etc.)
- `Kind` — object type (for collision resolution)
- `id` — object identifier (required)

Short forms:

```markdown example
[[#users]]                              # short form (current workspace/namespace)
[[#Table:users]]                        # with Kind (current namespace)
[[#storage:Table:users]]                # with namespace
[[#myproject:storage:Table:users]]      # full form
```

Rules:

- References work only in values (not in headings)
- Start with `#` inside `[[...]]`
- In headings, `[[...]]` without `#` is a definition (object or field)

## Typed Edges [[typed_edges: text]]

Every edge in the graph carries an `edge_type` — a user-defined string describing the relationship semantics.

For inline fields, `edge_type` equals the field name:

```markdown example
## Payment [[payment: Service]]

- depends: [[#auth]]
- database: [[#payments_db]]
```

| source_id | source_field | target_id   | edge_type |
| --------- | ------------ | ----------- | --------- |
| payment   | depends      | auth        | depends   |
| payment   | database     | payments_db | database  |

For text field preambles, `edge_type` comes from the preamble key, while `source_field` is the text field name:

```markdown example
### Rationale [[rationale: text]]

- about: [[#checkout_flow]]
- depends: [[#order_svc]], [[#payment_svc]]

This service handles payments...
```

| source_id | source_field | target_id     | edge_type |
| --------- | ------------ | ------------- | --------- |
| payment   | rationale    | checkout_flow | about     |
| payment   | rationale    | order_svc     | depends   |
| payment   | rationale    | payment_svc   | depends   |

Preamble rules:

- A preamble is a markdown list at the beginning of a text field where ALL items have the format `- key: [[#ref]]`
- All-or-nothing: if even one item is invalid, the entire preamble is ignored and references are extracted the standard way (`edge_type` = `source_field`)
- Separated from the rest of the text by a blank line
- Preamble lines remain in the raw text value — they are not stripped during parsing

SQL query examples:

```sql example
-- All "depends" relationships (typed edges)
SELECT s.__id, t.__id FROM edges e
JOIN objects s ON e.source_id = s.__global_id
JOIN objects t ON e.target_id = t.__global_id
WHERE e.edge_type = 'depends'

-- All "about" links from text fields
SELECT s.__id, e.source_field, t.__id FROM edges e
JOIN objects s ON e.source_id = s.__global_id
JOIN objects t ON e.target_id = t.__global_id
WHERE e.edge_type = 'about'

-- Preamble edges (where edge_type differs from source_field)
SELECT s.__id, e.source_field, t.__id, e.edge_type FROM edges e
JOIN objects s ON e.source_id = s.__global_id
JOIN objects t ON e.target_id = t.__global_id
WHERE e.edge_type != e.source_field
```

## Hierarchical Dot-Path References [[hierarchical_refs: text]]

References use full hierarchical dot-paths to target child objects:

```markdown example
## Team [[team]]

### Members [[members: [User]]]

#### Alice [[alice]]

- role: admin

## Report [[report]]

- author: [[#team.members.alice]]
```

The dot-path `[[#team.members.alice]]` resolves to the child object with hierarchical ID `team.members.alice`.

## Field-Level References [[field_level_refs: text]]

A dot-path that extends beyond an object ID references a field on that object:

```markdown example
- alice_role: [[#team.members.alice.role]]
```

This references the `role` field on the object `team.members.alice`.

**Disambiguation:** If a dot-path resolves both as an object ID and as a field-path on a parent object, the validator produces an `ambiguous_field_reference` error.

## Resolve Process [[resolve_process: text]]

Two-phase process for working with references:

Phase 1 — Parsing: references remain as strings in the parse result:

```json example
{
  "__id": "orders",
  "__kind": "Table",
  "user_ref": "[[#users]]"
}
```

Phase 2 — Validation and indexing (after loading all QMD.md files):

1. Indexing: the workspace builds an index of all objects by all possible paths:

    ```python example
    index = {
        "users": users_object,
        "Table:users": users_object,
        "storage:Table:users": users_object,
        "myproject:storage:Table:users": users_object
    }

    # Additionally, a by_local_id index for fallback resolution:
    by_local_id = {
        "alice": [team_members_alice_object],   # __id = "team.members.alice", __local_id = "alice"
        "users": [users_object],                # __local_id = "users" (same as __id when no dot)
    }
    ```

2. Validation: checking that target objects exist:

    ```python example
    "[[#users]]"   → parse() → lookup("users")   → found ✓
    "[[#missing]]" → parse() → lookup("missing") → not found → try __local_id → not found ⚠️
    "[[#child]]"   → parse() → lookup("child")   → not found → try __local_id("child") → found 1 match ✓
    "[[#name]]"    → parse() → lookup("name")    → not found → try __local_id("name") → found 3 matches ⚠️ ambiguous
    ```

3. Building the edge graph: the workspace knows who references whom.

How references are used for graph navigation is an implementation decision:

- Option A: references remain strings, navigation via Workspace API
- Option B: references replaced with direct pointers/IDs in memory
- Option C: references remain strings, resolved on demand at access time

The QMD.md specification defines only:

- Reference syntax `[[#...]]`
- Parsing rules (references = strings)
- Validation rules (existence checks)

It does not define: the navigation mechanism (that is up to the library/framework).

Problem handling — all reference issues are warnings (the graph continues loading):

- Object not found (by both `__id` and `__local_id`): `[[#missing]]` → reference remains a string, warning in logs
- Object found via `__local_id` fallback (unambiguous): `[[#child]]` where `__id` is `parent.child` → resolves successfully
- Multiple `__local_id` matches: `[[#name]]` where several objects have `__local_id: "name"` → unresolved reference, warning
- ID collision without Kind: `[[#users]]` (both Table:users and Entity:users exist) → unresolved reference, warning
- Query with 0 results: `[[#users.columns[name=unknown]]]` → may be interpreted as `null` or warning
- Query with >1 result for a single ref: `[[#users.columns[type=bigint]]]` (3 found) → unresolved reference, warning

Philosophy: reference problems must not break the entire graph. Validation produces warnings, the object loads, references remain as strings.

Strict mode (optional): an implementation may provide a strict mode where warnings become errors.

## Examples [[examples: text]]

Database FK example:

```markdown example
## Users Table [[users: Table]]

- name: users

### Columns [[columns: [Column]]]

#### ID [[id_col]]

- name: id
- type: bigserial
- primary_key: true

## Orders Table [[orders: Table]]

- name: orders

### Foreign Keys [[foreign_keys: [ForeignKey]]]

#### User FK [[fk_user]]

- column: user_id
- references: [[#users.columns[name=id]]]
- on_delete: cascade
```

Microservice architecture:

```markdown example
## API Gateway [[gateway: Service]]

- port: 8080

### Dependencies [[dependencies]]

- [[#user_service]]
- [[#order_service]]
- [[#payment_service]]

### Routes [[routes: [Route]]]

#### Users Route [[route_users]]

- path: /api/users/*
- target: [[#user_service]]
- methods: [GET, POST, PUT, DELETE]

## User Service [[user_service: Service]]

- port: 8081
- database: [[#storage:Database:users_db]]
```

Knowledge graph:

```markdown example
## JavaScript [[javascript: Article]]

- title: JavaScript

### Related [[related]]

- [[#typescript]]
- [[#nodejs]]
- [[#react]]

## TypeScript [[typescript: Article]]

- title: TypeScript
- based_on: [[#javascript]]

## Node.js [[nodejs: Article]]

- title: Node.js
- runtime_for: [[#javascript]]
```

Relationship types:

One-to-one (1-1):

```markdown example
## User [[alice]]

- name: Alice
- profile: [[#alice_profile]]

## Profile [[alice_profile]]

- theme: dark
- language: en
```

One-to-many (1-*):

```markdown example
## Team [[team]]

- name: Engineering

### Members [[members: [User]]]

#### Alice [[alice]]

- role: admin

#### Bob [[bob]]

- role: dev
```

Many-to-many (*-*):

```markdown example
## User [[alice]]

- name: Alice
- roles: [[[#admin]], [[#dev]]]

## Role [[admin]]

- name: Admin
- permissions: [read, write, delete]

## Role [[dev]]

- name: Developer
- permissions: [read, write]
```

Self-reference:

```markdown example
## Category [[electronics]]

- name: Electronics
- parent: null

## Category [[phones]]

- name: Phones
- parent: [[#electronics]]

## Category [[smartphones]]

- name: Smartphones
- parent: [[#phones]]
```

Composition vs association:

Composition (ownership) — child object has no meaning without the parent:

```markdown example
## Order [[order_1]]

- total: 1000

### Items [[items: [OrderItem]]]

#### Item 1

- product: Laptop
- price: 1000
```

Association (reference) — objects are independent, the link does not imply ownership:

```markdown example
## Order [[order_1]]

- customer: [[#alice]]
- products: [[[#laptop]], [[#mouse]]]
```

## Rules [[rules: text]]

- References work only in field values. In headings, `[[...]]` without `#` is a definition.
- In text fields (`:text`), references `[[#id]]` are validated for existence but remain as part of the text — they are not resolved into objects.
- In YAML pipe blocks (`|`), references are not parsed at all — content remains plain text.
- In inline code (`` `[[#ref]]` ``), references are not parsed.
- In fenced code blocks with the `example` modifier, references are not parsed.
- Kind-qualified references (`[[#Kind:id]]`) are required when two objects share the same ID but have different Kinds. Without Kind, the reference is ambiguous — a warning is produced.
- Cross-namespace format: `[[#namespace:id]]` or `[[#namespace:Kind:id]]` for referencing objects in other namespaces.
- Full cross-workspace format: `[[#workspace:namespace:Kind:id]]`.
