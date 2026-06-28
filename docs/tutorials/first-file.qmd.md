# Write Your First QMD.md File [[guide_first_file: Tutorial]]

- goal: build a QMD.md file from scratch and understand why each piece works
- audience: newcomer
- time: 15m
- outcome: a file using objects, fields, types, arrays, nesting, and references
- about: [[#object]], [[#field]], [[#heading]], [[#data_type]], [[#reference]], [[#array]]
- prerequisites: [[#quickstart]]
- next: [[#guide_validate]]

## Content Generator [[guide_first_file_gen: ContentGenerator]]

- target: [[#guide_first_file.content]]
- about: [[#object]], [[#field]], [[#heading]], [[#data_type]], [[#reference]], [[#array]]
- sources_hash: 7f4631445580e540

### Prompt [[guide_first_file_gen_prompt: text]]

Write a detailed tutorial: "Write your first QMD.md file — with explanations."

This is the TUTORIAL version of the quickstart. The quickstart gives you commands to copy-paste. This guide explains WHY each piece works, covers edge cases, and builds a richer example.

Structure:

1. **Create the file** — `team.qmd.md`. Explain the `.qmd.md` extension (it's still valid Markdown, tools render it normally).
2. **Your first object** — `## Alice [[alice]]`. Explain: heading = object, `[[id]]` = unique identifier. Show what happens if you omit the ID (auto-generated from title). Show the `[[id: Kind]]` variant.
3. **Add fields** — `- role: developer`, `- active: true`, `- score: 95.5`. Explain data types: strings, numbers, booleans (`true`/`false` lowercase only!), `null`. Show common mistakes: `True` (wrong), `yes` (wrong).
4. **Arrays** — `- tags: [python, rust]`. Show YAML compact form and expanded heading-syntax form. Explain when to use which.
5. **Nested objects** — add `### Address [[address]]` under Alice. Explain: child heading = child object, parent gets a reference field automatically.
6. **References** — add Bob with `- mentor: [[#alice]]`. Explain: `#` means "link to", this creates an edge in the graph. Show what happens with a broken reference.
7. **Parse and inspect** — `qmdc parse -i team.qmd.md`. Walk through the JSON output field by field. Explain `__id`, `__kind`, `__parent`.
8. **Validate** — show a deliberate error (typo in reference), run parse, show the error message, fix it.

Each step: show the full file so far, explain what's new, show the parsed output. Build understanding, not just muscle memory.

Tone: patient, thorough. Assume the reader wants to understand, not just get it working.
End with: "You now know objects, fields, types, arrays, nesting, and references. Next: validate your file or build a multi-file workspace."

## Content [[content: text]]

In this tutorial you'll build a QMD.md file from scratch, step by step. Unlike the [[#quickstart]] (which gives you commands to copy-paste), this guide explains **why** each piece works and what happens under the hood.

By the end you'll understand [[#object]]s, [[#field]]s, [[#data_type]]s, [[#array]]s, nesting, and [[#reference]]s — the full foundation of QMDC.

---

## 1. Create the file

Create a file called `team.qmd.md`:

```bash
touch team.qmd.md
```

**Why `.qmd.md`?** The double extension means the file is valid Markdown *and* a QMD.md document. Any Markdown renderer (GitHub, VS Code, etc.) displays it normally. QMDC tools parse the structured data inside it. You get human-readable docs and machine-readable data in one file.

---

## 2. Your first object

Open `team.qmd.md` and add:

```qmd
## Alice [[alice]]
```

This single line creates an **object**. Here's what each part does:

- `##` — a Markdown heading. In QMDC, every [[#heading]] creates an object. The heading level determines nesting depth.
- `Alice` — the **title**. Stored in the system field `__label`.
- `[[alice]]` — the **identifier**. The unique ID for this object in the workspace graph. You'll use it to reference Alice from other objects.

**What if you omit the ID?**

```qmd
## Alice
```

Without `[[...]]`, QMDC auto-generates an ID from the title. The algorithm: lowercase, replace non-alphanumeric characters with `_`, strip leading/trailing underscores. If the result starts with a digit, prepend `_`.

| Title | Auto-generated ID |
|-------|-------------------|
| `Alice` | `alice` |
| `John Doe` | `john_doe` |
| `My Team #1` | `my_team_1` |
| `2024 Report` | `_2024_report` |

Auto-generated IDs work, but explicit IDs are clearer and won't break if you rename the heading.

**Adding a Kind (type):**

```qmd
## Alice [[alice: User]]
```

The `[[id: Kind]]` syntax assigns a type. `User` is a user-defined Kind (PascalCase). Kinds enable schema validation and help distinguish objects that might share similar IDs.

For now, keep it simple — no Kind yet:

```qmd
## Alice [[alice]]
```

---

## 3. Add fields

[[#field]]s are key-value pairs defined with Markdown bullet lists. Add some data to Alice:

```qmd
## Alice [[alice]]

- role: developer
- active: true
- score: 95.5
```

Each `- key: value` line becomes a field on the object. QMDC auto-detects the [[#data_type]] of each value:

| Value | Detected type | Rule |
|-------|--------------|------|
| `developer` | string | Default — anything that isn't a number, boolean, or null |
| `true` | boolean | Exactly `true` or `false` (lowercase only!) |
| `95.5` | number | Integer, float, or scientific notation |
| `null` | null | The keyword `null`, or an empty value after the colon |

**Common mistakes:**

| What you write | What QMDC sees | Why |
|---------------|--------------|-----|
| `True` | string `"True"` | Only lowercase `true`/`false` are booleans |
| `FALSE` | string `"FALSE"` | Same reason |
| `yes` | string `"yes"` | QMDC does not treat `yes`/`no` as booleans |
| `- field:` | null | Empty value after colon = null |
| `- field: ""` | empty string | Quotes force string type |
| `- field: "123"` | string `"123"` | Quotes prevent number detection |

**Field key rules:** Keys must match `[a-zA-Z_][a-zA-Z0-9_]*` — start with a letter or underscore, then letters, digits, or underscores. No spaces, no hyphens, no special characters. Lines with invalid keys are treated as plain Markdown text, not fields.

---

## 4. Arrays

Add a tags field to Alice:

```qmd
## Alice [[alice]]

- role: developer
- active: true
- score: 95.5
- tags: [python, rust]
```

The `[python, rust]` syntax is **YAML inline notation** — a compact way to define [[#array]]s. Values are comma-separated inside brackets.

**When values contain spaces or commas**, wrap them in quotes:

```qmd
- tags: [python, "machine learning", "rust, embedded"]
```

**Expanded form with a subheading** — use this when you have many items or long values:

```qmd
## Alice [[alice]]

- role: developer
- active: true
- score: 95.5

### Tags [[tags]]

- python
- rust
- machine learning
- distributed systems
```

A subheading with `[[field_id]]` followed by a bullet list without colons creates a primitive array. Each `- value` line is one element. The `__syntax` field records `markdown_list` so the format is preserved on round-trip.

**When to use which:**

- **Inline `[a, b, c]`** — short lists, simple values, fits on one line
- **Subheading form** — long lists, multiline values, or when readability matters

There's also a **YAML multiline** variant for inline arrays that are too long for one line:

```qmd
- files_changed: [
    qmdc-rs/src/parser.rs,
    qmdc-py/qmdc/parser.py,
    qmdc-ts/src/parser.ts
  ]
```

Your file so far:

```qmd
## Alice [[alice]]

- role: developer
- active: true
- score: 95.5
- tags: [python, rust]
```

---

## 5. Nested objects

Add an address for Alice by creating a child [[#heading]] one level deeper:

```qmd
## Alice [[alice]]

- role: developer
- active: true
- score: 95.5
- tags: [python, rust]

### Address [[address]]

- city: Berlin
- country: Germany
```

**How nesting works:** `### Address` is one heading level below `## Alice`, so it becomes a **child object** of Alice. The parser automatically:

1. Creates a separate object with `__id: "alice.address"` (hierarchical — parent ID + dot + child ID)
2. Sets `__local_id: "address"` on the child
3. Sets `__parent: "[[#alice]]"` on the child
4. Adds a reference field `address: "[[#alice.address]]"` on Alice

The child is a full object in the graph — it just happens to live under Alice. You can reference it from anywhere using `[[#alice.address]]`.

---

## 6. References

Now add a second person who [[#reference]]s Alice as their mentor:

```qmd
## Alice [[alice]]

- role: developer
- active: true
- score: 95.5
- tags: [python, rust]

### Address [[address]]

- city: Berlin
- country: Germany

## Bob [[bob]]

- role: junior developer
- mentor: [[#alice]]
```

The `[[#alice]]` syntax creates a **reference** — a typed edge from Bob to Alice in the graph. The `#` prefix distinguishes references (in values) from definitions (in headings).

**What this creates:**

- An edge in the graph: `bob → alice` with edge type `mentor`
- The field value is stored as the string `"[[#alice]]"` during parsing
- During validation, QMDC checks that the target object exists

**What happens with a broken reference?**

```qmd
- mentor: [[#alce]]
```

If `alce` doesn't exist, QMDC produces a warning — but the object still loads and the graph keeps working. References never break the entire graph. They degrade gracefully: the reference remains as a string, the edge just isn't created.

**Reference formats:**

| Syntax | When to use |
|--------|-------------|
| `[[#alice]]` | Short form — most common |
| `[[#User:alice]]` | With Kind — needed when two objects share an ID but have different types |
| `[[#alice.address]]` | Hierarchical dot-path — target a nested child object |
| `[[#namespace:Kind:id]]` | Cross-namespace — reference objects in other namespaces |

---

## 7. Parse and inspect

Save your file and parse it:

```bash
qmdc parse -i team.qmd.md
```

The output is a JSON array of objects. Here's what you'll see (simplified):

```json
[
  {
    "__id": "alice",
    "__label": "Alice",
    "__kind": "__Object",
    "__level": 2,
    "role": "developer",
    "active": true,
    "score": 95.5,
    "tags": ["python", "rust"],
    "address": "[[#alice.address]]",
    "__types": { "active": "boolean", "score": "number", "tags": "array" },
    "__syntax": { "tags": "yaml_array" }
  },
  {
    "__id": "alice.address",
    "__label": "Address",
    "__kind": "__Object",
    "__local_id": "address",
    "__parent": "[[#alice]]",
    "__level": 3,
    "city": "Berlin",
    "country": "Germany"
  },
  {
    "__id": "bob",
    "__label": "Bob",
    "__kind": "__Object",
    "__level": 2,
    "role": "junior developer",
    "mentor": "[[#alice]]"
  }
]
```

**Key system fields explained:**

| Field | Meaning |
|-------|---------|
| `__id` | The unique identifier you defined in `[[...]]` |
| `__label` | The heading title text |
| `__kind` | Object type — `__Object` is the default when you don't specify a Kind |
| `__level` | Heading level (2 = `##`), used for lossless round-trip rebuild |
| `__parent` | Reference to the parent object (nested objects only) |
| `__local_id` | Local part of a hierarchical ID (e.g., `address` from `alice.address`) |
| `__types` | Records which fields have non-string types |
| `__syntax` | Records notation syntax for round-trip fidelity |

Notice how everything is **flat**: the output is a single array of objects. Nesting is expressed via `__parent` references and hierarchical IDs, not through JSON nesting.

---

## 8. Validate — find and fix errors

Let's introduce a deliberate mistake. Change Bob's mentor to a typo:

```qmd
## Bob [[bob]]

- role: junior developer
- mentor: [[#alce]]
```

The parser still produces output — parsing never crashes on bad references. But when you validate the workspace:

```bash
qmdc workspace validate .
```

You'll see a warning like:

```text
unresolved reference [[#alce]] in bob.mentor — target not found
```

**Fix it** by correcting the reference back to `[[#alice]]`, then validate again — you should get `[]` (empty array = no errors).

The philosophy: reference problems produce warnings, not fatal errors. Your graph keeps loading, and you fix issues incrementally.

---

## Final file

Here's the complete `team.qmd.md`:

```qmd
## Alice [[alice]]

- role: developer
- active: true
- score: 95.5
- tags: [python, rust]

### Address [[address]]

- city: Berlin
- country: Germany

## Bob [[bob]]

- role: junior developer
- mentor: [[#alice]]
```

---

## What you learned

You now know the six core concepts of QMD.md:

- **Objects** — headings create objects, `[[id]]` assigns an identifier
- **Fields** — `- key: value` bullet lists store data on objects
- **Data types** — strings, numbers, booleans (`true`/`false` lowercase only), and `null`
- **Arrays** — inline `[a, b, c]` or expanded subheading form
- **Nesting** — child headings create child objects with automatic parent links
- **References** — `[[#id]]` links objects in the graph, creating typed edges

Next: [[#guide_validate]] your file to catch errors, or jump to the [Format Specification](../format/index.md) for the complete syntax reference.
