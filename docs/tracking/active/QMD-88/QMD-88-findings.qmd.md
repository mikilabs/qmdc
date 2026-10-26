# QMD-88: Findings

## The specification excluded body text [[qmd88_finding_spec: Finding]]

The Rules of `docs/format/references.qmd.md` said references work only in field values. The parsers
did not follow it (Rust read body paragraphs, Python and TypeScript read single-line body text) and
the docs themselves link from body text throughout. Decided with the operator: references work in
field values and in body text; inline code follows CommonMark; fenced code in body text is a sample
and is not parsed, while text fields keep their existing rule (only `example` fences excluded).

- category: design
- related_to: [[#qmd88_references_parity]]
- solution: The spec's Description, Syntax and Rules now say so, and `docs/format/comments.qmd.md` says comment text holds references.

## Five causes, each in one or two parsers [[qmd88_finding_causes: Finding]]

- category: parser
- related_to: [[#qmd88_references_parity]]
- affected_files: [qmdc-rs/src/parser.rs, qmdc-rs/src/core/reference_scan.rs, qmdc-rs/src/parser_modules/references.rs, qmdc-py/qmdc/parser.py, qmdc-ts/src/parser.ts]
- solution: Fixed per cause below; all three parsers now apply the same rule.

### The causes [[qmd88_finding_causes_list: text]]

- about: [[#qmd88_finding_causes]]

1. Multi-line body text (Python, TypeScript): a comment was matched only when its whole content sat
   on one source line, so references in a multi-line paragraph, list or table were lost. Now every
   line is read, the way multiline text fields are. Rust read body paragraphs only and now reads
   lists, tables and quotes too.
2. Inline code: three different backtick checks. Rust counted every backtick, so ``` ``[[#x]]`` ```
   was a reference; Python and TypeScript paired double backticks, so ``` `` `[[#x]]` `` ``` was one.
   All three now use CommonMark's rule: a run of N backticks is closed by the next run of N.
3. Lines of another object (Python, TypeScript): a list or comma-separated field's line was searched
   from the top of the file, so an identical `- about:` line of an earlier object was used.
4. Shared cursor and repeated lines: Python and TypeScript searched other fields from a cursor shared
   by the object, and Rust started every multiline text field at the object's line, so a value that
   also appeared earlier (body text, another field's preamble) was attributed to the earlier line and
   then dropped as a duplicate. Each field's search now starts at its own `- key:` line or `[[key]]`
   heading (Rust: its `__positions` line).
5. Parent fields (Python, TypeScript): a parent's field naming a child (`auth: [[#auth]]` from a
   `## Auth [[auth]]` heading) is structure, not source text; it was searched and picked up the
   child's own references. It is now skipped, with the test the field-position code already uses.

## Body-text references in the edges table [[qmd88_finding_edges: Finding]]

- category: design
- related_to: [[#qmd88_references_parity]]
- affected_files: [qmdc-rs/src/db/mod.rs, qmdc-rs/src/core/ops/references.rs, qmdc-rs/src/main.rs, qmdc-py/qmdc/db.py, qmdc-ts/src/db.ts]
- solution: Each reference in `__references` that no field holds becomes an edge with source field and edge type `__comments`, resolved like a field reference (a field path keeps `target_field`). A `__Workspace` root resolves inside the workspace it names. `find_references` reads the table. Rust's edge extractor reads only `[[#target]]`, as the other two do (it also read `["id"` and definitions). Rust `qmdc query` parses in full format so `__references` is present.

## What is not aligned yet [[qmd88_finding_remaining: Finding]]

- category: parser
- related_to: [[#qmd88_references_parity]]
- solution: Out of scope. In 7 of the 65 `tests/workspace` fixture workspaces (10 before) the parsers still differ in `__references` because they differ in the parsed objects themselves (a list under a `[string]` heading kept as a comment by Python and TypeScript, a `- __parent:` line, `[[#file]][]`, hierarchical-id children). `docs`, which the parse-parity gate covers, is identical. The parity gate does not compare `__references`; extending it is a maintainer decision.

## Tests [[qmd88_finding_tests: Finding]]

- category: testing
- related_to: [[#qmd88_references_parity]]
- solution: Data-driven fixtures run by all three parsers, each failing on the old code.

### Fixtures [[qmd88_finding_tests_list: text]]

- about: [[#qmd88_finding_tests]]

- `tests/parser/298-reference-multiline-paragraph`: references in a three-line paragraph.
- `tests/parser/299-reference-body-text-blocks`: paragraph, list, quote and table references in body
  text; fenced and `example` code not parsed.
- `tests/parser/300-reference-inline-code-spans`: single, double and double-around-single code spans,
  an unclosed backtick.
- `tests/parser/301-reference-line-of-own-field`: a field's references come from its own line, not
  an identical earlier one.
- `tests/parser/302-reference-repeated-preamble`: a preamble repeated across text fields is kept for
  each.
- `tests/workspace/body-text-references`: shared SQL test of body-text edges, field paths, inline
  code and the workspace root's child field.
