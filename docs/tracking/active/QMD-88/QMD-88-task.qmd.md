# QMD-88: the three parsers disagree on an object's references

## Parsers report different references; body text is not in the graph [[qmd88_references_parity: Bug]]

GitHub [#21](https://github.com/mikilabs/qmdc/issues/21), filed from #18. The Rust, Python and
TypeScript parsers produced different `__references` for the same document: on the `docs` workspace
the gap was 129 references only Rust reported and 181 only Python and TypeScript reported, across
108 objects. `__references` is what `validate`, the LSP and `find_references` read. The parse-parity
gate compares the default format, which has no `__references`, so the gap was never caught.

The goal behind it (#18): references written in body text become edges, so `edges`, `traverse`,
`find_path` and `find_references` agree, and agree across the three parsers.

- status: done_review
- priority: high
- category: parser
- related_task: [[#qmd87_graph_unresolved]]
- findings: [[#qmd88_finding_spec]], [[#qmd88_finding_causes]], [[#qmd88_finding_edges]], [[#qmd88_finding_remaining]], [[#qmd88_finding_tests]]
- result: [[#qmd88_result]]

### Goals [[goals: [Goal]]]

#### A1: One reference rule in the specification [[qmd88_goal_a1]]

`docs/format/references.qmd.md` says where references work (field values and body text), how
inline code is recognised, and that fenced code in body text is not parsed.

- group: A_spec
- done: true

#### B1: The parsers agree on __references [[qmd88_goal_b1]]

Each cause is fixed in every parser it affects and pinned by a `tests/parser` fixture that all three
run; on `docs`, `__references` is identical (target, line, column) for every object.

- group: B_parsers
- done: true

#### C1: Body-text references are edges [[qmd88_goal_c1]]

The edges table holds references written in body text, `find_references` reads that table, and the
table is identical across the three parsers on `docs`.

- group: C_edges
- done: true
