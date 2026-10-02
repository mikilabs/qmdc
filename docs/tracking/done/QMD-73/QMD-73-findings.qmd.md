# QMD-73: Findings

## 38 line forms against git: each parser breaks about half of them, differently [[qmd73_finding_matrix: Finding]]

Measured with git as the oracle, because the spec says "works like .gitignore". For each of 38 line
forms, a scratch workspace holding the same 20 files got the lines both as `.qmdcignore` and as
`.gitignore`, and each parser's `workspace parse` file list was compared with what
`git ls-files --others --exclude-standard` keeps. The inputs, git's answers and each parser's answers
before the fix are in `artifacts/gitignore-matrix.json`; `artifacts/measure_matrix.py` reproduces it.

Rust differs from git on 19 forms, Python on 17 and TypeScript on 22. On 14 forms the three parsers
also disagree with each other. All three agree with git on 13 forms, and apart from the root line
`tests/*` those are exactly the forms this repository and its fixtures use, which is why no existing
test noticed.

- category: parser
- related_to: [[#qmd73_qmdcignore]]
- affected_files: [docs/tracking/active/QMD-73/artifacts/gitignore-matrix.json, docs/tracking/active/QMD-73/artifacts/measure_matrix.py]
- solution: Use the matrix as the acceptance gate (goal B3). After the fix, every parser's answer equals git's on all 38 forms.

### By rule [[qmd73_finding_matrix_rules: text]]

- about: [[#qmd73_finding_matrix]]

| git rule | lines from the matrix | rs | py | ts |
| --- | --- | --- | --- | --- |
| a star never crosses a slash | `docs/*.qmd.md`, `tracking/*/artifacts/` | also hides deeper files | also hides deeper files | ok |
| a directory matched by a star hides its contents | `tests/*` | ok, because its star crosses slashes | ok, same reason | hides the first level only |
| a directory matched by a plain name or path hides its contents | `tests`, `tests/sub`, `**/tests` | hides nothing | hides nothing | hides nothing |
| no slash, or only a trailing one: any depth | `build/`, `sub/`, `te?ts/` | root only | root only | root only |
| the same for a file name | `a.qmd.md`, `test_*.qmd.md` | root only | ok | root only |
| a leading slash anchors the line | `/tests`, `/a.qmd.md` | hides nothing | hides nothing | hides nothing |
| a negated line re-includes a path | `keep/*` then `!keep/important.qmd.md` | the line has no effect | the line has no effect | hides the whole workspace |

The last row is the worst. In TypeScript any line that starts with `!`, even a lone one, hides every
file it does not name, the workspace's own `readme.qmd.md` included, so the workspace silently drops
out of the result. All six negation forms in the matrix do this. Rust and Python ignore such a line,
which happens to be right in the two forms where git refuses the re-include.

## Three glob engines, none of them gitignore [[qmd73_finding_engines: Finding]]

All three parsers prepare the lines the same way: blank lines and `#` comments are dropped, and a
trailing `/` is rewritten to `/**`. Each then hands the line to a general-purpose glob matcher, a
different one in each language, so the divergence is whatever those matchers happen to do with `*`,
`/` and `!`.

- category: parser
- related_to: [[#qmd73_qmdcignore]]
- affected_files: [qmdc-rs/src/workspace.rs, qmdc-rs/src/lsp/server.rs, qmdc-py/qmdc/workspace.py, qmdc-ts/src/workspace.ts]
- affected_functions: [load_qmdcignore, is_ignored, loadQmdcignore, isIgnored]
- solution: Replace the glob matchers with one ignore decision per parser that follows git's rules and checks ancestor directories before the path, and route every call site through it.

### Per parser [[qmd73_finding_engines_detail: text]]

- about: [[#qmd73_finding_engines]]

- Rust builds a `globset::GlobSet` with default options (`qmdc-rs/src/workspace.rs:806`), in which
  `*` crosses `/`, and matches the whole relative path (`workspace.rs:844`). Workspace discovery
  prunes directories with `filter_entry`. The LSP folder scan reuses the same pair
  (`qmdc-rs/src/lsp/server.rs:292`).
- Python calls `fnmatch` (`qmdc-py/qmdc/workspace.py:1420`), in which `*` also crosses `/`, retries
  every line against the bare file name, and rewrites `**/` to `*`. It decides whether a directory
  is ignored by asking about a made-up file inside it, `d_path / "__dummy__"` (`workspace.py:176`,
  `workspace.py:252`).
- TypeScript calls `minimatch` with default options (`qmdc-ts/src/workspace.ts:1458`), in which `*`
  stops at `/` and nothing is retried against the file name. A leading `!` negates the glob itself:
  `!x` matches every path except `x`, and every such path is reported as ignored.
- None of the three has git's rule that an excluded directory hides everything under it. Rust and
  Python get `tests/*` right only because their star crosses slashes, which is also exactly why they
  get `docs/*.qmd.md` wrong.

## Library probe: npm and crates have a git-exact matcher, PyPI does not [[qmd73_finding_libraries: Finding]]

Measured on the same 38 answers, in a scratch directory outside the repository, so no manifest was
touched. This matters for the choice of approach in the questions finding.

- category: parser
- related_to: [[#qmd73_qmdcignore]]
- solution: Recommended is one small matcher written from git's rules in each language, mirrored line for line, with the matrix as its gate. The alternative is the two libraries where they exist and a hand-written Python port.

### Results [[qmd73_finding_libraries_results: text]]

- about: [[#qmd73_finding_libraries]]

| library | how it was called | differs from git |
| --- | --- | --- |
| npm `ignore` 7.0.10 | `ignore({ ignorecase: false }).add(lines).ignores(path)` | 0 of 38 |
| crate `ignore` 0.4.33 | `Gitignore::matched_path_or_any_parents` | 2 of 38 |
| crate `ignore` 0.4.33 | `matched` on each ancestor directory, then on the path | 0 of 38 |
| PyPI `pathspec` 1.1.1 | `GitIgnoreSpec.match_file(path)` | 2 of 38 |
| PyPI `pathspec` 1.1.1 | each ancestor as `dir/`, then the path | 1 of 38 |

Every miss is about negation. Without an ancestor loop, both the crate and `pathspec` let
`!keep/important.qmd.md` bring a file back from under an excluded `keep/`, which git refuses. With the
loop, `pathspec` still fails `keep/**` followed by `!keep/important.qmd.md`, because it compiles
`keep/` and `keep/**` to the same pattern and the two differ exactly when a negated line follows. So
Python has no drop-in library.

The npm package is case-insensitive by default: measured, `Tests/` then hides `tests/a.qmd.md` and
`docs/tests/d.qmd.md`. It needs `ignorecase: false` to agree with git and the other two parsers.

## What the repository root sees [[qmd73_finding_repo_root: Finding]]

At this repository's root Rust and Python scan the same 115 files, and TypeScript scans 941: the
same 115 plus 826 under `tests/`. The ignore divergence explains all 292 TypeScript errors. 286 are
in files under `tests/`. The other 6, in `docs/mcp/`, are ambiguous references caused by LSP fixtures
under `tests/` that give their workspace the id `docs`, which composes them into the real one.

The 2 Rust errors have a different cause, and they are not new: the Rust binary built from
`git archive HEAD` reports the same two. The file set is identical in all three parsers. Both errors
come from `qmdc-rs/src/qmdc-guide.qmd.md`, the copy of the guide embedded into the Rust binary, which
lies outside every workspace and declares `qmdc_guide` a second time. Rust reports the reference to
`qmdc_guide` in `docs/mcp/discovery.qmd.md` and `docs/mcp/resources.qmd.md` as ambiguous, while
Python and TypeScript resolve it to the object in `docs` and say nothing.

- category: parser
- related_to: [[#qmd73_qmdcignore]]
- solution: Goal C1 covers the scanned file set. The disagreement over a reference that matches both a workspace object and one in a file outside every workspace is about resolution, not ignoring, and gets its own task. Separately, the embedded copy is a build input rather than documentation, so the root `.qmdcignore` could list it.

### After the fix [[qmd73_finding_repo_root_after: text]]

- about: [[#qmd73_finding_repo_root]]

`workspace validate .` at the repository root now scans the same 118 files in all three parsers
(the 115 from triage plus this task's three documents). TypeScript reports 0 errors instead of 292
and Python 0 as before. Rust still reports the 2 `qmdc_guide` ambiguities, which are the separate
resolution divergence described above.

## Regression fixtures, red before the fix for their own reasons [[qmd73_finding_tests: Finding]]

Existing coverage: `tests/workspace/qmdcignore-test`, `empty-workspace-with-qmdcignore` and
`virtual-workspace-qmdcignore-issue` use only `name.qmd.md`, `dir/` and `**/name_*.qmd.md` at the
root, which all three parsers match the way git does. None of them reproduces this bug, so five
workspace fixtures were added under `tests/workspace/qmdcignore-gitignore/`, one per rule. Each
`_expected.json` file list is what git keeps, written by `artifacts/gen_fixtures.py`.

- category: testing
- related_to: [[#qmd73_qmdcignore]]
- affected_files: [tests/workspace/qmdcignore-gitignore/01-directory-star-hides-descendants, tests/workspace/qmdcignore-gitignore/02-directory-match-hides-contents, tests/workspace/qmdcignore-gitignore/03-star-stays-in-one-directory, tests/workspace/qmdcignore-gitignore/04-anchoring, tests/workspace/qmdcignore-gitignore/05-negation-re-includes]
- solution: Keep the five as the reported-shape gate, add fixture 06 (goal B2) and the matrix test (goal B3) during implementation, and require the whole suite green before done_review.

### Which parser is red on which fixture, and why [[qmd73_finding_tests_red: text]]

- about: [[#qmd73_finding_tests]]

| fixture | lines | red in | what the red parser does |
| --- | --- | --- | --- |
| 01 | `examples/*` | ts | keeps `examples/sub/mid.qmd.md` |
| 02 | `generated`, `examples/sub` | rs, py, ts | keeps both named directories' files |
| 03 | `docs/*.qmd.md` | rs, py | also hides `docs/sub/nested.qmd.md` |
| 04 | `build/`, `notes.qmd.md`, `/top.qmd.md` | rs, py, ts | rs and ts keep all three targets; py keeps `src/build/b.qmd.md` and `top.qmd.md` |
| 05 | `keep/*`, `!keep/important.qmd.md` | rs, py, ts | rs and py hide `keep/important.qmd.md`; ts hides every file, so no workspace is found |

Each red pair fails the `files` and `objects_by_kind` aspects of the workspace suite; TypeScript on
05 also fails `workspace_id`. Measured per fixture against each parser directly, then confirmed by
`make -k test`, where every failure is one of the rows above:

- Rust: 3 of 157 tests. `workspace_conformance` fails 8 cases (fixtures 02 to 05). The two
  `workspace_unit` tests `test_workspace_files` and `test_workspace_objects_by_kind` loop over the
  same fixtures and stop at the first mismatch, fixture 02.
- Python: 8 failed, 944 passed (fixtures 02 to 05).
- TypeScript: 9 of 175 workspace cases (fixtures 01, 02, 04 and 05). Its `npm test` chain stops
  there and skips `test-sql.ts`, which passes when run on its own.
- Everything else is green: `validate-docs`, `validate-compare`, parse parity (0 divergent of 117
  documents), `md-lint`, the guide budget, and the mkdocs, semantic and vscode suites.

### Test plan [[qmd73_finding_tests_plan: text]]

- about: [[#qmd73_finding_tests]]

- Now: fixtures 01 to 05, red as listed.
- During implementation: fixture 06, `keep/` then `!keep/important.qmd.md`, where git hides both. It
  passes today in Rust and Python only because they ignore the negated line, so it guards the new
  negation code, and `pathspec` is the measured example of an implementation that gets it wrong.
- During implementation: the matrix moves to `tests/` as data, and each language gets a short test
  that feeds its 38 forms to the ignore function directly. New test code, so it is listed here for
  approval with the triage.
- Verification: `make -k test` with Rust's `--no-fail-fast`, the per-fixture check against each
  parser, and `workspace validate .` at the repository root in all three (goal C1).

## Open questions [[qmd73_finding_questions: Finding]]

Three decisions for the operator before implementation.

- category: parser
- related_to: [[#qmd73_qmdcignore]]
- solution: Recommended answers are given with each question.

### Questions [[qmd73_finding_questions_list: text]]

- about: [[#qmd73_finding_questions]]

- **Q1: how to build the matcher.** (a) One small matcher from git's rules, written in each
  language and mirrored line for line: a line becomes a regex plus three flags (negated,
  directory-only, anchored), and a path is decided by checking its ancestor directories first, with
  the last matching line winning. No new dependency; `minimatch` and `globset` leave the parsers.
  (b) npm `ignore` and the `ignore` crate, both git-exact in the probe, plus a hand-written Python
  port. That adds two dependencies and keeps three engines, which is the structural cause of this
  bug. Recommended: (a).
- **Q2: which ignore files apply.** Measured, and the same in all three: a workspace reads the
  `.qmdcignore` in its own root, a container's file only steers workspace discovery (a container line
  naming a path inside a workspace has no effect), and a `.qmdcignore` in a subdirectory is not read.
  Git reads every level. Recommended: keep today's rule and document it (goal D1); moving to git's
  layering is a separate feature.
- **Q3: users will see different files.** After the fix some lines hide more and some hide fewer
  files, differently per parser (goal D2). That is a behaviour change under the still-unversioned
  `[Unreleased]`, so it joins the version decision left open by QMD-71 and QMD-72.

## Implementation: one port of git's matcher, mirrored in three languages [[qmd73_finding_port: Finding]]

Q1 was answered (a). Each parser got an `ignore` module holding a port of git's `dowild()`
from `wildmatch.c` and of `match_basename()`, `match_pathname()`, the line parsing and the
excluded-directory walk from `dir.c`. The three modules have the same functions in the same
order, work on UTF-8 bytes as git does, and match case-sensitively. `globset` leaves the Rust
parser and `minimatch` the TypeScript parser; Python drops `fnmatch`.

- category: parser
- related_to: [[#qmd73_qmdcignore]]
- affected_files: [qmdc-rs/src/ignore.rs, qmdc-py/qmdc/ignore.py, qmdc-ts/src/ignore.ts, qmdc-rs/src/workspace.rs, qmdc-rs/src/lsp/server.rs, qmdc-py/qmdc/workspace.py, qmdc-ts/src/workspace.ts]
- affected_functions: [dowild, match_pathname, parse_qmdcignore, is_ignored_relative, is_ignored]
- solution: Every scan asks `is_ignored(path, root, rules, is_dir)`, which checks each ancestor directory before the path. Walker entries pass their own type; known files pass false.

### Details and verification [[qmd73_finding_port_detail: text]]

- about: [[#qmd73_finding_port]]

- git's prefix step is ported too. `match_pathname()` compares a pattern's literal prefix before
  calling `dowild()`, and that changes the meaning of a `**` right after the prefix: git treats
  `te**/c.qmd.md` as `te` followed by a whole-segment `**`, so it matches `tests/sub/deep/c.qmd.md`.
  The matrix pins it (`literal-prefix-then-double-star`).
- The acceptance data grew from the triage's 38 forms to 79 in `tests/ignore/gitignore-matrix.json`,
  every answer generated by git 2.44.0 through `tests/ignore/gen_matrix.py`. The additions cover
  CRLF, a byte-order mark, escaped and unescaped trailing spaces, a significant leading space, a tab
  that is not trimmed, `\#`, bracket negation both ways, ranges, a POSIX class, escaped glob
  characters, and `?` matching one byte of a two-byte character.
- Five forms were added after review, because the first 74 did not pin the `**` rule: a matcher in
  which EVERY `**` crosses `/` passes all 74 of them. `double-star-not-followed-by-slash`
  (`tracking/**artifacts/*`) and `double-star-after-glob-segment` (`t*s/su**/c.qmd.md`) both fail
  that matcher, so the pair now holds both halves of the rule — the `**` must end the segment AND
  be reached with no earlier wildcard. `case-sensitive-file`, `trailing-space-on-negation` and
  `byte-order-mark-then-anchored` pin case, trimming on a negated line, and a byte-order mark
  in front of an anchored line.
- All three ports agree with git on all 79, and the report counts them as a new `ignore` suite
  with parity enforced: 79, 79, 79.
- The ancestor loop is what fixture 06 guards. With it disabled in Python, the matrix goes red on
  every directory form; with it disabled in Rust, fixture 06 keeps both files in `keep/`. Both
  measured, then restored.
- Two call-site bugs went with the helpers. Python decided whether a directory was ignored by
  asking about a made-up file `__dummy__` inside it. TypeScript's nested-workspace search skipped a
  whole subtree when only its `readme.qmd.md` was ignored. Both now ask about the directory itself.

## A `--with` path's `.qmdcignore` steered discovery in Rust only [[qmd73_finding_with_scan: Finding]]

Found while reading the call sites, then measured. A directory holding workspaces `a/` and `b/`,
with a `.qmdcignore` that hides `b/`: `workspace validate -w ./container` exited 0 in Rust and 2
in Python and TypeScript with "contains 2 workspaces". Rust's bounded scan pruned by the path's
own ignore file; the Python and TypeScript scans added in QMD-72 read none.

- category: parser
- related_to: [[#qmd73_qmdcignore]]
- affected_files: [qmdc-py/qmdc/workspace.py, qmdc-ts/src/workspace.ts, tests/cli/036-with-honours-qmdcignore]
- affected_functions: [find_workspace_dirs_bounded, find_all_workspace_dirs, scanWorkspaceDirs]
- solution: Both scans now load the root's rules and skip ignored readmes and directories, as the Rust scan does. `tests/cli/036-with-honours-qmdcignore` pins it with a query that must name exactly the visible workspace, so a result that loads nothing cannot pass.

## Noticed, measured, out of scope: a workspace nested in a nested workspace [[qmd73_finding_nested_in_nested: Finding]]

Three workspaces nested in a chain, each directory inside the previous one. Rust and TypeScript
report `nested_workspace` for both inner ones; Python reports only the first, because its scan
stops descending once it finds a nested workspace. The Python built from `git archive HEAD`
reports the same, so this is older than QMD-73 and not about ignoring.

- category: parser
- related_to: [[#qmd73_qmdcignore]]
- solution: A separate task, together with the `qmdc_guide` ambiguity from the repository-root finding.
