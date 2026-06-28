# Guides [[guides: __Namespace]]

## Content Generator [[guides_gen: ContentGenerator]]

- target: [[#guides.content]]
- about: [[#guide_validate]], [[#guide_query]], [[#guide_workspace]], [[#guide_vscode]], [[#qmdc_guide]]
- audience: developer
- sources_hash: 9fdc59e70b8f0f04

### Prompt [[guides_gen_prompt: text]]

Generate the landing/overview page for the **Guides** (how-to) section of the QMDC docs. It must orient a reader who already knows the basics and route them to the right task.

Query each `about:` object for its `goal` and `audience` fields (via `qmdc query`) and build the descriptions from those — do not invent them.

Structure the content as:

1. A short intro (2–3 sentences): how-to guides are practical recipes (Diátaxis sense) that walk a competent reader through a single real-world task. They serve *work*, not study.
2. A brief "where to go" paragraph that routes the reader: brand new? start with the [[#tutorials]]; want the *why*? read the [[#learn]] explanations. Link those namespaces with QMD references.
3. A section "## Pick your task": a bulleted list of the four how-to guides — [[#guide_validate]], [[#guide_query]], [[#guide_workspace]], [[#guide_vscode]] — each linked with its QMD reference and a one-line description from its `goal`.
4. A short note pointing to [[#qmdc_guide]] as the full reference an AI agent needs to author QMD.md correctly (it is reference material, not a task recipe).
5. A final section "## At a glance" containing a single live table block: a fenced code block whose language is `table`, with first line `scope: all` and a `sql:` block running `SELECT __label as Guide, json_extract(data, '$.goal') as Goal, json_extract(data, '$.audience') as Audience FROM objects WHERE __namespace = 'guides' AND __kind = 'HowTo' ORDER BY __label`.

Audience: developer — technical language is fine. Keep it concise and scannable.

## Content [[content: text]]

How-to guides are practical recipes. Each one walks you through a single real-world task from start to finish — they serve *work*, not study. You already know the basics; these pages get you unstuck.

If you're brand new, start with the [[#tutorials]] — they teach concepts step by step. If you want the *why* behind the design, read the [[#learn]] explanations.

## Pick your task

- **[[#guide_validate]]** — find and fix errors in your `.qmd.md` files
- **[[#guide_query]]** — query your workspace graph with SQL
- **[[#guide_workspace]]** — build a multi-file workspace with namespaces and cross-file references
- **[[#guide_vscode]]** — set up QMDC in VS Code for navigation, validation, and preview

For AI agents authoring QMD.md, [[#qmdc_guide]] is the full reference — it covers syntax rules, common errors, and the metamodel in a format optimized for LLM consumption.

## At a glance

```table
scope: all
sql: SELECT __label as Guide, json_extract(data, '$.goal') as Goal, json_extract(data, '$.audience') as Audience FROM objects WHERE __namespace = 'guides' AND __kind = 'HowTo' ORDER BY __label
```
