# Parser Microtests [[suite_parser_microtests: TestSuite]]

- location: tests/parser/
- format: NNN-name.qmd.md + NNN-name.expected.json
- test_count: 177
- implementations: [test_parser.py, test-parser.ts, parser.rs]
- is_data_driven: true
- tests: [[#format]]

## Description [[description: text]]

Atomic tests for verifying parsing of individual QMD.md syntax constructs. Each test consists of an input `.qmd.md` file and an `.expected.json` file with the expected parse result.

Use parser microtests when:

- Adding a new syntactic construct to QMDC
- Fixing a parsing bug for a specific construct
- Verifying edge cases (empty values, special characters, etc.)
- Testing a single isolated feature

Do not use for multi-file scenarios (use workspace tests), LSP features (use LSP microtests), or SQL queries (use SQL tests).

## How to Add a New Test

### Step 1: Create the input file

Create `tests/parser/0XX-name.qmd.md`:

```markdown
## User [[user_1]]

- name: Alice
- age: 30
```

### Step 2: Create the expected result

Create `tests/parser/0XX-name.expected.json`:

```json
[
  {
    "__id": "user_1",
    "__kind": "",
    "__label": "User",
    "__level": 2,
    "name": "Alice",
    "age": 30
  }
]
```

### Step 3: Run tests

```bash
# All parsers
make test

# Specific test by number
MICROTEST_FILTER=088 cargo test --test parser -- --nocapture  # Rust
uv run pytest tests/test_parser.py -k "088" -v               # Python
MICROTEST_FILTER=088 npx tsx tests/test-parser.ts             # TypeScript
```

All tests are discovered automatically via `glob("*.qmd.md")`.

## Output Formats

Parser microtests support three output formats:

- **standard** (default): `NNN-name.expected.json` — all fields except `__types`, `__syntax`, `__has_explicit_id`
- **minimal**: `NNN-name.expected.minimal.json` — only user fields, no system fields
- **full**: `NNN-name.expected.full.json` — all fields including `__types`, `__syntax`, `__has_explicit_id`

## Test Categories

### 001–010: Basic Objects

Objects with explicit IDs, auto-generated IDs, Kind, nesting, multiple objects per file.

### 011–020: Primitive Types

String, number, float, negative number, boolean, null, zero, quoted strings.

### 021–030: Multiline Fields and Arrays

Multiline text, YAML arrays, Markdown lists as arrays, empty arrays, nested objects, mixed arrays.

### 031–045: Object Arrays, Tables, Comments

Object arrays via H4, Markdown tables, comments before/after fields, HTML comments, references, YAML blocks, bold metadata.

### 046–070: Edge Cases

Explicit `__kind` via bold, `__workspace`/`__namespace`, empty string values, special characters, deep nesting.

### 071–073, 085: Code Fences

`__code_fences` parsing — fenced code blocks (` ```lang `) and verifying that references inside example fences are not extracted. Full-mode only (`.expected.full.json`). Formerly a separate QMD-7 suite, now part of this single parser corpus.

## Rebuild Tests

Parser microtests verify round-trip in two ways:

**JSON round-trip** (parse → rebuild → parse): Parses the original, rebuilds, parses the result, compares JSON. Guarantees rebuild generates valid QMD.md.

**Text round-trip** (content loss detection): Compares original text with rebuilt text using LCS-diff. Normalizes both sides (removes `[[...]]` brackets, quotes, HTML comments, heading markers, whitespace). If normalized versions differ — real content loss (FAIL). Heading level changes are also flagged.

Implemented in all three parsers.
