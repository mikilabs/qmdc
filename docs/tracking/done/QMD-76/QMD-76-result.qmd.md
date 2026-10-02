# QMD-76: Result

## Reworded, single shape, one shared builder per parser [[qmd76_result: Result]]

The report now states what is missing and how to get it back, in one wording built by one function
per parser (`nested_workspace_message` in Rust, `_nested_workspace_message` in Python,
`nestedWorkspaceMessage` in TypeScript), each carrying the same comment about why nesting is reported
at all:

```text
nested_workspace: Nested workspace 'qmdc' inside this workspace: its files are excluded from this
graph. Validate both together: --with <root> --with <root>/.qmdc
```

The inner workspace's directory is inserted relative to the outer root, so the remedy is
copy-pasteable and no absolute path from the running machine reaches the message. `<root>` stays a
placeholder rather than the real path for the same reason: the message is identical on every machine,
which is what lets one shared CLI fixture pin it for all three parsers.

Type, severity, `file`, `line`, `objectId` and the exit code are untouched, so nothing that consumes
the error programmatically changes. The rule itself is untouched: reported when the inner workspace is
left out, dropped when both are composed — the pair of fixtures QMD-72 wrote for exactly this
(`034-with-nested-member-not-an-error`, `035-with-nested-left-out-still-reported`) is what proved the
old message's claim false, since 034 is a nested workspace that validates clean.

Verified on the repository that produced the original report: the single error now carries the remedy,
and running that remedy returns `[]` with exit 0.

One item is out of this repository's reach and is left as a note rather than a change here:
`qmdc-model init` creates the shape that triggers the report — a `.qmdc/readme.qmd.md` declaring
`[[qmdc: __Workspace]]` inside a repository whose root readme already declares one — and it is the
natural place to tell the user, at scaffold time, that the repository is from then on a container of
two workspaces. The scaffold lives in the separate `qmdc-model` package, at
`src/qmdc_model/templates/workspace-readme.qmd.md`.

- feature: [[#qmd76_nested_message]]
- files_changed: [qmdc-rs/src/workspace.rs, qmdc-py/qmdc/workspace.py, qmdc-ts/src/workspace.ts, tests/cli/035-with-nested-left-out-still-reported/expected.json, CHANGELOG.md]
- tests_added: []
