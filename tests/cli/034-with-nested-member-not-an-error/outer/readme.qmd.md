# Outer [[cli72_nest_outer: __Workspace]]

## Project [[project: Thing]]

A repository that is a workspace and also carries a second, independent workspace in a
subdirectory: the layout qmdc-wiki gives every repository it models (`.qmdc/`). Composed
with `-w ./outer -w ./outer/.qmdc`, the inner one is a member of the set, so its files are in
the result under its own id and `nested_workspace` has nothing to report.

The assertion is not an empty list, which loading nothing would satisfy. The qualified
reference below resolves only if BOTH workspaces are loaded, and the inner workspace carries
one genuinely broken reference, so the expected output is exactly that one `broken_link`,
reported under the inner workspace's id, and no `nested_workspace`.

- model: [[#cli72_nest_runtime::model]]
