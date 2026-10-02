# QMD-73: Result

## `.qmdcignore` means what `.gitignore` means, in all three parsers [[qmd73_result: Result]]

The three parsers now decide ignored or kept exactly as git does. Each carries the same port of
git's own matcher, and every scan asks it, ancestors first. On 79 line forms, whose expected
answers git itself generated, Rust, Python and TypeScript all give git's answer. At this
repository's root the three now scan the same 118 files, and TypeScript's 292 errors are gone.

All eight goals are complete. `make test` is green: 4072 cases in the unified report, 0 failures,
with the new `ignore` suite at 79 cases in each language under enforced parity.

- feature: [[#qmd73_qmdcignore]]
- completed: 2026-09-27
- files_changed: [qmdc-rs/src/ignore.rs, qmdc-rs/src/lib.rs, qmdc-rs/src/workspace.rs, qmdc-rs/src/lsp/server.rs, qmdc-rs/Cargo.toml, qmdc-py/qmdc/ignore.py, qmdc-py/qmdc/workspace.py, qmdc-ts/src/ignore.ts, qmdc-ts/src/workspace.ts, qmdc-ts/package.json, qmdc-ts/package-lock.json, scripts/test-report.py, scripts/test-baseline.json, docs/architecture/algorithms.qmd.md, CHANGELOG.md]
- tests_added: [tests/ignore/gitignore-matrix.json, tests/ignore/gen_matrix.py, qmdc-rs/tests/ignore_matrix.rs, qmdc-py/tests/test_ignore.py, qmdc-ts/tests/test-ignore.ts, tests/workspace/qmdcignore-gitignore, tests/cli/036-with-honours-qmdcignore]

### What changed [[qmd73_result_changes: text]]

- about: [[#qmd73_result]]

- **One matcher.** `ignore.rs`, `ignore.py` and `ignore.ts` port git's `dowild()` and its
  `dir.c` line parsing and matching, function for function. `globset`, `minimatch` and `fnmatch`
  leave the parsers.
- **One question per scan.** File scans, workspace discovery, the bounded `--with` scan and the
  Rust LSP folder scan all call `is_ignored` with the path's type. Python's `__dummy__` probe and
  TypeScript's subtree skip on an ignored readme are gone.
- **`--with` agrees.** The path's own `.qmdcignore` now steers the search for its workspace in
  Python and TypeScript too, as it already did in Rust.
- **Pinned.** Six workspace fixtures for the reported shapes and the re-include rule, one CLI
  fixture for `--with`, and the 79-case matrix as a shared suite. The rules are written into
  `docs/architecture/algorithms.qmd.md`, and `CHANGELOG.md` names the lines whose meaning changed.
- **Left for a separate task.** Rust alone reports `qmdc_guide` as ambiguous at the repository
  root, and Python alone misses a workspace nested inside a nested workspace. Both are older than
  this task and not about ignoring. Review added two more: TypeScript's `scanWorkspace` throws an
  uncaught EACCES on an unreadable directory and returns nothing where Python, Rust and git skip
  it and return the readable files (pre-existing, confirmed by running `HEAD`'s scanner); and
  invalid-UTF-8 path bytes decode differently in Python than in the other two, unreachable on
  macOS but a real divergence on a filesystem that permits such names.

**Post-review fixes (CR #04, `reviews/04-cr-qmd73-qmdcignore.md`):** four reviewers, nine items,
no blocker and no high. The matrix grew from 74 to 79 forms because the first 74 did not pin `**`
at all — a matcher in which every `**` crosses `/` passed all of them — and the two new
double-star cases now hold both halves of the rule. The architecture doc's `**` rule was wrong
(git strips a line's literal prefix, so `te**/c.qmd.md` does cross) and is rewritten. A dead
readme re-filter in `parse_all_workspaces` was removed in all three languages after measuring
that the case it would cover is already handled by discovery; Rust's five unpruned walkers now
use `filter_entry` like their sibling; the `ignore` re-export in `workspace.rs` became private;
`gen_fixtures.py` reads `ls-files` NUL-separated; and the changelog now says an existing ignore
file may hide a different set of files.
