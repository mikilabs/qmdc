# QMD-88: Result

## One reference rule in three parsers; body text in the graph [[qmd88_result: Result]]

The specification now says references work in field values and in body text, with inline code
recognised by CommonMark's rule and fenced code in body text not parsed
([[#qmd88_finding_spec]]). The Rust, Python and TypeScript parsers apply that rule the same way
([[#qmd88_finding_causes]]): on `docs`, `__references` is identical for every object, target, line
and column. The edges table includes body-text references and is identical across the three parsers;
`find_references`, `traverse` and `find_path` agree with it ([[#qmd88_finding_edges]]).

- feature: [[#qmd88_references_parity]]
- files_changed: [docs/format/references.qmd.md, docs/format/comments.qmd.md, qmdc-rs/src/parser.rs, qmdc-rs/src/core/reference_scan.rs, qmdc-rs/src/parser_modules/references.rs, qmdc-rs/src/db/mod.rs, qmdc-rs/src/core/ops/references.rs, qmdc-rs/src/main.rs, qmdc-py/qmdc/parser.py, qmdc-py/qmdc/db.py, qmdc-ts/src/parser.ts, qmdc-ts/src/db.ts]
- tests_added: [tests/parser/298-reference-multiline-paragraph, tests/parser/299-reference-body-text-blocks, tests/parser/300-reference-inline-code-spans, tests/parser/301-reference-line-of-own-field, tests/parser/302-reference-repeated-preamble, tests/workspace/body-text-references]

### Verification [[qmd88_result_verification: text]]

- about: [[#qmd88_result]]

- On `docs`, objects whose `__references` differ between Rust and Python: 108 before, 0 after;
  Python and TypeScript identical before and after.
- Edges table on `docs`: Rust 1797 / Python 1731 / TypeScript 1731 with body edges from the old
  reference lists; now 1860 in each, identical, 141 from body text. On `main` the tables already
  differed by 3 edges (Rust read a JSON sample as references); gone.
- `find_references` against the 2.0.1 binary on `docs`: no object loses a referrer; 637 referrers are
  added (body text, parent child fields, workspace roots, `__Document` objects). `traverse`,
  `find_path` and `find_references` agree with the edges table on all 777 objects.
- Each new parser fixture fails on the old code in the parsers that had the cause and passes in all
  three.
- Gates: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`; Python
  `pytest tests/` (1194 passed); TypeScript `npm test` (all suites, 0 failures); `make
  validate-docs`; `scripts/compare_validate_errors.sh docs` (identical); `scripts/compare_parse_output.py`
  (exact parity, 154 documents).
- Behaviour changes users will see: `validate` checks references in body-text lists, tables and
  quotes (previously paragraphs only in Rust, single lines in Python and TypeScript); a double-backtick
  code span hides references in all three; `find_references` and the graph include body-text and
  structural referrers.
- Code review: self-review; no review agent could be started from this session.
