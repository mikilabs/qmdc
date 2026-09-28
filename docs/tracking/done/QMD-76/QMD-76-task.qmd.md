# QMD-76: the nested_workspace report explained nothing and claimed the wrong rule

## nested_workspace named neither the consequence nor the remedy [[qmd76_nested_message: Bug]]

Running `qmdc workspace validate <repo>` on a repository managed by `qmdc-model` reported one error
and gave the user nothing to act on:

```text
nested_workspace: Nested workspace 'qmdc' found inside workspace. Workspaces cannot be nested.
  .qmdc/readme.qmd.md:1
```

Two things were wrong with it. The claim is not the rule: workspaces CAN sit inside one another and
the container form composes them without complaint — `--with <root> --with <root>/.qmdc` on the same
repository returns `[]` and exit 0. And the report named no remedy, so the reader's only conclusion
was that the layout `qmdc-model init` had just created was illegal.

What the rule actually means is that the outer scan leaves the inner workspace's files out, so they
are missing from the graph this run produced — measured on the reporting repository, 250 `*.qmd.md`
on disk, 240 scanned, and the 10 left out are exactly `.qmdc/`. The report is therefore correct and
must stay; only its wording was wrong.

- status: done
- priority: medium
- category: parser
- related_task: [[#qmd75_body_identity]]
