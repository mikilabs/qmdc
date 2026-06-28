# Full index of the micro-tests (001-050)

## 001-010: Basic objects and metadata

| # | Name | What it checks |
|---|----------|---------------|
| 001 | empty-object | Empty object (H2 heading only) |
| 002 | object-explicit-id | Explicit ID at the end `## User [[user_123]]` |
| 003 | id-at-start | ID at the start `## [[admin]] Admin User` |
| 004 | id-in-middle | ID in the middle `## Super [[admin_user]] Account` |
| 005 | two-objects | Two objects in one file |
| 006 | object-with-kind | Kind in the heading `[[user_1: Person]]` |
| 007 | only-kind-no-id | Kind only `[[:Person]]` |
| 008 | h3-nested-object | H3 as a nested object |
| 009 | two-objects-with-ids | Two objects with explicit IDs |
| 010 | label-with-dash | Label with a dash → ID autogeneration |

## 011-020: Primitive types

| # | Name | What it checks |
|---|----------|---------------|
| 011 | one-string-field | Single string field |
| 012 | number-field | Integer |
| 013 | boolean-field | `true` and `false` |
| 014 | null-field | `null` value |
| 015 | multiple-fields | Several fields of different types |
| 016 | string-with-spaces | String with spaces |
| 017 | float-number | Float `99.99` |
| 018 | negative-number | Negative number `-500` |
| 019 | quoted-string | Quoted string `"Hello, World!"` |
| 020 | zero-number | Zero `0` |

## 021-030: Multiline fields and arrays

| # | Name | What it checks |
|---|----------|---------------|
| 021 | multiline-text | Multiline text field (H3) |
| 022 | yaml-array | YAML array `[a, b, c]` + `__syntax` |
| 023 | markdown-list-array | Markdown list as an array + `__syntax` |
| 024 | empty-array | Empty array `[]` |
| 025 | numbers-array | Array of numbers `[80, 443, 8080]` |
| 026 | nested-object | Nested object via H3 |
| 027 | mixed-array | Mixed array `[1, true, "test", null]` |
| 028 | object-and-field | Simple field + nested object |
| 029 | two-nested-objects | Two nested objects (H3) |
| 030 | multiline-with-blank | Multiline text with a blank line |

## 031-038: Arrays of objects, tables, YAML

| # | Name | What it checks |
|---|----------|---------------|
| 031 | array-of-objects | Array of objects via H4 headings |
| 032 | table | Markdown table → array of objects |
| 033 | comment-before-field | Comment before a field (`after: "__self"`) |
| 034 | comment-after-field | Comment after a field (`after: "name"`) |
| 035 | html-comment | HTML comment `<!-- ... -->` (ignored) |
| 036 | reference-string | Reference as a string `[[#user_123]]` |
| 037 | reference-array | Array of references `[[[#order_1]], [[#order_2]]]` |
| 038 | yaml-block | YAML block `[[field: yaml]]` + `__syntax` |

## 039-045: Complex cases

| # | Name | What it checks |
|---|----------|---------------|
| 039 | reference-with-kind | Reference with Kind `[[#User:alice]]` |
| 040 | comment-multiple | Multiple comments in the `__comments` array |
| 041 | list-one-item | Single-item Markdown list |
| 042 | table-one-row | Table with one data row |
| 043 | array-one-object | Array with one object (H4) |
| 044 | bold-metadata | Bold metadata `**kind**: Person` |
| 045 | colon-in-value | Colon in a value `http://localhost:8080` |

## 046-050: Edge cases and special metadata

| # | Name | What it checks |
|---|----------|---------------|
| 046 | explicit-kind | Explicit `__kind` via bold |
| 047 | workspace-namespace | Explicit `__workspace` and `__namespace` |
| 048 | empty-string | Empty string `""` as a value |
| 049 | special-chars | Email and URL with special characters |
| 050 | nested-levels | Deep nesting (H2→H3→H4→H5) |

---

**Files:**

- `NNN-name.qmd.md` — input QMD document
- `NNN-name.expected.json` — expected result

**Usage:** see `README.md` for instructions on running the tests.
