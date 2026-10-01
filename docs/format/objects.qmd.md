# Object [[object: SyntaxConcept]]

- depends: [[#heading]]

## Description [[description: text]]

An object is a named data structure with a set of fields (key-value pairs). Objects are defined via Markdown headings at any level. The result of parsing a QMD.md document is an array of objects.

## System Fields [[system_fields: text]]

Every object may contain these system fields:

| Field | Description |
|-------|-------------|
| `__id` | Unique identifier |
| `__label` | Human-readable name from heading Title |
| `__kind` | Object type: user (`User`, `Config`) or system (`__Document`, `__TextBlock`, `__Workspace`, `__Namespace`) |
| `__workspace` | Reference to the `__Workspace` object (set automatically on graph load) |
| `__namespace` | Reference to the `__Namespace` object (optional, null for root objects) |
| `__container` | Reference to the `__Document` container, if the document has text blocks |
| `__parent` | Reference to parent object, if nested inside another object |
| `__parent_field` | Field name in parent through which this object is accessed |
| `__local_id` | Local part of a hierarchical ID (segment after the last dot) |
| `__comments` | Array of comments with semantic binding to object/fields |
| `__types` | Field data types for round-trip |
| `__syntax` | Field syntax metadata for round-trip |
| `__level` | Heading level (1–6+) for lossless rebuild |
| `__has_explicit_id` | `false` if `[[id]]` was auto-generated (absent when explicit) |
| `__file` | File path relative to the base of the workspace parse; under `-w`, prefixed by the workspace id (workspace parsing only) |
| `__line` | Line number where the object is defined (for LSP) |

Notes:

- `__workspace` and `__namespace` are references set automatically on graph load
- `__file` and `__line` are added only during workspace parsing
- `__container` is created only if the document contains text blocks
- `__parent` and `__parent_field` are created automatically for objects inside array sections
- `__comments` is created only if the object has comments
- `__types` is created if the object has fields with types other than string
- `__syntax` is created only if the object has fields with alternative syntax
- `__has_explicit_id` is created only when `false`

## System Types [[system_types: text]]

QMD.md documents can contain free text between objects. For lossless rebuild, the parser creates system objects:

`__Document` — created when the document contains text blocks outside objects. Contains a `content` field with an array of references to objects and text blocks in order of appearance.

`__TextBlock` — created for headings without `[[id]]` and without fields, and for the content above a document's first heading. Contains `content` and optionally `__code_fences` (metadata about fenced code blocks). `content` is the block's source text verbatim: from its first line — the heading, or the first non-blank line above the first heading — to the line before the heading that ends it, with blank lines at either end dropped and a trailing `\r` removed from each line. Every construct in that range is kept as written: a blockquote with its `>`, a list with its bullets, an HTML block, the underline of a setext heading, the markup inside the heading. Any content above the first heading opens the leading block, except HTML comments alone, which are ignored. A `__code_fences` entry's `offset_line` is the 0-based line of the opening fence within `content`; an indented code block is not a code fence.

`__Object` — fallback kind for objects with `[[id]]` but no explicit Kind.

`__Workspace` and `__Namespace` — created from anchor files (`readme.qmd.md`) with explicit `[[id: __Workspace]]` or `[[id: __Namespace]]` declarations.

Explicit declaration of `[[id: __Document]]`, `[[id: __TextBlock]]`, or `[[id: __Object]]` in a heading is an error (`explicit_system_type`). Only `__Workspace` and `__Namespace` allow explicit declaration.

The id of a synthesised `__Document` or `__TextBlock` is derived from the file it belongs to, so that two files in one namespace never carry the same one. The stem is the file's path relative to the directory that declared its namespace — relative to the workspace root when the file has no namespace — lowercased, with every run of other characters folded to a single `_` and the `.qmd.md` suffix dropped; a name with no ASCII letter or digit folds to `file`. A `__Document` becomes `doc_<stem>` and a `__TextBlock` keeps the ordinal that separates several blocks in one file: `text_<stem>_<n>`.

Folding is not one-to-one — `a-b`, `a_b` and `a/b` share the stem `a_b` — so uniqueness is assigned, the way a heading-anchor slugger assigns it: the files of one namespace are taken in path byte order, the first keeps the plain stem, and a file whose ids are already taken, by an earlier file or by an id an author wrote anywhere in that namespace, takes the smallest free suffix `_1`, `_2`, … for all of its ids. Renaming, moving or adding a file can change synthesised ids; nothing may be referenced by them from another file, and a `__Document` names its own blocks in `content` while each block and each top-level object names the document in `__container`.

A single-file `parse` has no workspace and no namespace, and one document cannot collide with itself, so there the ids keep the file-local counter form (`doc_<random>`, `text_<n>`). Only a workspace result, where two files meet and where the graph key `<workspace>:<namespace>:<id>` is formed, qualifies them.

```markdown
notes/alpha.qmd.md, no namespace  ->  doc_notes_alpha    text_notes_alpha_0
format/deep/commands.qmd.md       ->  doc_deep_commands  text_deep_commands_0
  (namespace `format` declared in format/readme.qmd.md)
a-b.qmd.md, a_b.qmd.md            ->  doc_a_b, doc_a_b_1  (same stem; path byte order decides)
```

Heading type determination:

| Heading | `[[id]]` | Fields | Result |
|---------|----------|--------|--------|
| `## Title` | no | no | `__TextBlock` |
| `## Title` | no | yes | Object (auto-generated ID) |
| `## Title [[id]]` | yes | no | Object |
| `## Title [[id]]` | yes | yes | Object |

## Hierarchical IDs [[hierarchical_ids: text]]

Child objects declared with dot-ID syntax receive a hierarchical `__id` formed from their parent's ID:

- **Single children** (nested objects): `parent.__id + "." + local_id`
- **Array elements** (objects in a `[Kind]` array field): `parent.__id + "." + field_name + "." + local_id`

Every child object also gets a `__local_id` field containing just the local part of the ID (the segment after the last dot).

```markdown example
## Team [[team]]

### Members [[members: [User]]]

#### Alice [[alice]]

- role: admin
```

**Result:** object `alice` gets:

- `__id`: `"team.members.alice"`
- `__local_id`: `"alice"`
- `__parent`: `"[[#team]]"`
- `__parent_field`: `"members"`

**Exception:** Children of `__Workspace` and `__Namespace` objects keep flat IDs. Objects declared directly under a workspace root or namespace do not receive hierarchical prefixes — their `__id` remains the literal value from the heading `[[id]]`.

## Comments [[comments_system: text]]

- about: [[#comment]]

All text and subheadings without `[[field_id]]` that don't become fields are collected into the `__comments` system field.

Format: array of objects, each containing:

- `after`: anchor indicating position — `__self` for the object itself, or `field_name` for a specific field
- `content`: raw markdown slice — the parser does not interpret the content, it simply extracts the fragment between structural boundaries

Boundaries are determined by structural elements: headings at the same or higher level, headings with `[[field_id]]`, field lists with valid QMD.md keys, or end of document.

Field order in the object MUST be strictly preserved (insertion order). Without order preservation, `__comments` anchors lose their meaning.

## Round Trip [[round_trip_fields: text]]

- about: [[#object]]

For lossless round-trip (QMD.md → JSON → QMD.md), the parser stores metadata:

`__types` — field data types. Created if the object has at least one non-string field. Records types for all fields (string, number, boolean, null, array, map).

`__syntax` — notation syntax for fields with multiple possible representations. Values: `yaml_array`, `yaml_multiline_array`, `markdown_list`, `headers`, `table`, `yaml_object`, `json_object`, `yaml_multiline`, `multiline_text`, `map`. Created only when fields use alternative syntax.

`__level` — heading level, always created for rebuild.

`__has_explicit_id` — only created as `false` when ID was auto-generated.

`__labels` — original heading labels for heading-syntax fields. Rebuild uses these instead of auto-generating from the key.

Rebuild must restore the type in the heading from `__syntax`:

| `__syntax` value | Heading on rebuild |
|------------------|-------------------|
| `multiline_text` | `### Label [[field: text]]` |
| `markdown_list` | `### Label [[field: array]]` |
| `yaml_object` | `### Label [[field: yaml]]` |
| `json_object` | `### Label [[field: json]]` |
| `map` | `### Label [[field: map]]` |
| `headers` | `### Label [[field: [Kind]]]` |
| `table` | `### Label [[field: [Kind]]]` |

## Rules [[rules: text]]

- System fields are output in canonical order: `__id`, `__label`, `__kind`, `__container`, `__parent`, `__parent_field`, `__comments`, user fields, `__types`, `__syntax`, `__level`, `__line`, `__has_explicit_id`, `__references`, `__positions`, `__labels`
- All parsers MUST output fields in this order for consistency
- HTML comments `<!-- ... -->` are ignored completely (not captured in `__comments`)
- Rebuild MUST output `__comments.content` as raw string without interpretation
- Rebuild MUST handle `__Workspace` and `__Namespace` as regular objects
- Rebuild always ends output with exactly one `\n` (POSIX convention)
- YAML blocks preserve original key order on rebuild (`sort_keys: false`)
