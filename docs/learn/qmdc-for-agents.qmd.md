# QMDC for Agents [[qmdc_for_agents: Explanation]]

- goal: understand how AI agents benefit from a queryable QMDC workspace
- audience: developer
- about: [[#workspace]], [[#reference]], [[#object]], [[#format]]
- next: [[#qmd_md_vs_quarto]]

## Content Generator [[qmdc_for_agents_gen: ContentGenerator]]

- target: [[#qmdc_for_agents.content]]
- about: [[#workspace]], [[#reference]], [[#object]], [[#format]]
- sources_hash: 809d8b011695aec8

### Prompt [[qmdc_for_agents_gen_prompt: text]]

Explain how AI agents (LLMs, coding assistants) benefit from QMDC workspaces.

Cover:

- The problem: agents need context about a codebase, but READMEs are unstructured and incomplete
- QMDC workspaces give agents: queryable architecture docs, typed relationships between components, findable specifications
- Show how an agent can: "find all modules that depend on X" (SQL query), "get the API contract for service Y" (direct object lookup), "understand the data flow for scenario Z" (follow edges)
- Mention: the same docs humans read are the docs agents query — no separate "agent context" files needed
- Briefly mention: semantic search (embeddings), graph navigation, reference resolution

Tone: practical, aimed at developers who use AI coding assistants and want to give them better context.

## Content [[content: text]]

## The Problem: Agents Need Structured Context

AI coding assistants are only as good as the context they receive. Most projects give agents a README, scattered doc comments, and maybe an architecture diagram as a PNG. The agent has to guess at module boundaries, infer relationships, and hope the docs are current.

This breaks down fast. When an agent needs to answer "what depends on the auth service?" or "what's the contract for the payments endpoint?", it's stuck grepping through source code or hallucinating an answer from stale prose.

Unstructured docs can't be queried. They can only be read — and reading doesn't scale.

## What a QMDC Workspace Gives Agents

A QMDC [[#workspace]] is a directory of `.qmd.md` files that form a queryable knowledge graph. Every component — service, table, endpoint, config — is a typed [[#object]] with named fields and explicit [[#reference]]s to other objects. The graph is exposed via SQL.

What agents gain:

- **Typed objects** — every component has a Kind (`Service`, `Table`, `Route`) and structured fields, not just paragraphs
- **Typed edges** — relationships carry semantics: `depends`, `database`, `owner`. Field names become edge types automatically
- **SQL queries** — precise lookups instead of regex over prose
- **Cross-file resolution** — `[[#auth_service]]` resolves across the entire workspace, across namespaces and files
- **Hierarchical IDs** — dot-paths like `[[#team.members.alice]]` address nested objects unambiguously

## How an Agent Uses QMDC

**Find all services that depend on auth:**

```sql
SELECT s.__id, s.__kind FROM edges e
JOIN objects s ON e.source_id = s.__global_id
JOIN objects t ON e.target_id = t.__global_id
WHERE t.__id = 'auth_service' AND e.edge_type = 'depends'
```

**Get the full definition of a service:**

```bash
qmdc query . "SELECT data FROM objects WHERE __id = 'user_service'" --format json
```

The `data` column returns all fields as JSON — port, routes, dependencies, database refs — structured and machine-readable.

**Trace data flow from an endpoint:**

```bash
qmdc query . "SELECT target_id, edge_type FROM edges WHERE source_id = 'route_users'"
```

Follow each target to build the path: route → service → database → table. Every hop is a typed edge in the graph.

**List all objects in a namespace:**

```bash
qmdc query . "SELECT __id, __kind, __label FROM objects WHERE __file LIKE 'storage/%'"
```

## One Set of Docs, Two Audiences

QMD.md files are valid Markdown. Humans read them as documentation — headings, lists, prose. Agents query them as a graph — objects, fields, edges. There is no separate "agent context" layer to maintain.

When a developer updates a service definition, both the human-readable docs and the agent-queryable graph update in the same commit. The [[#format]] guarantees round-trip fidelity: parse → modify → rebuild produces the same Markdown.

## Beyond SQL: Other Access Patterns

- **Graph navigation** — follow edges iteratively to explore neighborhoods. Start from one object, traverse `depends` edges outward, build a dependency tree
- **Semantic search** — generate embeddings over object descriptions and fields. "Find components related to payment processing" works without knowing exact IDs
- **Reference resolution** — given `[[#team.members.alice.role]]`, resolve through the hierarchy to the exact field on the exact child object, across files
- **Kind-based filtering** — query all objects of a Kind (`SELECT * FROM objects WHERE __kind = 'Service'`) to get a system-wide inventory

## Practical Setup

Point your agent at a QMDC workspace and give it access to `qmdc query`. That's the entire integration. The agent can now:

1. Discover what exists: `SELECT __id, __kind FROM objects`
2. Understand structure: `SELECT target_id, edge_type FROM edges WHERE source_id = '...'`
3. Get details: `SELECT data FROM objects WHERE __id = '...'`

No custom tooling, no separate context files, no format conversion. The docs humans maintain are the graph agents query.

For the complete object model and query capabilities, see the [[#format]] reference.
