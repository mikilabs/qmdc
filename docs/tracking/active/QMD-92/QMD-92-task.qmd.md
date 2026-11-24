# QMD-92: Flaky LSP diagnostics microtest under load

## The LSP microtest harness adds up repeated diagnostics publishes [[qmd92_lsp_diag_flake: Bug]]

GitHub [#31](https://github.com/mikilabs/qmdc/issues/31). Under the full parallel
`make test-fast`, the Rust LSP microtest `diagnostics/036-table-in-primitive-array` sometimes
gets its one `table_in_array` diagnostic twice and fails. Run alone, it passes.

The fix must not change the server's behaviour.

- status: done_review
- priority: medium
- category: testing
- findings: [[#qmd92_finding_cause]], [[#qmd92_finding_fix]]
- result: [[#qmd92_result]]

### Goals [[goals: [Goal]]]

#### A1: A test reproduces the duplicate [[qmd92_goal_a1]]

A unit test of the harness's diagnostics collection, fed two publishes for one file, fails on
the current behaviour for the documented reason.

- group: A_tests
- done: true

#### B1: The harness keeps the latest publish per URI [[qmd92_goal_b1]]

The diagnostics microtests read what an LSP client would show: the last publish for each
file. The new test and all LSP microtests pass, and so does `make test-fast`.

- group: B_fix
- done: true
