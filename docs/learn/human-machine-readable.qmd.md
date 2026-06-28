# Human Text, Machine Structure [[human_machine_readable: Explanation]]

- goal: understand QMDC's dual nature — readable docs and queryable data at once
- audience: newcomer
- about: [[#format]], [[#workspace]], [[#object]], [[#field]]
- next: [[#qmdc_for_agents]]

## Content Generator [[human_machine_gen: ContentGenerator]]

- target: [[#human_machine_readable.content]]
- about: [[#format]], [[#workspace]], [[#object]], [[#field]]
- sources_hash: 3d2b29e6c8dc87b1

### Prompt [[human_machine_gen_prompt: text]]

Explain the dual nature of QMDC: it's simultaneously human-readable documentation AND machine-queryable structured data.

Cover:

- A QMD.md file looks like normal Markdown — you can read it in any text editor, on GitHub, in a wiki
- But a parser can extract objects, fields, references, and build a graph from it
- This means: one source of truth, two audiences. Humans read prose, machines query structure.
- Show a concrete example: the same file rendered as a web page (human view) vs queried as data (machine view)
- Explain the tradeoff: QMDC adds minimal syntax (`[[id]]`, `[[#ref]]`) — just enough for structure, not enough to hurt readability

Tone: enthusiastic but grounded. This is the core value proposition of QMDC.

## Content [[content: text]]

A QMD.md file is plain Markdown. Open it in any text editor, render it on GitHub, drop it into a wiki — it reads like normal documentation. But when a parser processes that same file, it extracts **[[#object]]s**, **[[#field]]s**, and **references** to build a queryable knowledge graph.

**One source of truth. Two audiences.** Humans read prose. Machines query structure.

## The human view

Here's a QMD.md file as any reader would see it:

```qmd
## Auth Service [[auth: Service]]

- protocol: JWT
- port: 9000
- database: [[#postgres]]

## PostgreSQL [[postgres: Resource]]

- version: 16
```

Headings, bullet lists, a few bracketed annotations. It renders cleanly everywhere Markdown renders — GitHub, VS Code, Notion, a printed PDF. Nothing here breaks readability.

## The machine view

The same file, when parsed, yields structured data:

```sql
SELECT __id, __kind, data FROM objects WHERE __kind = 'Service'
-- → auth | Service | {"protocol":"JWT","port":9000,"database":"[[#postgres]]"}

SELECT source_id, target_id, edge_type FROM edges WHERE source_id LIKE '%auth%'
-- → auth → postgres | database
```

The parser sees:

- **[[#object]]s** from headings — `[[auth: Service]]` declares an object with ID `auth` and Kind `Service`
- **[[#field]]s** from bullet lists — `- port: 9000` becomes a typed key-value pair on that object
- **References** from `[[#id]]` links — `[[#postgres]]` creates a typed edge in the graph
- **Hierarchy** from heading levels — child headings become nested objects with dot-path IDs like `team.members.alice`

All of this feeds into a [[#workspace]]-wide graph: every `.qmd.md` file in the directory is indexed, cross-file references are resolved, and the whole thing is queryable with SQL.

## Minimal syntax, maximum compatibility

QMDC adds exactly two constructs to standard Markdown:

1. **`[[id]]`** or **`[[id: Kind]]`** on headings — declares an object
2. **`[[#ref]]`** in field values — creates a link to another object

No frontmatter. No custom block syntax. No angle-bracket tags. The annotations sit inside the natural structure of Markdown headings and lists — just enough for machines to parse, not enough to hurt readability.

A file without any `[[...]]` annotations is still valid QMD.md — it just produces text blocks instead of structured objects. You can adopt the format incrementally, annotating only the parts you need to query.

## Why this matters

Traditional approaches force a choice: write docs (readable but unstructured) or write data (structured but unreadable). QMDC eliminates the tradeoff:

- **Documentation stays readable** — no context-switching between prose and config files
- **Data stays queryable** — the full [[#workspace]] graph supports tooling, validation, and code generation
- **One file to maintain** — no drift between a spec document and its machine-readable counterpart

For the complete syntax reference, see the [[#format]] specification — especially [[#object]], [[#field]], and [[#reference]].
