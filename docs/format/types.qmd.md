# Data Type [[data_type: SyntaxConcept]]

- depends: [[#field]]

## Description [[description: text]]

QMD.md supports these primitive data types: string, number, boolean, null. Type auto-detection rules determine how field values are parsed.

## Syntax [[syntax: text]]

Type auto-detection rules (applied to field values):

1. `true` | `false` → Boolean (lowercase only)
2. `null` or empty value after colon → Null
3. Integer or decimal → Number (`-?\d+(\.\d+)?`, magnitude from `1e-4` to 2^53-1)
4. Everything else → String

NOT supported: `yes`, `no`, `True`, `FALSE` (for unambiguity)

Nor are these numeric forms — each is a String **and raises `unsupported_number_format`**, because
each one only ever worked in whichever host language happened to accept it: `1e5`, `1.5e-3`, `2E3`
(exponents), `.5` and `5.` (a bare leading or trailing dot), `+1` (unary plus), `1_000` (digit
separators), `0x10` and `0o17` (other bases). The authored text is kept as the value, so nothing is
lost — the error exists so the spelling is refused out loud rather than silently reinterpreted.

A bare `~` is likewise a String, not null — that spelling belongs to YAML — but it raises no error,
because it is not a number anyone mis-spelled. `Infinity` and `NaN` are plain Strings for the same
reason.

Forcing string type: use quotes `"123"` to prevent number parsing.

Empty string requires quotes: `- field: ""` (without quotes, empty value = null).

## Primitives [[primitives: text]]

- String: text values. Quotes optional for simple strings. Required in YAML arrays for values with spaces/commas.
- Number: an integer or a decimal, optionally negative — exactly `-?\d+(\.\d+)?`, with a magnitude between `1e-4` and `9007199254740991` (2^53-1). No exponents, no bare leading or trailing dot, no unary plus, no digit separators, no alternative bases. A literal outside the range keeps its authored text as a String and raises `unsupported_number_format`, so digits are never silently rounded (`9223372036854775807` stays text) and nothing is written back in a spelling the format cannot read (`0.00001` would need an exponent). Within the range a decimal carries IEEE-754 double semantics, so authored precision beyond a double's is not preserved (`0.1234567890123456789` reads back shorter).
- Boolean: `true` or `false` only (lowercase). `True`, `FALSE`, `yes`, `no` are strings.
- Null: keyword `null` or empty value after colon (`- field:`).
- Array: ordered list of primitives or objects. Two syntaxes: YAML notation and Markdown lists.
- Object: nested structure with fields. Created via subheadings — result is a separate object + reference in parent. Everything is flat: array of objects + `[[#id]]` references.
- Map: flat string dictionary (str→str). Defined via `[[field: map]]`. No type auto-detection — all values stored as strings.

## Map Type [[map_type: text]]

The `map` type is a flat dictionary `str→str`. Defined via `[[field: map]]` heading syntax. Type auto-detection is NOT applied: all values are stored as strings. `true`, `false`, `null`, numbers — everything remains a string.

```markdown example
### env [[env: map]]
- port: 8080
- debug: true
- count: null
- description:
```

Result: `{"port": "8080", "debug": "true", "count": "null", "description": ""}`.

Map supports multiline values via YAML pipe syntax `|`.

## Rules [[rules: text]]

- Type detection is case-sensitive: only lowercase `true`, `false`, `null` are recognized
- Quoted values are always strings regardless of content
- Empty value after colon = null; empty string requires quotes
- Map fields accept only bullet lists with `- key: value` pairs
- Invalid map entries generate `invalid_map_entry` error
- Non-bullet content in map fields generates `invalid_map_content` error
