# Why QMDC [[why_qmdc: Explanation]]

- goal: understand what problem QMDC solves and who it is for
- audience: newcomer
- about: [[#format]], [[#object]], [[#reference]], [[#workspace]]
- next: [[#markdown_to_graph]]

## Content Generator [[why_qmdc_gen: ContentGenerator]]

- target: [[#why_qmdc.content]]
- about: [[#format]], [[#object]], [[#reference]], [[#workspace]]
- sources_hash: 809d8b011695aec8

### Prompt [[why_qmdc_gen_prompt: text]]

Write a compelling 1-page explanation of why QMDC exists and what problem it solves.

Key points to cover:

- The gap between unstructured docs (Markdown) and structured data (YAML/JSON) — QMDC bridges it
- The problem: documentation rots because it's disconnected from the system it describes
- The solution: Markdown that IS a graph — human-readable AND machine-queryable
- Compare briefly: plain Markdown (no structure), YAML (no readability), wikis (no graph), QMDC (both)
- Who benefits: developers writing docs, AI agents consuming docs, teams maintaining knowledge

Tone: conversational, concrete examples, no jargon. Imagine explaining to a senior developer who's skeptical about "yet another format".

Use the source objects' descriptions and principles as raw material. Do NOT copy-paste — synthesize.

## Content [[content: text]]

You've written docs in Markdown. You've defined schemas in YAML. You've built wikis. And somehow — documentation still rots. Why?

Because **documentation lives in one world and the systems it describes live in another**. Your Markdown is readable prose that no tool can query. Your YAML is perfectly structured but painful to read. Your wiki has links between pages, but no tool can ask "which services depend on the auth service?" across the whole project.

QMDC bridges this gap.

## The core problem

Documentation decays because it has no structure a machine can act on. Think about a typical project:

- **Architecture diagrams** live in Markdown files nobody updates after sprint 2
- **Service dependencies** are described in prose that contradicts the actual config
- **Data models** are documented in one place and implemented in another

When something changes, nobody updates the docs because the docs can't tell you they're wrong. There's no validation, no broken-link detection, no way to ask "show me everything that connects to this database."

## What QMDC does differently

QMD.md is plain Markdown — with [[#object]]s and [[#reference]]s baked into the syntax you already know. A `.qmd.md` file renders in any Markdown viewer. But it's also a graph you can query with SQL.

```qmd
## API Gateway [[gateway: Service]]

- port: 8080
- depends: [[#user_service]]

## User Service [[user_service: Service]]

- port: 8081
- database: [[#users_db]]
```

That's it. Headings become **objects** — named, typed data structures with an ID and a Kind. Bullet lists become **fields** (key-value pairs). `[[#references]]` become **typed edges** in a graph. The field name (`depends`, `database`) becomes the edge type automatically.

Now you can query your docs:

```sql
SELECT source_id, target_id FROM edges WHERE edge_type = 'depends'
```

## How it compares

| Approach | Human-readable | Machine-queryable | Graph structure |
|----------|:-:|:-:|:-:|
| Plain Markdown | ✓ | ✗ | ✗ |
| YAML / JSON | ✗ | ✓ | ✗ |
| Wiki (Notion, Confluence) | ✓ | ✗ | untyped links |
| **QMD.md** | **✓** | **✓** | **typed edges** |

Plain Markdown gives you prose. YAML gives you data. Wikis give you links. QMD.md gives you all three — prose that IS data, with links that carry meaning.

## Who benefits

**Developers writing docs** — You keep writing Markdown. You get a queryable graph for free. No context-switching to a schema language, no separate "source of truth" that drifts from the docs.

**AI agents** — Instead of parsing unstructured text and hoping for the best, agents query the graph directly. "Find all services that depend on auth" is a SQL query, not a prompt-engineering exercise.

**Teams maintaining knowledge** — A [[#workspace]] validates every [[#reference]] automatically. Rename an object, and broken links surface immediately. Delete a service definition, and everything that referenced it lights up with warnings. Documentation can't silently drift.

## The building blocks

Three concepts make it work:

1. **Objects** — A Markdown heading with `[[id: Kind]]` becomes a named, typed node in the graph. It has fields (from bullet lists) and a unique identity.

2. **References** — Writing `[[#target_id]]` in a field value creates a directed edge. The field name gives the edge its type — `depends`, `database`, `author` — whatever makes sense for your domain.

3. **Workspaces** — A folder of `.qmd.md` files that cross-reference each other. The toolchain indexes every object, validates every link, and lets you query across all files with SQL.

No build step. No compilation. No special editor required. Just Markdown that happens to be a graph.

---

To see exactly how headings and lists become graph nodes and edges, continue to [[#markdown_to_graph]]. For the complete syntax, see the [[#format]] reference.
