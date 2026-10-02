# QMD-78: an ignore rule and a path whose bytes are not valid UTF-8

## A non-UTF-8 path byte matches differently in each parser [[qmd78_non_utf8_path: Bug]]

An ignore rule matches a path whose bytes are not valid UTF-8 in Python but not in Rust or
TypeScript. Python carries the raw bytes through `surrogateescape` and matches them; the other two
replace each invalid byte with U+FFFD before matching, so the pattern no longer applies and a file the
operator excluded is scanned.

Split out of [[#qmd77_tails]], where it was goal D2 and the one item that could not be closed with the
rest: the divergence is unreachable on APFS, which refuses such a filename outright, and real on Linux
ext4. Closing it needs a Linux run, and the task's own verification rule says a goal that cannot be
seen red must be split rather than claimed.

- status: planned
- priority: low
- category: parser
- related_task: [[#qmd77_tails]]
- requires_changes: []
- findings: [[#qmd78_finding_measurement]]
- result: null

### Goals [[goals: [Goal]]]

#### A1: A non-UTF-8 path byte matches the same way in all three [[qmd78_goal_a1]]

An ignore pattern applied to a path carrying an invalid UTF-8 byte produces the same match decision in
Rust, Python and TypeScript, and the decision the format documents.

- group: A_bytes
- done: false

#### A2: The behaviour is pinned by a test that can run on Linux [[qmd78_goal_a2]]

A fixture creates the offending filename at run time (it cannot be committed: a checkout on APFS or
NTFS would fail), skips itself where the filesystem refuses the name, and is seen RED on the
pre-change binaries on Linux before it is accepted.

- group: A_bytes
- done: false
