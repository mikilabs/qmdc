# QMD.md vs Quarto's QMD [[qmd_md_vs_quarto: Explanation]]

- goal: understand how QMD.md differs from Quarto's QMD and why they don't compete
- audience: newcomer
- about: [[#format]], [[#workspace]], [[#reference]]
- next: [[#quickstart]]

Two formats share three letters. They are not the same thing, and one did not grow out of the other. **Quarto's QMD** (`.qmd`) is a publishing format. **QMD.md** (`.qmd.md`) is a structured-data format. QMD.md was shaped by several influences — Quarto's QMD among them — but it points in a different direction: structured data that stays legible to both people and agents.

If you arrived expecting Quarto, you want [quarto.org](https://quarto.org). QMD.md is an independent project and is not affiliated with Quarto.

## Two different goals

Quarto's QMD exists to **produce documents**. You write Markdown with a YAML header and executable code cells, and Quarto renders it — through Pandoc — into articles, slides, websites, and PDFs. The code runs; the output is a finished document.

QMD.md exists to **carry structured data**. Headings become objects, list items become fields, and `[[#references]]` become typed edges. Nothing executes. The output is a queryable knowledge graph that a person can read as plain Markdown and a machine (or an agent) can parse, validate, and query with SQL.

## What inspired QMD.md

QMD.md is its own format, drawn from a handful of influences:

- **Quarto's QMD** — the instinct that Markdown can carry far more than prose (and, yes, where the name resonance comes from).
- **Obsidian** — `[[…]]` wiki-links as a natural, human-friendly way to connect notes; QMD.md promotes them to first-class typed references.
- **Graph databases and knowledge graphs** — model a domain as objects connected by typed edges, then query it.
- **Graph reasoning** — an agent that can *traverse* explicit relationships reasons more reliably than one guessing from prose.
- **The limits of naive RAG** — chunk-embed-retrieve loses structure and fails in subtle ways; explicit, typed structure gives deterministic, inspectable context instead of fuzzy recall.

## Quarto's QMD, in brief

A Quarto document interleaves a YAML header with **executable** code cells:

```yaml
title: "Quarterly Report"
format: html
```

Below the header, a fenced R cell (e.g. one that calls `plot(revenue)`) runs at build time, and Quarto embeds the computed plot into the page. The file is a recipe for a document.

## QMD.md, in brief

A QMD.md file describes things and how they relate:

```qmd.md example
## API Gateway [[gateway: Service]]

- port: 8080
- depends: [[#auth]]

## Auth Service [[auth: Service]]

- protocol: JWT
```

Parse it and you get objects (`gateway`, `auth`), fields (`port`, `protocol`), and a typed edge (`gateway --depends--> auth`) you can query with SQL. The file is data you can still read.

## Side by side

| | Quarto's QMD (`.qmd`) | QMD.md (`.qmd.md`) |
| --- | --- | --- |
| Goal | Reproducible documents | Structured, queryable data |
| Code fences | **Executed** (R/Python/Julia) | A content or query block (e.g. a `table` block) |
| Output | Rendered HTML / PDF / slides | JSON, a graph, SQL results |
| Primary audience | Authors and readers | People **and** agents/tools |
| Toolchain | Quarto / Pandoc | QMDC |

## Why the `.qmd.md` extension

The double extension is deliberate: `qmd` nods to the heritage, `md` keeps the file honest — it is still plain Markdown that any Markdown tool can open. It also reads clearly as *not* a bare Quarto `.qmd`.

## Can they coexist?

Yes. They are different formats with different toolchains and different file names, so they don't compete for the same files. Just don't point Quarto at a `.qmd.md` file or QMDC at a Quarto `.qmd` — each expects its own conventions.
