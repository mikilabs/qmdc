# QMD-80: the depth to which files outside every workspace are loaded

## Rust loads files outside every workspace to depth 5, Python and TypeScript to any depth [[qmd80_orphan_depth: Bug]]

`query` on a directory that is not itself a workspace also loads the `.qmd.md` files lying outside
every workspace under it. Rust walks that directory to depth 5, Python and TypeScript walk it to any
depth, so a file six or more levels down is in the graph of two parsers and missing from the third,
silently.

Found during the code review round of [[#qmd77_tails]] and outside its goals, so it is its own task
under that task's rule rather than another tail.

- status: planned
- priority: low
- category: workspace
- related_task: [[#qmd77_tails]]
- requires_changes: []
- findings: [[#qmd80_finding_measurement]]
- result: null

### Goals [[goals: [Goal]]]

#### A1: Files outside every workspace are loaded to the same depth in all three [[qmd80_goal_a1]]

The set of files outside every workspace that `query` loads is identical in Rust, Python and
TypeScript, at the depth the format documents.

- group: A_depth
- done: false

#### A2: The depth is pinned on both sides of the limit [[qmd80_goal_a2]]

A CLI fixture with one file exactly at the limit and one a level deeper, seen red on the pre-change
build, so an off-by-one in either direction fails.

- group: A_depth
- done: false
