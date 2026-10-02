## Bug [[bug: Bug]]

QMD-70: the table below sits inside goal A2, which is an ELEMENT of the `goals` array. It is
ordinary content of that element, so the element — and the reference to it from `tracks` —
must survive.

Today it does not in Rust: the array captures the table's rows, `goal_a2` is replaced by an
anonymous `goals_0`, and the reference to it becomes a false `broken_link`. Python and
TypeScript keep the element in this shape and report nothing, so the three parsers disagree
about whether this file is valid.

This is the VALIDATOR half of the bug. The parser half is covered by the microtests
`212`-`215`; neither those nor any other fixture reaches `workspace validate`, which is why
the divergence went unnoticed.

- tracks: [[#goal_a2]]

### Goals [[goals: [Goal]]]

#### A2 [[goal_a2]]

Prose before the table.

| form | means |
| --- | --- |
| id | this workspace |

- group: A
- done: false
