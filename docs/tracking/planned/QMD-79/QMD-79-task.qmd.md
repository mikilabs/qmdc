# QMD-79: an HTML comment inside an object's comments

## An HTML comment in `__comments` is dropped by Rust and kept by Python and TypeScript [[qmd79_html_comment_in_comments: Bug]]

An object's prose after its fields becomes `__comments`. When that prose holds an HTML comment, Rust
drops it and Python and TypeScript keep it in the comment's content, so one document yields two
different objects. The spec says HTML comments are "ignored completely (not captured in
`__comments`)", which is what Rust does.

Found during the code review round of [[#qmd77_tails]] and outside its goals, so it is its own task
under that task's rule rather than another tail.

- status: planned
- priority: low
- category: parser
- related_task: [[#qmd77_tails]]
- requires_changes: []
- findings: [[#qmd79_finding_measurement]]
- result: null

### Goals [[goals: [Goal]]]

#### A1: An HTML comment in an object's comments is handled the same way in all three [[qmd79_goal_a1]]

The comment content an object carries is identical in Rust, Python and TypeScript when the prose
after its fields holds an HTML comment, and it is what the format documents.

- group: A_comments
- done: false

#### A2: The behaviour is pinned by a fixture seen red on the pre-change binaries [[qmd79_goal_a2]]

A parser fixture with an HTML comment between two paragraphs after an object's fields, and one with
an HTML comment as the only text after the fields, each seen red on the pre-change build.

- group: A_comments
- done: false
