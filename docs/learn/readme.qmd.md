# Learn [[learn: __Namespace]]

## Content Generator [[learn_gen: ContentGenerator]]

- target: [[#learn.content]]
- about: [[#why_qmdc]], [[#markdown_to_graph]], [[#human_machine_readable]], [[#qmdc_for_agents]], [[#qmd_md_vs_quarto]]
- audience: newcomer
- sources_hash: 5c178d1ee949387a

### Prompt [[learn_gen_prompt: text]]

Generate the landing/overview page for the **Learn** (explanation) section of the QMDC docs. It must orient a reader and give them a sensible reading order.

Query each `about:` object for its `goal` and `audience` fields (via `qmdc query`) and build the descriptions from those — do not invent them.

Structure the content as:

1. A short intro (2–3 sentences): explanations give background and reasoning (Diátaxis sense) — what QMDC is, why it exists, how the pieces fit. They serve *understanding*, and can be read away from the keyboard.
2. A brief "where to go" paragraph that routes the reader: need to do something specific? see the [[#guides]]; just getting started? work through the [[#tutorials]] first. Link those namespaces with QMD references.
3. A section "## Reading path": a numbered list in this order — [[#why_qmdc]], [[#markdown_to_graph]], [[#human_machine_readable]], [[#qmdc_for_agents]], [[#qmd_md_vs_quarto]] — each linked with its QMD reference and a one-line description from its `goal`.
4. A final section "## At a glance" containing a single live table block: a fenced code block whose language is `table`, with first line `scope: all` and a `sql:` block running `SELECT __label as Topic, json_extract(data, '$.goal') as Goal FROM objects WHERE __namespace = 'learn' AND __kind = 'Explanation' ORDER BY __label`.

Audience: newcomer — plain language, no jargon. Keep it concise and scannable.

## Content [[content: text]]

The **Learn** section explains the ideas behind QMDC — what it is, why it exists, and how the pieces fit together. These pages are for *understanding*; read them away from the keyboard whenever you want the bigger picture.

If you need to accomplish something specific right now, jump to the [[#guides]]. If you're brand new, work through the [[#tutorials]] first and come back here when you want the "why" behind the steps.

## Reading path

1. **[[#why_qmdc]]** — understand what problem QMDC solves and who it is for
2. **[[#markdown_to_graph]]** — see how plain Markdown headings and lists become a queryable graph
3. **[[#human_machine_readable]]** — understand QMDC's dual nature — readable docs and queryable data at once
4. **[[#qmdc_for_agents]]** — understand how AI agents benefit from a queryable QMDC workspace
5. **[[#qmd_md_vs_quarto]]** — understand how QMD.md differs from Quarto's QMD and why they don't compete

## At a glance

```table
scope: all
sql: SELECT __label as Topic, json_extract(data, '$.goal') as Goal FROM objects WHERE __namespace = 'learn' AND __kind = 'Explanation' ORDER BY __label
```
