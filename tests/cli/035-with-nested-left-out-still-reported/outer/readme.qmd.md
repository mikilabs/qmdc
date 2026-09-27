# Outer [[cli72_left_outer: __Workspace]]

## Project [[project: Thing]]

The same layout as `034-with-nested-member-not-an-error`, but only the outer workspace is
composed. The inner one is then left out of the result, and its files are missing from it:
that is what `nested_workspace` exists to report, so it must still fire. Together with 034
this pins the rule to "reported exactly when the nested workspace is not a member", not
"never reported under `-w`".

- model: [[#cli72_left_runtime::model]]
