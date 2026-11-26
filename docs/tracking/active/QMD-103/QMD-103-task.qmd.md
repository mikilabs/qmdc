# QMD-103: Generated nav names readmes by their heading

## The generated nav labels namespace and workspace readmes "Readme" [[qmd103_readme_nav_label: Bug]]

GitHub [#39](https://github.com/mikilabs/qmdc/issues/39), part 1. In the nav that `init` and
`build` generate, a readme whose heading declares `__Namespace` or `__Workspace` is listed as
`Readme`, while the graph sidebar names the same page by its heading. In a namespace build every
page falls back to its file name. Part 2 of the issue (ordering pages by label instead of path)
changes the order sites rely on and is left for a maintainer decision.

- status: done_review
- priority: medium
- category: mkdocs
- findings: [[#qmd103_finding_cause]], [[#qmd103_finding_fix]]
- result: [[#qmd103_result]]

### Goals [[goals: [Goal]]]

#### A1: The wrong labels are reproduced and pinned by tests [[qmd103_goal_a1]]

On the issue's workspace the generated nav lists `Readme` twice; in a namespace build it also
lists `Alpha` for the page titled `Zebra`. Nav tests for both builds fail on the current code.

- group: A_tests
- done: true

#### B1: Every nav entry uses the page's heading [[qmd103_goal_b1]]

A readme is named by its `__Namespace` / `__Workspace` heading and every other page by its
level-1 heading, in full and namespace builds. Page order is unchanged.

- group: B_fix
- done: true
