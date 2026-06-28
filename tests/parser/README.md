# QMD Micro-tests

A set of atomic tests for developing the QMD parser. Each test checks one specific feature.

## Structure

Each test consists of two files:

- `NNN-name.qmd.md` — input QMD document
- `NNN-name.expected.json` — expected parsing result

## Test categories

### 001-010: Basic objects and metadata

- `001` — Empty object (heading only)
- `002` — Object with explicit ID at the end of the heading
- `003` — ID at the start of the heading
- `004` — ID in the middle of the heading
- `005` — Two objects in one file
- `006` — Object with Kind in the heading `[[id: Kind]]`
- `007` — Kind only, no ID `[[:Kind]]`
- `008` — H3 heading as a nested object
- `009` — Two objects with explicit IDs
- `010` — Label with a dash (ID autogeneration)

### 011-020: Simple fields (primitive types)

- `011` — Single string field
- `012` — Number field
- `013` — Boolean fields (true/false)
- `014` — Null value
- `015` — Multiple fields
- `016` — String with spaces
- `017` — Float number
- `018` — Negative number
- `019` — Quoted string
- `020` — Zero (0)

### 021-030: Multiline fields and arrays

- `021` — Multiline text field
- `022` — YAML array `[a, b, c]`
- `023` — Markdown list as an array
- `024` — Empty array `[]`
- `025` — Array of numbers
- `026` — Nested object (H3)
- `027` — Mixed array (different types)
- `028` — Object with a field and a nested object
- `029` — Two nested objects
- `030` — Multiline text with a blank line

### 031-045: Arrays of objects, tables, comments, references

- `031` — Array of objects via H4 headings
- `032` — Markdown table
- `033` — Comment before a field
- `034` — Comment after a field
- `035` — HTML comment (ignored)
- `036` — Reference as a string `[[#id]]`
- `037` — Array of references
- `038` — YAML block `[[field: yaml]]`
- `039` — Reference with Kind `[[#Kind:id]]`
- `040` — Multiple comments
- `041` — Single-item list
- `042` — Single-row table
- `043` — Array with one object
- `044` — Bold metadata (**kind**, **label**)
- `045` — Colon in a field value

### 046-050: Edge cases and metadata

- `046` — Explicit `__kind` via bold
- `047` — Explicit `__workspace` and `__namespace`
- `048` — Empty string as a value
- `049` — Special characters (email, URL)
- `050` — Deep nesting (4 levels, H2-H5)

### 051-058: Special cases and relationship types

- `051` — ID collision with special characters (only `!!!`)
- `052` — Kind collision with empty label `[[:Kind]]`
- `053` — Empty brackets `[[]]`
- `054` — All heading levels H1-H7 (deep nesting)
- `055` — Two top-level objects in one file
- `056` — Optional relationship (0-1-1) with a null reference
- `057` — Self-reference in a hierarchy
- `058` — Several field lists (merge of `__types`/`__syntax`)

## Using these for development

### Building the parser step by step

**Stage 1: Basic objects (001-005)**

- Recognize H2 headings as objects
- Extract `[[id]]` from headings
- Autogenerate IDs from the title
- Generate `__id` and `__label`

**Stage 2: Simple fields (011-018)**

- Parse `- key: value` lists
- Detect types (string, number, boolean, null)
- Multiple fields in one object

**Stage 3: Multiline fields (021)**

- Recognize subheadings as fields
- Multiline text

**Stage 4: Arrays of primitives (022-025)**

- YAML notation `[a, b, c]`
- Markdown lists `- value`
- Detect element types
- `__syntax` for arrays

**Stage 5: Nested objects (026, 050)**

- Subheadings as object fields
- Deep nesting

**Stage 6: Arrays of objects (031)**

- `[[field: [Kind]]]` syntax
- Autogenerate `__parent` and `__parent_field`
- Two-way relationships

**Stage 7: Tables (032)**

- Parse Markdown tables
- Autogenerate IDs for rows
- `__syntax: table`

**Stage 8: Comments (033-035)**

- Text between fields → `__comments`
- Structure `{after: "...", content: "..."}`
- Ignore HTML comments

**Stage 9: References (036-037)**

- Recognize `[[#id]]` as strings
- Arrays of references
- No resolution (resolve is a separate stage)

**Stage 10: YAML blocks (038)**

- `[[field: yaml]]` syntax
- Parse YAML into an object/array
- Error handling → `__parse_error`

**Stage 11: Metadata (046-047)**

- `**kind**: Value` → `__kind`
- `**workspace**: Value` → `__workspace`
- `**namespace**: Value` → `__namespace`

**Stage 12: Edge cases (048-049)**

- Empty strings
- Special characters
- Boundary cases

## Running the tests

### Run from the repository root

```bash
make test   # all parsers + lint + format
```

Do not run the tests inside `qmdc-py/` or `qmdc-ts/` on their own.

## Tips

1. **Start with the simple tests** (001-005)
2. **Work through the tests in order** — each builds on the previous ones
3. **Don't skip failing tests** — fix them right away
4. **Add your own tests** when you find bugs
5. **Check on every parser** (Python, TypeScript, Rust)

## Extending the suite

When adding new tests:

1. Create the files:
   - `0XX-name.qmd.md` — input QMD
   - `0XX-name.expected.json` — expected JSON

2. The runners discover fixtures automatically from this directory.

3. Update this README (add a description of the test).

4. Run `make test` and confirm the new test fails first (as expected).

5. Implement the feature and run `make test` again.

**Important:**

- A test should check **one feature**
- Keep it minimal for maximum clarity
