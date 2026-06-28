# Tutorials [[tutorials: __Namespace]]

## Content Generator [[tutorials_gen: ContentGenerator]]

- target: [[#tutorials.content]]
- about: [[#quickstart]], [[#guide_first_file]]
- audience: newcomer
- sources_hash: ce6359ed2f36aac1

### Prompt [[tutorials_gen_prompt: text]]

Generate the landing/overview page for the **Tutorials** section of the QMDC docs. It must orient a reader who just arrived and route them correctly.

Query each `about:` object for its `goal`, `time`, and `audience` fields (via `qmdc query`) and build the descriptions from those — do not invent them.

Structure the content as:

1. A short intro (2–3 sentences): tutorials are hands-on, learn-by-doing lessons in the Diátaxis sense — the reader types real files and runs real commands. They serve *learning*, not getting work done.
2. A brief "where to go" paragraph that routes the reader: stay here if you're new; head to the how-to [[#guides]] when you have a specific task in mind; read the [[#learn]] explanations for the *why*. Link those three namespaces with QMD references.
3. A section "## The path": a numbered list of the tutorials in learning order — [[#quickstart]] first, then [[#guide_first_file]]. Link each with its QMD reference and give a one-line description from its `goal` and `time`.
4. A final section "## At a glance" containing a single live table block: a fenced code block whose language is `table`, with first line `scope: all` and a `sql:` block running `SELECT __label as Tutorial, json_extract(data, '$.goal') as Goal, json_extract(data, '$.time') as Time FROM objects WHERE __namespace = 'tutorials' AND __kind = 'Tutorial' ORDER BY __label`.

Audience: newcomer — plain language, no jargon. Keep it concise and scannable.

## Content [[content: text]]

Tutorials are hands-on, learn-by-doing lessons. You'll type real files, run real commands, and end up with something working. They exist to teach you QMDC's core ideas through practice — not to solve a specific task you already have in mind.

If you're new, start here and work through the list in order. When you have a concrete task to accomplish, head over to the how-to [[#guides]]. If you want to understand *why* things work the way they do, read the [[#learn]] explanations.

## The path

1. **[[#quickstart]]** — Go from zero to a working QMDC graph in five minutes. *(5m)*
2. **[[#guide_first_file]]** — Build a QMD.md file from scratch and understand why each piece works. *(15m)*

## At a glance

```table
scope: all
sql: SELECT __label as Tutorial, json_extract(data, '$.goal') as Goal, json_extract(data, '$.time') as Time FROM objects WHERE __namespace = 'tutorials' AND __kind = 'Tutorial' ORDER BY __label
```
