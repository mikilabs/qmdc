# QMDC: Guide for AI Agents [[qmdc_guide]]

Practical guide to the QMD.md format for AI agents

- version: 2.0.0

⚠️ **Important:** In QMD.md, field and object order is strictly preserved as written (insertion order). All parsers guarantee this.

## Metamodel [[metamodel: Metamodel]]

### Anchor Kinds [[anchor_kinds: text]]

| kind       | definition                                                              |
| ---------- | ----------------------------------------------------------------------- |
| Syntax     | How QMD.md text maps to structure — objects, fields, arrays, text blocks, data types |
| Reference  | The linking mechanism — `[[#id]]` and its variants                       |
| Workspace  | Multi-file project structure — namespaces, cross-file resolution         |
| Validation | Error detection, error types, pre-commit checks                          |
| CLI        | The `qmdc` commands — parse, rebuild, lint, query, validate              |

### Link Roles [[link_roles: text]]

| role      | meaning                                          |
| --------- | ------------------------------------------------ |
| `about`   | This section describes or explains X             |
| `depends` | This anchor relies on another anchor at runtime  |

## What Good QMD.md Looks Like [[good_qmdc: NarrativeDoc]]

- about: [[#syntax]], [[#reference]], [[#workspace]]

### Principles [[principles: text]]

A good QMD.md document starts with real things in your domain — not document structure. Your headings should be Parsers, Contracts, Modules — not Sections, Examples, Pages. Those real things are your anchors. Everything else — explanations, examples, walkthroughs — is narrative that links back to anchors via `about`.

Keep anchor kinds coarse. 5–15 is the sweet spot. If you have 30 kinds, you're modeling the document, not the domain. If you have 3, you probably merged things that deserve separate identity.

Use `depends` between anchors to show the dependency chain. Use `about` on narrative sections to say what they explain. If a section isn't `about` any anchor, ask yourself why it exists.

IDs should be guessable — `rust_parser`, not `parser_001`. Types should be PascalCase domain nouns — `Module`, not `ModuleSection`. Text fields for anything longer than a line.

Identity comes from Kind + position in the hierarchy, not from a bespoke globally-unique name. Top-level anchors get descriptive ids; nested children get simple, repeatable local ids (`description`, `constraints`) scoped by their parent's dot-path; homogeneous collections become `[Kind]` arrays instead of hand-named items.

When you're unsure which things are anchors, what the right granularity is, or whether two concepts should be merged or split — stop and ask. Propose your anchor list and dependency chain to the human, get explicit approval before writing the document. A wrong metamodel is worse than no metamodel: it creates structure that actively misleads. Iterate on the anchors first, write content second.

## Syntax [[syntax: Syntax]]

### Overview [[syntax_overview: text]]

QMD.md is a format for writing structured data in Markdown. You write regular Markdown with headings and lists, and the parser turns it into a graph of objects with references.

**Core idea:**

- Headings (`##`, `###`) = objects
- Lists (`- key: value`) = object fields
- `[[id]]` in a heading = object identifier
- `[[#id]]` in a value = reference to another object

### Creating Objects [[creating_objects: text]]

```markdown
## User [[alice]]

- name: Alice
- age: 30
- active: true
- score: 95.5
```

**Result:** object with `__id: "alice"`, fields `name`, `age`, `active`, `score`.

**Heading variants:**

- `## Title [[id]]` — explicit ID
- `## Title [[id: Kind]]` — with type (for validation)
- `## Title [[:Kind]]` — Kind only, ID auto-generated
- `## Title` — auto-generate ID from Title (lowercase, spaces → `_`)
- `## [[id]] Title` — ID before Title (also valid)

### Data Types [[data_types: text]]

```markdown
## Config [[config]]

- text: Hello World          # string
- number: 42                 # number
- float: 3.14                # number
- bool: true                 # boolean (lowercase only)
- empty: null                # null
- array: [a, b, c]          # array (YAML notation)
```

**Important:** `true`/`false`/`null` must be lowercase only. `True`, `FALSE`, `yes`, `no` are strings!

### Text Fields (Multiline Content) [[text_fields: text]]

```markdown
## Article [[article]]

### Content [[content]]

This is **multi-line** content.

You can use Markdown formatting here:
- Lists work
- Bold, italic, etc.
```

**Result:** field `content` with multiline text (Markdown preserved).

**References in text fields:** References `[[#id]]` inside text fields are validated for existence (broken links detected), but remain part of the text — they are not resolved into objects.

### When to Use Heading-Syntax Text Fields [[heading_syntax_rule: text]]

**Rule of thumb:** any field that could plausibly be more than a short scalar should use `### FieldName [[field: text]]` heading syntax instead of `- key: value`.

**Use heading-syntax (`### Field [[field: text]]`) for:**

- descriptions, responsibilities, rationale
- assumptions, deltas, deferred items
- public surfaces, summaries, explanations
- anything that might span multiple lines or contain markdown formatting

**Keep as inline fields (`- key: value`) for:**

- status, language, repo, version
- references (`- ref: [[#something]]`)
- short scalars (names, IDs, booleans, numbers)

**Example:**

```markdown
## Feature [[feature1: Feature]]

- status: planned
- priority: high

### Description [[description: text]]

This feature adds support for semantic search across
the entire workspace. It combines keyword matching with
dense vector embeddings for hybrid retrieval.

### Rationale [[rationale: text]]

Users need to find relevant objects without knowing
exact file paths or object IDs.
```

### YAML Multiline Strings [[yaml_multiline: text]]

```markdown example
## Config [[config]]

- description: |
    This is a multiline string
    using YAML pipe syntax.
    References like [[#something]] are NOT parsed here.
```

**Important:** References `[[#id]]` inside YAML pipe `|` blocks are NOT parsed by the QMDC parser — content remains plain text. This means they won't create edges in the graph or trigger broken_link errors in the target document.

### Nested Objects [[nested_objects: text]]

```markdown
## User [[user]]

- name: Alice

### Address [[address]]

- street: Main St
- city: NYC
```

**Result:** two objects:

1. `user` with field `address: "[[#user.address]]"` (reference to the child's full ID)
2. an object with `__id: "user.address"`, `__local_id: "address"`, fields `street`, `city`, and `__parent: "[[#user]]"`

**Key distinction:** QMD.md has no nested JSON objects! Everything is flat, connections via references.

**Hierarchical ID formation:** a child's effective `__id` is composed from its parent:

- Single nested child → `parent.child` (e.g. `user.address`)
- Object-array element → `parent.field.child` (e.g. `team.members.alice`)
- Exception: children of `__Workspace` and `__Namespace` objects keep flat IDs

**Reuse simple local ids under different parents.** Because a child's identity is parent-scoped, the same local id may — and should — repeat across parents. Prefer `[[description: text]]` and `[[constraints]]` under every object over hand-made globally-unique names:

```markdown example
## Run [[schema_run: KindSchema]]
### Description [[description: text]]   ← id resolves as schema_run.description

## Task [[schema_task: KindSchema]]
### Description [[description: text]]   ← id resolves as schema_task.description
```

Do NOT write `run_desc`/`task_desc` or prefix children with the parent name (`schema_run_constraints`) — the dot-path already carries the parent. Parent-scoped children never trigger `duplicate_id` (which fires only on identical FULL hierarchical `__id`s — see Error Types).

### Dot-ID Declarations [[dot_id_declarations: text]]

A TOP-LEVEL heading may declare a hierarchical ID directly — attaching the object to a parent defined elsewhere (even in another file or namespace):

```markdown example
# Operations [[my_service.operations]]

- count: 12
```

The workspace loader finds the parent by the ID prefix up to the last dot (`my_service`), injects `__parent: "[[#my_service]]"`, and the object behaves exactly like a nested child. If no object with the parent ID exists anywhere in the workspace, validation reports `broken_parent`.

Rules:

- Dot-IDs are legal **only on top-level headings**. A dot in a NESTED child's explicit ID is a parse error (`invalid_id_character`) — nested children get their dot-path composed automatically.
- Only the prefix up to the LAST dot must exist as an object (`a.b.c` requires `a.b`, not `a`).
- Works cross-file and cross-namespace.

### Arrays [[arrays: text]]

**Compact form (YAML):**

```markdown
- tags: [react, nodejs, typescript]
- ports: [8080, 8081, 8082]
```

**Multiline YAML form:**

```markdown
- long_list: [
    first_very_long_item,
    second_very_long_item,
    third_very_long_item
  ]
```

**Expanded form (lists):**

```markdown
### Members [[members]]

- Alice
- Bob
- Charlie
```

**Comma-separated references (no outer brackets):**

```markdown example
- deps: [[#auth]], [[#db]], [[#cache]]
```

Syntax choice is preserved in `__syntax` for lossless round-trip.

### Object Arrays via Subheadings [[object_arrays_subheadings: text]]

```markdown
## Team [[team]]

### Members [[members: [User]]]

#### Alice [[alice]]

- role: admin
- email: alice@ex.com

#### Bob [[bob]]

- role: dev
- email: bob@ex.com
```

**Result:**

- Object `team` with field `members: ["[[#team.members.alice]]", "[[#team.members.bob]]"]`
- Two objects with `__id: "team.members.alice"` / `"team.members.bob"`, `__parent: "[[#team]]"` and `__parent_field: "members"`

**Prefer `[Kind]` arrays for homogeneous collections.** When many items share a Kind and don't need to be referenced individually (lint rules, steps, columns), use a `[Kind]` array with a table — ids are auto-generated; do NOT hand-name each item as a separate top-level object (`rule_a`, `rule_b`, ...). If items WILL be referenced, name them with **simple local names**: an array child's name is just the last dot-path segment (`svc_a.rules.timeout_rule`), so use `[[timeout_rule]]` — never a parent-prefixed `[[svc_a_timeout_rule]]`. The same local name under different parents is fine.

### Object Arrays via Tables [[object_arrays_tables: text]]

```markdown
## Team [[team]]

### Members [[members: [User]]]

| name    | role      | email          |
| ------- | --------- | -------------- |
| Alice   | admin     | alice@ex.com   |
| Bob     | developer | bob@ex.com     |
| Charlie | designer  | charlie@ex.com |
```

**Result:** same thing — 3 objects with auto-generated positional IDs (`team.members.members_0`, `members_1`, `members_2`). These are the IDs to use when referencing a table row: `[[#team.members.members_0]]`. Positional ids shift if rows are reordered — name the item explicitly (subheading form) if it will be referenced.

**When to use tables:**

- ✅ Many homogeneous objects with simple fields
- ✅ Tabular data (configurations, lists)
- ❌ Objects with nested structures
- ❌ Objects with multiline fields

### Embedded YAML Blocks [[yaml_blocks: text]]

QMD.md supports embedded YAML blocks for migrating existing configurations.

````markdown
## Server [[server]]

### Configuration [[config: yaml]]

```yaml
database:
  host: localhost
  port: 5432
replicas: 3
```
````

**Result:** field `config` contains parsed YAML as a nested object. Data accessible directly: `server.config.database.host`.

If YAML is invalid — field is saved as text with `__parse_error`, graph continues loading.

### Auto-Detection Rules [[auto_detection: text]]

When the parser encounters a heading-syntax field, it determines the type:

1. Has child subheadings with `[[field_id]]`? → **Object array**
2. Has lists with `- key: value` (valid keys)? → **Nested object**
3. Has lists WITHOUT colons `- value`? → **Primitive array**
4. Has text but no valid lists? → **Text field**

When in doubt — explicitly specify the type:

```markdown
### Description [[desc: text]]        # text field

### Config [[config: object]]         # nested object

### Items [[items: [Item]]]           # object array

### Tags [[tags: array]]              # primitive array
```

### System Types [[system_types: text]]

- about: [[#workspace]]

QMD.md uses system types with the `__` prefix:

| Type | Description |
|------|-------------|
| `__Workspace` | Root project container |
| `__Namespace` | Logical grouping (subfolder) |
| `__Document` | Document container (auto-created if text blocks exist) |
| `__TextBlock` | Unstructured text (heading without `[[id]]` and without fields) |

User types: `User`, `Config`, `Table`, etc. (PascalCase).

Filtering: `objects.filter(obj => !obj.__kind?.startsWith("__"))` — business objects only.

`__Document` and `__TextBlock` are created by the parser automatically only. Explicit declaration `[[id: __Document]]` or `[[id: __TextBlock]]` is an error.

### Lossless Round-Trip [[round_trip: text]]

QMD.md supports lossless round-trip: `QMD.md → JSON → QMD.md` without data loss.

The parser stores metadata in system fields:

| Field | Description |
|-------|-------------|
| `__types` | Field data types (string, number, boolean, null, array) |
| `__syntax` | Notation syntax (yaml_array, markdown_list, table, headers, yaml_multiline, yaml_object) |
| `__level` | Heading level (1-6) |
| `__has_explicit_id` | `false` if ID was auto-generated (absent if ID is explicit) |
| `__comments` | Comments with semantic binding to fields |
| `__labels` | Original heading labels for fields |
| `__code_fences` | Metadata about fenced code blocks in TextBlock |

**`__comments` format:** array of `{after: "anchor", content: "raw markdown"}`.

- `after: "__self"` — comment on the object itself
- `after: "field_name"` — comment after a field
- `content` — raw markdown slice, parser does not interpret contents

**`__syntax` values and rebuild:**

| __syntax | Heading on rebuild |
|----------|-------------------|
| `multiline_text` | `### Label [[field: text]]` |
| `markdown_list` | `### Label [[field: array]]` |
| `yaml_object` | `### Label [[field: yaml]]` |
| `headers` | `### Label [[field: [Kind]]]` |
| `table` | `### Label [[field: [Kind]]]` |

## Reference [[reference: Reference]]

- depends: [[#syntax]]

### Overview [[reference_overview: text]]

References connect objects in the graph. They start with `#` inside `[[...]]`.

### Basic References [[basic_refs: text]]

```markdown example
## Order [[order1]]

- customer: [[#alice]]
- product: [[#laptop]]
```

### Array References [[array_refs: text]]

YAML reference arrays and expanded form:

```markdown example
## User [[alice]]

- roles: [[[#admin]], [[#dev]]]

## User [[bob]]

### Roles [[roles]]

- [[#dev]]
- [[#viewer]]
```

### Field References [[field_refs: text]]

References use full hierarchical dot-paths to target child objects and their fields:

```markdown example
## Team [[team]]

### Members [[members: [User]]]

#### Alice [[alice]]

- role: admin

## Report [[report]]

- author: [[#team.members.alice]]
- author_role: [[#team.members.alice.role]]
```

**Syntax:** `[[#parent.field.child]]` — hierarchical dot-path to a child object. `[[#parent.field.child.field_name]]` — references a field on that object.

**How resolution actually works:**

- An object dot-path resolves by **exact whole-string match** against an object's full hierarchical `__id`. Intermediate segments are not walked or validated — `[[#a.b.c]]` resolves iff an object with `__id` exactly `a.b.c` exists. Depth is unlimited.
- A field reference splits on the **last dot only**: everything before it must be a full object `__id`, and exactly one trailing segment names the field. Multi-segment field paths (`[[#obj.nested.deep_field]]` where `obj.nested` is not an object id) do not resolve.
- System fields (`__id`, `__file`, ...) cannot be referenced.
- Namespace-qualified field references (`[[#ns:obj.field]]`) are not supported — only local field refs.
- If a dot-path resolves both as an object `__id` AND as a field on the prefix object, validation reports `ambiguous_field_reference` (see Error Types).

There is no filter or wildcard syntax — forms like `[[#items[key=value]]]` are not part of the format and will not resolve. To reference a table row, use its auto-generated ID (`parent.field.field_<n>`, e.g. `[[#users.columns.columns_0]]`), or give the row an explicit name if it will be referenced.

### Kind-Qualified References [[kind_refs: text]]

When two objects have the same ID but different Kind, the reference must specify Kind:

```markdown example
## Users [[users: Table]]
- name: users

## Users [[users: Entity]]
- type: domain_model
```

```markdown example
- table_ref: [[#Table:users]]     # reference to table
- entity_ref: [[#Entity:users]]   # reference to entity
- ambiguous: [[#users]]           # ❌ ERROR!
```

### Cross-Namespace References [[namespace_refs: text]]

- about: [[#workspace]]

```markdown example
## API Service [[api_service: Service]]

- database: [[#storage:users]]                  # different namespace
- user_table: [[#other_ws:storage:users]]       # different workspace
- ledger: [[#other_ws::ledger]]                 # different workspace, any namespace
```

**Full format:** `[[#workspace:namespace:id]]` — the same grammar as `__global_id`, with an
optional `.field` suffix on the id. A reference and an identity are written the same way.

A reference is a right-aligned SUFFIX of that grammar; each qualifier you add narrows the
search:

- `[[#id]]` — this workspace: own namespace first, then any namespace of it
- `[[#namespace:id]]` — this workspace, that namespace
- `[[#workspace:namespace:id]]` — that workspace, that namespace
- `[[#workspace::id]]` — that workspace, ANY namespace (the empty middle segment ELIDES the
  namespace rather than asserting the workspace root)

Two rules follow from this, and both are enforced:

- **A cross-workspace reference must name its workspace.** A bare `[[#id]]` never reaches
  into a sibling workspace, not even when exactly one workspace holds that id.
- **A qualifier that matches more than one object is an `ambiguous_reference`, and builds no
  edge.** `[[#other_ws::ledger]]` is ambiguous if `other_ws` holds `ledger` in two
  namespaces — exactly as a bare `[[#ledger]]` is ambiguous within one workspace.

There is no `Kind` segment. Ids are unique within a namespace, so a Kind could only ever
disambiguate an already-invalid workspace; it was never part of identity, and
`[[#namespace:Kind:id]]` is not valid syntax.

### Resolution Order and __local_id [[resolution_order: text]]

Every object carries `__local_id` — the segment after the last dot of its hierarchical `__id` (for flat ids, the id itself). References resolve in this order:

1. Exact match on the full `__id` (including hierarchical dot-paths)
2. Field reference (last-dot split, prefix is a full `__id`)
3. Fallback match on `__local_id`, filtered by namespace

The `__local_id` fallback means `[[#alice]]` can resolve to `team.members.alice` when exactly one object with that local id exists in the namespace. If multiple objects share the local id, the reference is `ambiguous_reference` — fix it by using the full hierarchical ID (`[[#team.members.alice]]`).

### Rename Cascade [[rename_cascade: text]]

Renaming an object rewrites references to it AND to all its descendants: renaming `team` → `squad` rewrites `[[#team]]`, `[[#team.config]]`, `[[#team.members.alice]]`, and field refs like `[[#team.config.timeout]]`. Rewrites are token-bounded — `[[#teamwork]]` is never touched by renaming `team`. Descendant definitions themselves are not edited (they are anchored by `__local_id`).

### Reference Philosophy [[ref_philosophy: text]]

- about: [[#validation]]

Reference problems are reported with `severity: error`, but they are **non-fatal**: unlike syntax errors, they never prevent the graph from being built.

- Object not found → the object still loads; the reference remains a plain string; `broken_link` error reported
- ID collision without Kind → unresolved reference; `ambiguous_reference` error reported
- Broken links don't break the entire graph — validation collects them into a report instead of aborting

### Where References Are Not Parsed [[refs_not_parsed: text]]

References `[[#id]]` inside text fields are validated for existence (broken links detected), but remain part of the text — they are not resolved into objects.

References `[[#id]]` inside YAML pipe `|` blocks are NOT parsed — content remains plain text. They won't create edges in the graph or trigger broken_link errors.

References inside inline code (`` `[[#ref]]` ``) are NOT parsed.

References inside fenced code blocks with `example` modifier are NOT parsed.

### Typed Edges [[typed_edges: text]]

Every edge in the graph carries an `edge_type` — a user-defined string describing the relationship.

**For inline fields**, `edge_type` equals the field name:

```markdown example
## Payment [[payment: Service]]

- depends: [[#auth]]
- database: [[#payments_db]]
```

Creates edges: `(payment, depends, auth, edge_type="depends")` and `(payment, database, payments_db, edge_type="database")`.

**For text field preambles**, `edge_type` comes from the preamble key, while `source_field` is the text field name:

```markdown example
### Rationale [[rationale: text]]

- about: [[#checkout_flow]]
- depends: [[#order_svc]], [[#payment_svc]]

This service handles payments...
```

Creates edges:

- `(payment, source_field="rationale", checkout_flow, edge_type="about")`
- `(payment, source_field="rationale", order_svc, edge_type="depends")`
- `(payment, source_field="rationale", payment_svc, edge_type="depends")`

**Preamble rules:**

- Must start at the beginning of the text field value
- ALL list items must be valid `- key: [[#ref]]` fields (all-or-nothing)
- Separated from the rest of the text by a blank line
- Preamble lines stay in the raw text value — no stripping

**Edges table schema:**

```sql
edges (source_id, source_field, target_id, edge_type, __workspace)
UNIQUE(source_id, source_field, target_id, edge_type)
```

**Querying typed edges:**

```bash
# All "depends" relationships
qmdc query . "SELECT s.__id, t.__id FROM edges e JOIN objects s ON e.source_id = s.__global_id JOIN objects t ON e.target_id = t.__global_id WHERE e.edge_type = 'depends'"

# Preamble edges (where edge_type differs from source_field)
qmdc query . "SELECT s.__id, e.source_field, t.__id, e.edge_type FROM edges e JOIN objects s ON e.source_id = s.__global_id JOIN objects t ON e.target_id = t.__global_id WHERE e.edge_type != e.source_field"
```

## Workspace [[workspace: Workspace]]

- depends: [[#reference]]

### Structure [[workspace_structure: text]]

A workspace is a folder with multiple QMD.md files that can reference each other.

```text
my-project/
├── readme.qmd.md              # Workspace root (__Workspace)
├── users.qmd.md
├── storage/
│   ├── readme.qmd.md          # Namespace "storage" (__Namespace)
│   ├── tables.qmd.md
│   └── indexes.qmd.md
└── api/
    ├── readme.qmd.md          # Namespace "api" (__Namespace)
    └── endpoints.qmd.md
```

### Defining a Workspace [[defining_workspace: text]]

**File `readme.qmd.md` in project root:**

```markdown
# My Project [[myproject:__Workspace]]

- description: Project description
- version: 1.0
```

`__Workspace` in Kind indicates this is the workspace root.

### Defining a Namespace [[defining_namespace: text]]

**File `storage/readme.qmd.md`:**

```markdown
# Storage Layer [[storage:__Namespace]]

- description: Database schema and storage
```

`__Namespace` in Kind indicates this folder is a namespace.

**All files in the folder automatically inherit workspace and namespace.**

### Cross-File References [[cross_file_refs: text]]

- about: [[#reference]]

**File `api/endpoints.qmd.md`:**

```markdown example
## Get Users [[get_users: Endpoint]]

- method: GET
- path: /api/users
- returns: [[#storage:Table:users]]
```

**File `storage/tables.qmd.md`:**

```markdown example
## Users [[users: Table]]

- columns: [[[#id_col]], [[#email_col]]]
```

The parser automatically:

1. Finds all objects in all files
2. Indexes them by `workspace:namespace:id` (its `__global_id`)
3. Validates all references
4. Reports broken links

### Object Metadata [[object_metadata: text]]

When parsing a workspace, each object gets:

```json example
{
  "__id": "users",
  "__kind": "Table",
  "__file": "storage/tables.qmd.md",
  "__line": 5,
  "__workspace": "[[#myproject]]",
  "__namespace": "[[#storage]]",
  "name": "users"
}
```

- `__file` — file path, relative to the workspace root (under `-w`, prefixed by the workspace id; the output's `workspaces` list gives each workspace's location on disk)
- `__line` — line number
- `__workspace` — reference to `__Workspace` object
- `__namespace` — reference to `__Namespace` object (or `null` for root)

### Dynamic Blocks [[dynamic_blocks: text]]

- about: [[#cli]]

Dynamic blocks — a mechanism for executing SQL queries against workspace data and displaying results in documents.

**Reusable query object:**

```markdown example
## Get Tables [[get_tables: Query]]

- sql: SELECT __id, __label FROM objects WHERE __kind = 'Table'
```

**Reference to Query:**

Use a `table` code block with `query: [[#get_tables]]` to reference a Query object.

**Inline SQL:**

Use a `table` code block with `sql: SELECT __id, __kind FROM objects` for inline queries.

**Scope parameter:**

- `scope: workspace` (default) — filter by current workspace
- `scope: all` — data from all workspaces

**Block types:** `table` (HTML table), `diagram` (D2/Mermaid, future), `chart` (charts, future).

## Validation [[validation: Validation]]

- depends: [[#reference]], [[#syntax]]

### Commands [[validation_commands: text]]

- about: [[#cli]]

```bash
# 1. Check file syntax
qmdc parse -i file.qmd.md > /dev/null || exit 1

# 2. Validate workspace (returns JSON array of errors)
qmdc workspace validate ./my-project
# If no errors — returns []
# If errors exist — returns array of error objects, exit 1

# 3. Check all files in a directory
for file in $(find ./docs -name "*.qmd.md"); do
    qmdc parse -i "$file" > /dev/null || echo "❌ $file"
done
```

### Error Format [[validation_error_format: text]]

`qmdc workspace validate` returns a JSON array:

```json example
[
  {
    "type": "broken_link",
    "message": "Object 'xyz' not found",
    "file": "file.qmd.md",
    "line": 5,
    "objectId": "abc",
    "reference": "[[#xyz]]",
    "severity": "error"
  }
]
```

### Error Types [[validation_error_types: text]]

| Code | Description |
|------|-------------|
| `broken_link` | Reference `[[#id]]` to a non-existent object |
| `duplicate_id` | Two objects with the same FULL hierarchical `__id` (parent-scoped children with the same local id never collide) |
| `ambiguous_reference` | Reference `[[#id]]` could point to multiple objects |
| `broken_parent` | Parent object not found for a dot-ID declaration (see Dot-ID Declarations) |
| `ambiguous_field_reference` | Dot-path resolves both as an object `__id` and as a field on the prefix object |
| `nested_workspace` | Workspace inside another workspace, left out of the result; compose both with `-w` to include it |
| `workspace_in_wrong_file` | `__Workspace`/`__Namespace` declared outside `readme.qmd.md` |
| `structured_in_textblock` | Structured element inside `__TextBlock` |
| `multiple_definitions` | Heading contains more than one `[[...]]` |
| `ordered_list_in_array` | Numbered list in heading-syntax array (bullet lists only) |
| `table_in_array` | Markdown table under a primitive array field (use a bullet list, or declare an object array) |
| `extra_table_in_array` | A second Markdown table under one object-array heading (only the first feeds the array) |
| `mixed_array` | An object array fed by a table that also has heading elements (pick one form), or a declaration inside a primitive `[[field: array]]` (a heading is not a list item, and the declared kind still decides — no object is created) |
| `nested_subitems` | Nested lists `- key:\n  - item` (forbidden) |
| `block_in_inline_field` | An indented block under a field that already has a value (YAML multiline `- key: \|` is fine) |
| `unsupported_number_format` | A value that looks like a number QMD.md cannot carry: not `-?[0-9]+(\.[0-9]+)?`, or a magnitude outside `1e-4`…2^53-1. The text is kept as a String; quote it to say a String was meant. Carries a `hint` field naming what to write — the only error that does |
| `explicit_system_type` | Explicit declaration of `[[id: __Document]]` or `[[id: __TextBlock]]` |
| `mixed_field_keys` | Mix of valid and invalid keys in one object |
| `invalid_id_character` | Dot in a NESTED heading's explicit ID (dot-IDs are legal only on top-level headings) |
| `dangling_field` | Heading-syntax field (`text`, `array`, `yaml`, ...) with no parent object at a higher heading level |
| `invalid_map_entry` | List item inside `[[field: map]]` that is not a valid `key: value` pair |
| `invalid_map_content` | Content inside `[[field: map]]` that is not a bullet list of `key: value` items |
| `wrapped_field_value` | A field value or array element continued on an indented second line (a value is written on one line; use `- key: \|` or a text field) |

### Pre-Commit Checklist [[pre_commit_checklist: text]]

Before saving a QMD.md file:

- ✅ All objects have `[[id]]` (or are explicitly auto-generated)
- ✅ All field keys are valid (`[a-zA-Z][a-zA-Z0-9_]*`)
- ✅ `true`/`false`/`null` in lowercase
- ✅ No patterns like `- key:\n  - item` (use YAML or heading-syntax)
- ✅ No numbered lists in heading-syntax arrays
- ✅ Verified via `qmdc parse -i file.qmd.md`
- ✅ If workspace — verified via `qmdc workspace validate .`
- ✅ All references exist
- ✅ No ambiguous references (Kind specified on collision)

### What Is Normal [[validation_normals: text]]

- ⚠️ Broken links in examples (users, orders, User, Table) — normal
- ⚠️ Broken links in types (text, id, Structure, Method) — normal
- ⚠️ References NOT parsed in inline code (`` `[[#ref]]` ``) — normal
- ⚠️ References NOT parsed in fenced code blocks with `example` modifier — normal
- ⚠️ References NOT parsed in YAML multiline `|` blocks — normal

**The `example` modifier for code blocks:**

To show QMD.md code examples in documentation, use the `example` modifier:

```json example
{
  "__id": "user",
  "__label": "User",
  "name": "Alice",
  "profile": "[[#user_profile]]"  // ← this reference is not parsed
}
```

In `json example` blocks, `[[#id]]` references remain text and don't create broken_link errors.

## CLI [[cli: CLI]]

- depends: [[#workspace]], [[#validation]]

### Parse — QMD.md → JSON [[cmd_parse: text]]

Converts QMD.md to JSON.

```bash
# File → stdout
qmdc parse -i file.qmd.md

# Syntax check (exit 1 on error)
qmdc parse -i file.qmd.md > /dev/null

# Stdin → stdout
echo "## Test [[test]]" | qmdc parse

# Without metadata
qmdc parse -i file.qmd.md --no-comments --no-syntax

# Rust: output format (minimal, standard, full)
qmdc parse -i file.qmd.md --format full
```

### Rebuild — JSON → QMD.md [[cmd_rebuild: text]]

Converts JSON back to QMD.md (lossless round-trip).

```bash
qmdc rebuild -i data.json
qmdc rebuild -i data.json -o doc.qmd.md
```

### Formatting (parse → rebuild) [[cmd_lint: text]]

There is no separate `lint` command. Pipe through parse → rebuild to format QMD.md to canonical form:

```bash
# Canonical formatting (lossless round-trip)
qmdc parse -i doc.qmd.md | qmdc rebuild
```

### Workspace Parse [[cmd_workspace_parse: text]]

- about: [[#workspace]]

Parses entire workspace to JSON.

```bash
qmdc workspace parse ./my-project -o workspace.json
qmdc workspace parse ./my-project --format full

# Compose workspaces that do not share a parent directory (repeatable, every path a peer)
qmdc workspace parse -w ~/checkouts/repo_a -w /srv/repo_b
```

### Workspace Validate [[cmd_workspace_validate: text]]

- about: [[#workspace]], [[#validation]]

Validates workspace — broken links, duplicate IDs, ambiguous references.

```bash
# Returns JSON array of errors ([] if all ok)
qmdc workspace validate ./my-project

# A project spanning several repositories, composed in place
qmdc workspace validate -w ~/checkouts/repo_a -w /srv/repo_b

# Exit code: 0 if no errors, 1 if errors exist, 2 if the invocation was refused
```

### Query — SQL Queries [[cmd_query: text]]

- about: [[#workspace]]

SQL queries against workspace via SQLite.

```bash
# Find all objects of type Table
qmdc query ./my-project "SELECT __id, __kind, __file FROM objects WHERE __kind = 'Table'"

# Find all references to an object
qmdc query ./my-project "SELECT source_id, field_name FROM edges WHERE target_id = 'users'"

# Count objects by type
qmdc query ./my-project "SELECT __kind, COUNT(*) as count FROM objects GROUP BY __kind"

# Query via Query object (reference to [[id:Query]] in workspace)
qmdc query ./my-project "#all_services"

# JSON output format
qmdc query ./my-project "SELECT * FROM objects LIMIT 10" --format json
```

**Available tables:**

- `objects` — all objects (`__id`, `__kind`, `__label`, `__file`, `__line`, `data` JSON)
- `edges` — all graph edges (`source_id`, `source_field`, `target_id`, `edge_type`, `__workspace`)

### Parsers [[parsers: text]]

Three parser implementations, all providing the same `qmdc` CLI:

- **Python** (`qmdc-py`) — reference implementation, `uv pip install -e ./qmdc-py`
- **TypeScript** (`qmdc-ts`) — for Node.js/browser, `npm install`
- **Rust** (`qmdc-rs`) — production, LSP server, `cargo build --release`

### Agent Workflow [[agent_workflow: text]]

- about: [[#validation]]

```bash
# 1. Create/edit file
echo "## User [[user]]
- name: Alice" > users.qmd.md

# 2. Check syntax
qmdc parse -i users.qmd.md > /dev/null
if [ $? -ne 0 ]; then
    echo "Syntax error!"
    exit 1
fi

# 3. If workspace — validate references
qmdc workspace validate .
if [ $? -ne 0 ]; then
    echo "Validation error!"
    exit 1
fi

# 4. Format (optional)
qmdc lint -i users.qmd.md -o users.qmd.md
```

## Common Errors [[common_errors: NarrativeDoc]]

- about: [[#validation]], [[#syntax]]

### Missing Explicit ID [[err_missing_id: text]]

- about: [[#syntax]]

```markdown
## User

- name: Alice
```

ID is auto-generated from Title → `__id: "user"`, but `[[id]]` is not explicitly set. Rebuild format may change. Always use explicit IDs:

```markdown
## User [[user]]

- name: Alice
```

### Invalid Field Key [[err_invalid_key: text]]

- about: [[#syntax]]

```markdown
## User [[user]]

- First Name: Alice     # space in key → markdown text!
- my-key: value         # hyphen in key → markdown text!
- 2name: value          # starts with digit → markdown text!
```

Valid key: `[a-zA-Z][a-zA-Z0-9_]*` (starts with letter, only letters/digits/underscore).

If an object mixes valid and invalid keys — error `mixed_field_keys` is generated.

Correct:

```markdown
## User [[user]]

- first_name: Alice
- firstName: Alice
- my_key: value
```

### Case-Sensitive Types [[err_case_types: text]]

- about: [[#syntax]]

```markdown
- active: True    # will be string "True"!
- empty: NULL     # will be string "NULL"!
```

Must be lowercase:

```markdown
- active: true    # boolean
- empty: null     # null
```

### Broken Link [[err_broken_link: text]]

- about: [[#validation]], [[#reference]]

```markdown example
## Order [[order1]]

- user: [[#nonexistent]]   # object doesn't exist!
```

How to find:

```bash
qmdc workspace validate .
# Returns JSON with broken_link error
```

Check object existence before creating a reference:

```bash
qmdc query . "SELECT __id FROM objects WHERE __id = 'nonexistent'"
# → empty result = object not found
```

### Ambiguous Reference [[err_ambiguous_ref: text]]

- about: [[#validation]], [[#reference]]

```markdown example
## Users [[users: Table]]
...

## Users [[users: Entity]]
...

## Order [[order]]
- ref: [[#users]]    # ambiguous!
```

Fix by specifying Kind:

```markdown example
- table_ref: [[#Table:users]]
- entity_ref: [[#Entity:users]]
```

### Nested Lists in Fields [[err_nested_lists: text]]

- about: [[#syntax]]

```markdown
## Config [[config]]

- items:
  - first
  - second
```

Fix with YAML array or heading-syntax:

```markdown
## Config [[config]]

- items: [first, second]
```

Or:

```markdown
## Config [[config]]

### Items [[items: array]]

- first
- second
```

### Numbered Lists in Arrays [[err_numbered_lists: text]]

- about: [[#syntax]]

```markdown
### Steps [[steps: array]]

1. First step
2. Second step
```

Only bullet lists are allowed:

```markdown
### Steps [[steps: array]]

- First step
- Second step
```

## Practical Examples [[examples: NarrativeDoc]]

- about: [[#syntax]], [[#reference]]

### Database Schema [[uc_database: text]]

- about: [[#syntax]], [[#reference]]

Tables with columns, indexes, and references:

```markdown example
## Users [[users: Table]]

### Columns [[columns: [Column]]]

| name  | type    | nullable |
| ----- | ------- | -------- |
| id    | bigint  | false    |
| email | varchar | false    |
| age   | int     | true     |

### Indexes [[indexes: [Index]]]

#### Email Index [[email_idx]]

- columns: [[[#users.columns.columns_1]]]
- unique: true
```

Table rows get positional auto-IDs (`columns_0`, `columns_1`, ...). To make row references robust, use the subheading form and name each column instead.

### Microservice Architecture [[uc_microservices: text]]

- about: [[#syntax]], [[#reference]]

API Gateway with routes and dependencies:

```markdown example
## API Gateway [[gateway: Service]]

- port: 8080
- dependencies: [[[#user_service]], [[#order_service]]]

### Routes [[routes: [Route]]]

#### Users [[users_route]]

- path: /api/users/*
- target: [[#user_service]]
- methods: [GET, POST, PUT]
```

### Configuration [[uc_config: text]]

- about: [[#syntax]]

Database and cache settings:

```markdown
## Production [[prod: Config]]

### Database [[db]]

- host: db.prod.example.com
- port: 5432
- ssl: true

### Cache [[cache]]

- provider: redis
- nodes: [redis1.prod, redis2.prod, redis3.prod]
- ttl: 3600
```
