# Bug [[bug: Bug]]

QMD-70 on the LSP and MCP surfaces. The table belongs to goal A2, an ELEMENT of the `goals`
array, so the element and the reference to it must survive. In Rust the array swallows the
table's rows, the element disappears, and the editor shows a false QMDC001 on a reference to
a goal that is written right there in the file.

- tracks: [[#goal_a2]]

## Goals [[goals: [Goal]]]

### A2 [[goal_a2]]

Prose before the table.

| form | means |
| --- | --- |
| id | this workspace |

- group: A
