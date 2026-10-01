# QMD-80: Findings

## Measured on the QMD-77 build [[qmd80_finding_measurement: Finding]]

Measured by running all three parsers on one input: a directory holding a workspace `one/` and two
files outside it, one at depth 5 (`d1/d2/d3/d4/five.qmd.md`) and one at depth 7
(`d1/d2/d3/d4/d5/d6/seven.qmd.md`), each declaring one object, queried for both ids.

| parser | objects found |
|---|---|
| Rust | `at_five` |
| Python | `at_five`, `at_seven` |
| TypeScript | `at_five`, `at_seven` |

Rust's walk sets `max_depth(5)` in `qmdc-rs/src/workspace.rs`; Python's `rglob` and TypeScript's
`findQmdcFiles` have no limit. Workspace discovery already uses depth 5, documented in
`docs/mcp/readme.qmd.md`, so which depth applies to these files is the decision to make first.

- category: workspace
- related_to: [[#qmd80_orphan_depth]]
- solution: Decide the depth for files outside every workspace; then align the parsers that differ.
