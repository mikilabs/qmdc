# QMD-61: Findings

Triage for standing up the public `qmdc` open-source repository. Current state
verified against the working tree on 2026-06-13; open questions resolved with the
operator answers already embedded in the task.

## Current-state verification [[qmd61_finding_state: Finding]]

What actually exists in the tree today, checked before planning so the seed is built on facts, not assumptions.

- category: docs
- related_to: [[#qmd61]]
- solution: baseline established; each item feeds a layer in [[#qmd61_finding_layers]]

### Verified facts [[qmd61_state_facts: text]]

- Top-level dirs present: `dist-release/`, `docs2/`, `method_iterations/`, `org-ai-kb/`, `presentations/`, `qmdc-py/`, `qmdc-ts/`, `qmdc-rs/`, `qmdc-semantic/`, `qmdc-mkdocs/`, `qmdc-vscode/`, `reviews/`, `scripts/`, `tasks/`, `zold_docs/`.
- No `.github/` directory; no `CONTRIBUTING.md` / `CODE_OF_CONDUCT.md` / `CHANGELOG.md` / `SECURITY.md`. All community/CI files must be created from scratch.
- `.gitignore` ends with `**/.qmdc-semantic/embeddings.db` and `**/.qmdc-semantic/hints.json` (the "lines 25-26" the task flags — content confirmed). These contradict the commit-the-artifact model.
- `.gitattributes` routes only `*.db` through Git LFS. `hints.json` is plain (≈116 KB) and is NOT LFS-tracked.
- `git lfs ls-files` tracks `docs2/.qmdc-semantic/embeddings.db` (≈93 MB smudged working file) and `zold_docs/.qmdc-semantic/embeddings.db`. Only the `zold_docs` one leaves (its parent is excluded).
- `docs2/mkdocs.yml` has no `site_url`.
- `README.md` is the QMD-60 state: English "Getting Started" + package table, but the body is still Russian and shows the old `qmd-py`/`qmd-ts`/`qmd-rs` wrapper invocations. Needs the OSS English rewrite.
- `LICENSE` present (AGPL, from QMD-60) — verify only.
- `tasks/` holds the live parser fixtures (see [[#qmd61_finding_fixtures]]).

## Open questions resolved [[qmd61_finding_open: Finding]]

The four task open-questions, with the operator answers and the consequences triage found.

- category: docs
- related_to: [[#qmd61]]
- solution: all four answered; five secondary decisions surfaced for the checkpoint

### Resolutions [[qmd61_open_resolutions: text]]

1. **`tasks/` fixtures** → relocate (operator: "a"). Caveat: the task says "arguably its own task QMD-62", but **QMD-62 is already taken** (a `done/` task + `tasks/QMD-62/artifacts/envelope-tests`); a split-out would be QMD-63. Treated here as the highest-risk layer [[#qmd61_finding_fixtures]].
2. **Keep `docs2/tracking/` public?** → keep, after housekeeping (operator: analyse, decline/relocate dead or done-differently tasks, check in the rest). See [[#qmd61_finding_housekeeping]].
3. **Pages URL** → custom domain `qmdc.mikilabs.io`, root-served (no `/qmdc/` subpath, no redirect). `site_url = https://qmdc.mikilabs.io/`. Local `mkdocs serve` / `make site` keep working unchanged; the domain only applies on deploy. Operator adds the DNS `CNAME` record and the GitHub Pages custom-domain setting.
4. **License** → AGPL (delivered by QMD-60; verify present).

### Secondary decisions (resolved) [[qmd61_open_decisions: text]]

- D1 → **relocate inside QMD-61**, into a purpose-organized top-level `tests/` tree (not by task id). Layout: `tests/<purpose>/<case-folders>` (e.g. `tests/parser/`, `tests/workspace/`, `tests/lsp/`, `tests/sql/`, `tests/envelope/`). See [[#qmd61_finding_fixtures]].
- D2 → **confirmed**: ship with `docs/`; rename `docs2/` → `docs/` ([[#qmd61_finding_docs_rename]]).
- D3 → **commit `hints.json` plain** into the repo; remove its ignore line (do NOT add an LFS rule for it).
- D4 → **keep `.kiro/steering/` public**; exclude only `.kiro/{agents,hooks,skills,specs,aidlc-common}`.
- D5 → superseded by D1: a single repo-root `tests/` tree organized by purpose, shared by all parsers.

## Orphan-branch seed + exclusion list [[qmd61_finding_seed: Finding]]

How the public history is created and exactly which tree it contains.

- category: docs
- related_to: [[#qmd61]]
- solution: agent curates the tree + writes the exact command sequence; OPERATOR runs all mutating git

### Mechanism and tree [[qmd61_seed_detail: text]]

A plain branch still points into the existing graph, so it is not clean. Use an orphan branch (`git checkout --orphan public-main`) → stage only the curated public tree → single root commit → push as `main` to the new `mikilabs/qmdc` repo. This repo stays the private working repo (full history, tracking, internal dirs); nothing is archived.

Excluded from the seed (stay private):

- `zold_docs/`, `presentations/`, `reviews/`, `org-ai-kb/`, `method_iterations/`.
- `.playwright-mcp/` (already gitignored), `dist-release/` (build output).
- internal `.kiro/` subdirs (`agents/`, `hooks/`, `skills/`, `specs/`, `aidlc-common/`). **`.kiro/steering/` is KEPT public** (D4 — it documents the QMD format).
- non-fixture `tasks/` artifacts (plans, throwaway scripts, `.bak`).

Ships: the six `qmdc-*` packages, `docs/` (renamed from `docs2/`, minus excluded `tracking/` entries), the relocated fixtures, root tooling (`Makefile`, `setup.sh`, the `qmdc`/`qmdc-*` wrappers, `README.md`, `LICENSE`, `.gitignore`, `.gitattributes`, `.markdownlint.json`, `.markdownlintignore`), the new community/CI files, and the semantic LFS artifacts.

Pre-seed verification gate: grep the curated tree for any legacy internal codenames; scan for secrets/PII; confirm LFS pointers resolve. The commit must be clean.

## `docs2/` → `docs/` rename [[qmd61_finding_docs_rename: Finding]]

The task asks to rename `docs2/` to `docs/` and leave no dangling references.

- category: docs
- related_to: [[#qmd61]]
- solution: rename folder via `mv`, then sweep every `docs2` reference; gate on `make test` + site build
- status: done

### Outcome [[qmd61_docs_rename_outcome: text]]

Done. `docs2/` → `docs/` (folder + workspace id `docs2`→`docs` + title "QMDC Documentation"); all path/prose references swept across Makefile, configs, parser sources, and tracking. `make test` green (parser 600/600/600, workspace 135/135/135, sql 59/59/59, cli 10/10/10, all parity ✓; 3106 cases, 0 failures); `make site WS=./docs` builds (75 pages); `validate-docs` clean. Semantic reindex deferred to the operator — `docs/.qmdc-semantic/` still keys on old `docs2:` ids until then.

Folded in: the ignore-file convention `.qmdignore` → `.qmdcignore` (no backward compat) — renamed files, parser read-sites + identifiers (`load_qmdcignore`, `qmdcignore_path`/`qmdcignorePath`), 3 fixture dirs + their `expected.json` ids, `by_smart.rs` label, live docs, and all tracking except `old/`. `zold_docs/` keeps `.qmdignore` (seed-excluded).

### References to update [[qmd61_docs_rename_detail: text]]

At least: `Makefile` (`validate-docs` uses `docs2`; semantic targets `WS=./docs2`; `site`/`site-build` defaults), `docs2/mkdocs.yml`, `.qmdc-mkdocs.ignore`, `.qmdcignore`, `scripts/*`, any steering `fileMatchPattern`, and this `tracking/` tree's own internal references. A repo-wide grep for `docs2` is the completion check. Note the LFS-tracked `docs2/.qmdc-semantic/embeddings.db` path changes — re-confirm LFS still tracks it after the move.

## LFS + semantic-artifact commit workflow [[qmd61_finding_lfs: Finding]]

The semantic DB + hints are first-class committed artifacts, precomputed before release so the Pages build needs no embedding provider.

- category: docs
- related_to: [[#qmd61]]
- solution: drop the contradictory ignores, settle hints storage, add a regenerate-before-release step, document LFS

### Work [[qmd61_lfs_detail: text]]

- Remove the two `.gitignore` lines (`**/.qmdc-semantic/embeddings.db`, `**/.qmdc-semantic/hints.json`) so a re-index actually stages.
- `hints.json`: commit **plain** (D3) — it is small, diffable JSON; no LFS rule for it. `embeddings.db` stays LFS-tracked via the existing `*.db` rule.
- Add/confirm a release step to regenerate both before a release and commit the updated LFS DB + hints. Verified: `make semantic-index` (Makefile:635) and `make semantic-hints` (Makefile:643) both exist, alongside `semantic-audit`/`semantic-test` — no new target needed, just sequence them into the release flow.
- Document `git lfs install` for contributors; CI checkout must fetch LFS objects.

## Unified test reporting (prerequisite to relocation) [[qmd61_finding_test_reporting: Finding]]

`make test` chains five harnesses in three output dialects; counts are scattered and some tests report none, so "nothing was silently skipped" cannot be asserted. This must be fixed before the fixture relocation, not after.

- category: testing
- related_to: [[#qmd61]]
- solution: emit a uniform machine-readable summary per suite (JUnit XML), aggregate to one report, and make every data-driven runner fail on zero-discovered

### Current dialects + baseline counts [[qmd61_reporting_baseline: text]]

Captured from a green `make test` on 2026-06-13, now enforced as floors in `scripts/test-baseline.json` and checked by the aggregator (`scripts/test-report.py`):

| Suite | Harness | run | passed | skipped |
| --- | --- | --- | --- | --- |
| py | pytest | 814 | 773 | 41 |
| ts-microtests | tsx | 596 | 556 | 40 |
| ts-workspace | tsx | 134 | 134 | 0 |
| ts-sqlworkspace | tsx | 59 | 59 | 0 |
| ts-cli | tsx | 9 | 9 | 0 |
| ts-codefences | tsx | 4 | 4 | 0 |
| rs | cargo nextest | 145 | 145 | 0 |
| mkdocs | pytest | 346 | 345 | 1 |
| semantic | pytest | 41 | 41 | 0 |

Unified total: **2148 run, 2066 passed, 82 skipped, 0 failed** across 9 suites. semantic additionally `deselected` 58 provider-gated tests (absent from JUnit by design). The TS suites are now five JUnit reports instead of scattered console lines.

### The silent-skip risk [[qmd61_reporting_risk: text]]

Each data-driven runner iterates a fixture directory found at a hard-coded path. If a path is mispointed — exactly what relocating ~20 constants risks — the loop runs zero iterations and the suite passes **vacuously**: green with nothing tested. The TS runners exit non-zero only on `failed>0` (not on `discovered==0`); a pytest file or a Rust data-driven test that collects no cases also passes empty. This is the precise mechanism by which the relocation could drop coverage without anyone noticing.

### Proposed unification [[qmd61_reporting_plan: text]]

- JUnit XML as the common contract: pytest `--junit-xml=` (py, mkdocs, semantic); `cargo nextest` native JUnit (`[profile.ci.junit] path=`); a shared reporter helper for the TS runners that writes JUnit and exits non-zero when `discovered==0`.
- Aggregator (`scripts/test-report.*`, wired into `make test`): read every JUnit file → one table (suite | passed | skipped | failed | discovered) + grand totals; FAIL if any suite reports `discovered==0` or a skip outside a known allowlist (semantic's 58 provider-gated; the ~40/41 platform skips).
- Anti-vacuous guard inside each data-driven runner: assert `discovered >= expected`, pinning a per-suite baseline count; fail loudly otherwise. This is the actual safety net for the move.

### Sequencing [[qmd61_reporting_seq: text]]

Lands as **L0.5**, before the fixture relocation (L2). The relocation gate becomes: the unified report shows identical per-suite `discovered` + `passed` counts before and after, with zero unexplained skips.

### Final granularity (locked) [[qmd61_reporting_granularity: text]]

Decided with the operator. The report groups by **feature suite**, not by language, and distinguishes shared-corpus *conformance* suites (must match across parsers) from impl-specific *unit* tests.

Canonical conformance suites (rows), columns = `py | ts | rs`:

- `parser` — the parse/rebuild microtest corpus. **code-fences is merged in** (no separate suite).
- `workspace` — workspace discovery/resolution corpus.
- `sql` — SQL query/rewrite corpus.
- `cli` — CLI behaviour. **Promoted to a data-driven conformance suite**: a shared `tests/cli/` fixture corpus that all three parsers iterate, so parity holds by construction.
- `lsp`, `mcp` — Rust-only by design (LSP server is Rust; py/ts have no LSP). Single-column rows, not gaps.

Unit/component suites (counts naturally differ, no parity): `unit-py`, `unit-rs` (lib + core/index-seam/utf16/did-save/etc.), `mkdocs`, `semantic`.

Rules:

- **Parity is a hard fail**: for a multi-language conformance suite, unequal case counts across `py/ts/rs` fail `make test`.
- Granularity is **per fixture case** in every language. Rust's data-driven tests today report per *function* (e.g. all 203 parser cases = 3 nextest tests; ~100 LSP cases = 1); they are converted to emit **per-case** JUnit via a shared Rust reporter (`tests/common/`, mirroring `qmdc-ts/tests/_report.ts`), tagged with the canonical suite.
- The aggregator maps each JUnit testcase → canonical suite (by python module / ts suite name / rust binary), builds the matrix, and enforces parity on a staged allowlist (start with `parser`; add `workspace`, `sql`, `cli` as each is reconciled to true parity).

Sub-steps (all gated by green `make test`):

- L0.5a — Rust per-case reporter + aggregator feature-matrix + code-fences→parser + enforce `parser` parity.
- L0.5b — reconcile `workspace` + `sql` to parity (separate shared-corpus cases from impl-unit), enforce.
- L0.5c — CLI conformance as a **data-driven shared corpus**: define `tests/cli/<case>/` fixtures (command + input + expected stdout/exit), and give each parser a thin runner that iterates them. Same fixtures → identical counts → `cli` parity by construction (like `parser`/`sql`), rather than hand-authoring matching tests per language. Then enforce `cli` parity.

### Progress snapshot (L0.5a/b) [[qmd61_reporting_progress: text]]

Done and green:

- Per-case reporting infra: shared Rust reporter `qmdc-rs/tests/common/mod.rs` + TS `_report.ts`; feature-matrix aggregator `scripts/test-report.py` (maps each JUnit case → suite × language; excludes now-canonical Rust binaries from the nextest report to avoid double-count); config `scripts/test-baseline.json` (`enforce_parity`, `require_present`).
- **`parser` reconciled to true parity and ENFORCED: 600 = 600 = 600** across py/ts/rs. Root cause of the earlier 397: the Rust `test_all_microtests_rebuild_text` (203 cases) was not wired; parser = parse 214 + rebuild 179 + text 203 + code-fences 4. code-fences merged into `parser`.

Remaining (each its own chunk; counts are current matrix):

- ~~`cli`~~ — **DONE**: built the data-driven shared corpus `tests/cli/` (6 cases: parse stdin/file/minimal/full, workspace validate clean/broken) with a thin runner in each language (`cli_conformance.rs`, `test_cli_conformance.py`, `test-cli-conformance.ts`); old hand-written CLI tests remapped to `unit-*`. Enforced at 6/6/6.
- ~~`lsp` / `mcp`~~ — **DONE**: wired to per-case (`lsp 115`, `mcp 34`); Rust-only, no parity.

**All four conformance suites (`parser`, `sql`, `workspace`, `cli`) are now enforced at parity.** `enforce_parity` = all four; a mismatch fails `make test`. L0.5 complete.

### Adversarial review outcome [[qmd61_reporting_review: text]]

Three cold/adversarial sub-agent reviews of the staged L0.5 changes (parser fixes, runners, aggregator, fixtures, CLI corpus). The four core parser fixes were verified correct and convergent (py/ts/rs identical; spec fixture re-order confirmed set-identical; line numbers manually verified against source). Issues found and **fixed**:

- **Silent-skip holes**: `mcp_fixture_tests` and `test_qmd7_parser_microtests` had an early `return` before their per-case report was created — an empty/mispointed fixture dir (both are nextest-excluded canonical tests) would drop the suite and still pass. Changed both to `assert!(!tests.is_empty())` (fail loudly).
- **Stale reports**: `test-reports/` was never purged, so a renamed/removed report could leave a phantom count. Added `reports-clean` as a prerequisite of every suite (runs once before any writes).
- **Unmapped reports**: the aggregator silently ignored a JUnit file matching no suite pattern; added an else branch that fails on an unrecognized report stem.

Accepted as follow-ups (not blocking; documented):

- Parity is **count-based**, not case-identity-based (holds by construction today). Array-field reference lines in py/ts are still searched from file top (string/comment branches fixed; consistent py↔ts, latent vs rust). `workspace_conformance` `objects_by_kind` is slightly more lenient than py on malformed input. CLI corpus covers parse + validate only — `rebuild`/`query`/`-o`/non-trivial `--format full` and error-exit cases are coverage gaps for a later expansion.

### Real bugs surfaced by the parity gate [[qmd61_reporting_bugs: text]]

Enforcing per-case parity uncovered genuine cross-parser defects (all fixed; `parser`/`sql`/`workspace` now enforced at 600/59/135):

- **Rust** dropped the `[[ ]]` brackets from `broken_link` / `ambiguous_reference` / `ambiguous_field_reference` error `reference` fields (4 sites in `workspace.rs`); every other parser + fixture uses the bracketed form. Fixed.
- **Python + TypeScript** attributed every reference to the *first* occurrence of that target in the file (e.g. all `[[#user]]` → line 13) instead of the actual per-object occurrence line, because the reference-line search restarted at the file top for each field. Fixed both to search from the object's own line and advance monotonically. Rust was already correct.
- **Python + TypeScript** emitted `duplicate_id` candidates as bare filenames; the intended form is `file:line` (Rust's). Fixed py + ts.
- Two stale fixtures (`spec`, `validation-parser-consistency`) carried the buggy expectations; regenerated from the corrected output (py == rs verified).

## Fixture relocation (highest risk) [[qmd61_finding_fixtures: Finding]]

The parser tests read fixtures from `tasks/`; a public layout wants them under a clean fixtures dir.

- category: testing
- related_to: [[#qmd61]]
- solution: move fixtures into a purpose-organized repo-root `tests/` tree, update every path constant, gate on the unified test report ([[#qmd61_finding_test_reporting]]) showing identical per-suite discovered+passed counts before/after
- status: done

### Outcome [[qmd61_fixtures_outcome: text]]

Done in two staged, separately-gated chunks.

Stage 1 (relocation): `tasks/QMD-*/artifacts/...` → repo-root `tests/{parser,workspace,sql,lsp,mcp}` (cli already there). `QMD-5/microtests` + the `QMD-7/artifacts` workspace dirs merged into a single `tests/workspace/` — safe because suite discovery is marker-based (conformance keys on `_expected.json`, sql on `tests/*.sql`, tree-modes on `tests/tree-modes/`) and there are no name collisions; the three SQL scan-roots collapsed to one path. Code-fences folded into `tests/parser/` (filling the free 071–073 slots) and the three dedicated fence harnesses deleted — they parse identically (`parse()` defaults to `random_seed=666`). Root-finders re-anchored from `tasks/` to the `qmdc-rs/` marker; `BENCH_DIR` → `docs`.

Stage 2 (uniform test-file names): dropped task-ids and aligned the three languages. `microtests`→`parser`, `sql_workspace`/`sqlite_workspace`→`sql`, `qmd7_microtests` deleted; Rust `cli.rs`→`cli_unit.rs`, `workspace.rs`→`workspace_unit.rs`, `lsp_microtests`→`lsp`, `lsp_simple`→`lsp_smoke`, `mcp_tests`→`mcp`, `mcp_force_root_tests`→`mcp_force_root`, `sql_rewrite_tests`→`sql_rewrite`, `test_tree_modes_workspace`→`tree_modes`, and the `qmd58_*`/`qmd35_*`/`qmd_namespaced_*`/`test_*` regressions → descriptive `lsp_*`/`line_numbers`/`utf16_positions`/`core_index_seam`. `scripts/test-report.py` mapping tables (py_suite, RS_BINARY_SUITE, RS_CANONICAL_TESTS, TS_SUITE), the TS test script + report stems, and the `docs/testing/` suite docs all updated in lockstep.

Gate: `make test` green both stages — matrix parser 604 / workspace 135 / sql 59 / cli 10 (parity ✓), lsp 115, mcp 34, unit-rs 129; parser rose 600→604 because the 4 code-fence cases now also run round-trip-text in all three languages (parity preserved). `tasks/` no longer referenced by any test code (`grep -r tasks/QMD */tests` empty).

### Target layout (by purpose) [[qmd61_fixtures_layout: text]]

A clean repo-root `tests/` tree organized by purpose, not by task id (verified: no repo-root `tests/` exists today, so it is a clean destination — distinct from the per-package `qmdc-*/tests/` code dirs). Proposed mapping (final names settled during implementation, gated by green tests):

- `tests/parser/` ← `QMD-4/artifacts/microtests` + `QMD-7/artifacts/parser-microtests`
- `tests/workspace/` ← `QMD-5/artifacts/microtests` + `QMD-7/artifacts` workspace cases (incl. `test-workspace`)
- `tests/sql/` ← `QMD-24/artifacts/{sql-rewrite-tests,multi-workspace-isolation}`
- `tests/lsp/` ← `QMD-6/artifacts/lsp-microtests` + `QMD-24/artifacts/lsp-sql-tests`
- `tests/mcp/` ← `QMD-62/artifacts/envelope-tests` (focused MCP tests, consumed by `qmdc-rs/tests/mcp_tests.rs`)

Care points: `QMD-7/artifacts` is verified to hold ~20 workspace fixture dirs (`multi-workspace*`, `nested-*`, `virtual-workspace*`, `workspace-*`, `test-workspace*`, `typed-edges`, `qmdcignore-test`, …) plus `parser-microtests`. Two suites scan it wholesale as a `SCAN_PATHS` root (`test_sql_workspace.py`, `sqlite_workspace.rs`) and discover every workspace inside. So the split must send all those workspace dirs to `tests/workspace/` (the new scan root) and only `parser-microtests` to `tests/parser/` — verify discovered-workspace counts match before/after. `QMD-5/artifacts/microtests` is the other scan root.

### Full `tasks/` → `tests/` mapping [[qmd61_fixtures_mapping: text]]

Directory-level mapping of the entire `tasks/` tree (leaf case-folders omitted). Everything under each source dir moves as a unit.

| `tasks/` source (dir) | Consumed by | Destination |
| --- | --- | --- |
| `QMD-4/artifacts/microtests` | py + ts + rs parser microtests | `tests/parser/` |
| `QMD-7/artifacts/parser-microtests` | py + ts + rs code-fence tests | `tests/parser/` |
| `QMD-5/artifacts/microtests` | py + ts + rs workspace + sql-workspace scan | `tests/workspace/` |
| `QMD-7/artifacts/*` (19 workspace dirs, all but `parser-microtests`) | py + ts + rs workspace + sql-workspace + tree-mode scans | `tests/workspace/` |
| `QMD-24/artifacts/sql-rewrite-tests` | rs SQL-rewrite | `tests/sql/` |
| `QMD-24/artifacts/multi-workspace-isolation` | rs SQL-rewrite (multi-workspace) | `tests/sql/` |
| `QMD-24/artifacts/lsp-sql-tests` | rs lsp + mcp | `tests/lsp/` |
| `QMD-6/artifacts/lsp-microtests` | rs lsp + mcp | `tests/lsp/` |
| `QMD-62/artifacts/envelope-tests` | rs mcp | `tests/mcp/` |
| `QMD-2/artifacts` | nothing live (demo YAML + compare scripts; gitignored) | EXCLUDE (not shipped) |
| `QMD-4/artifacts/examples` | nothing live | EXCLUDE (not shipped) |
| `QMD-6/artifacts/docs` | nothing live | EXCLUDE (not shipped) |

The 19 `QMD-7/artifacts/*` workspace dirs (→ `tests/workspace/`): `empty-workspace-with-qmdcignore`, `mixed-workspaces`, `multi-workspace`, `multi-workspace-collision`, `multi-workspace-tree-isolation`, `nested-workspace-test`, `nested-ws-test`, `qmdcignore-test`, `test-workspace`, `test-workspace-parent-issue`, `test-workspace-smart`, `text-field-code-block`, `typed-edges`, `virtual-workspace`, `virtual-workspace-qmdcignore-issue`, `workspace-deep-nesting`, `workspace-invalid-file`, `workspace-mixed-namespaces`, `workspace-no-namespaces` (plus the loose `queries.qmd.md` file).

The three EXCLUDE rows are referenced only from `reviews/`, `org-ai-kb/`, and `zold_docs/` (themselves excluded from the seed), never from live test code — verified by repo-wide grep. `QMD-5/artifacts/microtests` and the `QMD-7/artifacts` workspace dirs both land in `tests/workspace/`, merging the two scan roots into one — verify no case-name collisions and that discovered-workspace counts match before/after.

### Live fixtures [[qmd61_fixtures_live: text]]

The only `tasks/` content tests touch: `QMD-4/artifacts/microtests`, `QMD-5/artifacts/microtests`, `QMD-6/artifacts/lsp-microtests`, `QMD-7/artifacts` (incl. `parser-microtests`, `test-workspace`), `QMD-24/artifacts/{sql-rewrite-tests,multi-workspace-isolation,lsp-sql-tests}`, `QMD-62/artifacts/envelope-tests`.

### Path constants to update [[qmd61_fixtures_paths: text]]

~20 constants across all three parsers:

- Python: `test_microtests.py`, `test_workspace.py`, `test_sql_workspace.py`, `test_cli.py`, `test_code_fences.py`, `debug_edges.py`.
- Rust: `cli.rs`, `sql_rewrite_tests.rs`, `sqlite_workspace.rs`, `qmd7_microtests.rs`, `workspace.rs`, `mcp_tests.rs`, `lsp_microtests.rs`, `microtests.rs`, `test_tree_modes_workspace.rs`.
- TypeScript: `test-code-fences.ts`, `test-cli.ts`, `test-sql-workspace.ts`, `test-workspace.ts`, `test-microtests.ts`, `debug-edges.ts`.

Risk: paths are hard-coded as `parent.parent.parent / "tasks/QMD-N/..."` (and `../../tasks/...` in TS). Moving requires touching every constant in lockstep and re-running the full sequential `make test` (parallel `test-fast` has an install race). Each constant repoints to its `tests/<purpose>/...` home.

## README rewrite + community/health files [[qmd61_finding_readme_community: Finding]]

OSS-facing front door.

- category: docs
- related_to: [[#qmd61]]
- solution: rewrite README in English for an OSS audience; add the standard community/health set

### Work [[qmd61_readme_detail: text]]

Rewrite `README.md` (what-is-QMD hook, install via `uvx`/`npx`/`cargo`/Marketplace, 60-second quickstart, docs-site + spec + contributing links, CI/PyPI/npm/crates/license badges, short three-parsers+LSP+SSG blurb). Add `CONTRIBUTING.md` (build via `make init`, `make test`, the data-driven test convention, PR norms, `git lfs install`), `CODE_OF_CONDUCT.md` (Contributor Covenant), `CHANGELOG.md` (seed 1.0.0), `SECURITY.md`, `.github/ISSUE_TEMPLATE/{bug,feature}`, `.github/PULL_REQUEST_TEMPLATE.md`. Verify `LICENSE` present.

## CI workflows + GitHub Pages [[qmd61_finding_ci_pages: Finding]]

Automation layer; plugs into the QMD-60 publish scripts.

- category: docs
- related_to: [[#qmd61]]
- solution: three workflows (CI, release, pages) + `site_url` + custom domain; builds hermetic via committed hints
- status: done

### Work [[qmd61_ci_detail: text]]

- `.github/workflows/ci.yml`: on push/PR, run the `make test` equivalent across Python, TypeScript, Rust, mkdocs, plus lint; OS matrix at least for the Rust build.
- `.github/workflows/release.yml`: on package-prefixed tags (`qmdc-py-v*`, `qmdc-rs-v*`, …), build per-platform artifacts and publish via the QMD-60 publish scripts using registry tokens from GitHub Secrets; honour the binary-cascade from `RELEASING.md`.
- `.github/workflows/pages.yml`: build the site (`make site WS=./docs` post-rename) and deploy to Pages on push to `main`; checkout must fetch LFS.
- `docs/mkdocs.yml`: add `site_url: https://qmdc.mikilabs.io/`; root-served (no base-path/subpath needed).
- Custom domain: add a `CNAME` file (`qmdc.mikilabs.io`) to the published site (`docs/CNAME` so mkdocs copies it, or set it in the Pages action). Operator adds the DNS `CNAME` record (`qmdc.mikilabs.io` → `mikilabs.github.io`) and sets the custom domain in the repo's Pages settings.
- Local: `mkdocs serve` / `make site` keep working at `localhost` regardless of `site_url`; the domain only affects the deployed canonical links/sitemap.
- Hermetic build: the `qmd` mkdocs plugin needs the Python parser installed; `hints.json` is precomputed/committed so no embedding provider is needed at build time; LFS fetch is required.

### Outcome [[qmd61_ci_outcome: text]]

Done — but the shape differs from the original three-workflow plan:

- `.github/workflows/ci.yml` — runs on push/PR and is `workflow_call`-able. Jobs: **binary** matrix on `ubuntu-latest` + `macos-latest` + `windows-latest` (clippy, `cargo test` for parser/LSP/MCP, and a `qmdc parse` CLI smoke per OS — the guard against shipping a binary/MCP broken on Windows); plus `python` (`make py-test`), `typescript` (`make ts-test`), `docs` (`make validate-docs md-lint`) on ubuntu.
- `.github/workflows/release.yml` — trigger `v*` (+ `workflow_dispatch`); `test` job reuses `ci.yml` via `uses:`, `publish` job `needs: test` and runs `make publish` on `macos-latest` with tokens from Secrets. **One tag, idempotent all-registry publish** — not package-prefixed tags, not the QMD-60 per-package scripts.
- **GitHub Pages was superseded by Cloudflare Workers.** Docs ship via the existing `deploy-docs.yml` (wrangler, assets-only Worker on `qmdc.mikilabs.io`) and `make site-deploy` (strict build, warnings fatal). `site_url` is set in `docs/mkdocs.yml`. No `/qmdc/` subpath, no Pages CNAME.

See [[#qmd61_finding_publish]] for the publish mechanism and the first-release status.

## Publish pipeline + first release [[qmd61_finding_publish: Finding]]

The idempotent multi-registry publisher and the bootstrap publish run.

- category: docs
- related_to: [[#qmd61]]
- solution: one build-all + upload-all path, idempotent per registry; bootstrap run surfaced and fixed a real npm defect
- status: in_progress

### Mechanism [[qmd61_publish_mechanism: text]]

`make publish` = `scripts/release-build.sh` (full platform matrix → `dist-release/`) then `scripts/release-publish.sh --publish` (idempotent upload of everything). `make publish-check` is the dry-run. Per-registry idempotency: PyPI `twine --skip-existing`; npm `npm view <pkg>@<ver>` pre-check (platform packages before the main launcher); crates.io version pre-check via the API; vscode `--skip-duplicate` on both Marketplace and Open VSX. Registry selection: `release-publish.sh [--publish] [pypi|npm|crate|vscode ...]`. Tokens load from `.env.publish` (gitignored; template `.env.publish.example`); CI uses Secrets of the same names.

### npm launcher pin fix [[qmd61_publish_npm_fix: text]]

The bootstrap run exposed a real defect: the main `qmdc` npm launcher pinned `optionalDependencies` to a stale `1.0.0`, while the `@qmdc/cli-*` platform packages are versioned by the crate (`1.0.4`) — so `npm i qmdc` would have resolved no binary and been uninstallable. Fixed permanently in `release-build.sh`: at main-launcher pack time it rewrites every `optionalDependencies` entry to the built cli version (backup/restore, source stays clean), so launcher and platform packages always match.

### First-release status [[qmd61_publish_status: text]]

- ✅ PyPI — `qmdc 1.0.3` (7 platform wheels), `qmdc-mkdocs 1.0.0`, `qmdc-semantic 1.0.0`.
- ✅ npm — `@qmdc/cli-*@1.0.4` (7 platform packages); npm org `@qmdc` created.
- ✅ crates.io — `qmdc 1.0.4` (needed a verified account email).
- ✅ Open VSX — `qmdc-vscode 1.0.6` (6 platforms); namespace `mikilabs` auto-created.
- ✅ npm main launcher — renamed to scoped `@qmdc/qmdc`. npm's name-similarity filter permanently rejected the unscoped `qmdc` (`too similar to rfdc/cmdk/md5`) and support declined an exception, so the launcher now ships under the `@qmdc` scope already owned for `@qmdc/cli-*`. Users run `npx @qmdc/qmdc`; the `bin` command stays `qmdc`. Publish token must bypass 2FA (Automation/Granular).
- ⏳ VS Code Marketplace — pending `VSCE_PAT` (Azure DevOps PAT, publisher `mikilabs`).

Note: packages version **independently**; crate `1.0.4` ≠ py/ts `1.0.3` ≠ vscode `1.0.6` is expected.

## Housekeeping [[qmd61_finding_housekeeping: Finding]]

Operator wants the tracking tree and top-level dirs tidied before going public.

- category: docs
- related_to: [[#qmd61]]
- solution: add a `declined/` tracking folder, audit stale tasks, keep legit backlog, verify exclusions

### Work [[qmd61_housekeeping_detail: text]]

Add a `declined/` folder to the tracking workflow (update `workflow.sop.qmd.md` state notes); audit `planned/` — `QMD-50` (Bug: "YAML pipe field eats subsequent headings", real, keep) and `QMD-51` (Feature: graph-aware TOC, real, keep); move only genuinely-dead or done-differently tasks to `declined/`. Confirm the top-level exclusion list from [[#qmd61_finding_seed]] and the `.kiro` per-subdir call (D4). GitHub repo metadata (description, topics `markdown`/`knowledge-graph`/`lsp`/`rust`/`python`/`typescript`, social preview, branch protection on `main`) is operator-run in the GitHub UI.

## Parity-hardening follow-ups (next chunk) [[qmd61_finding_parity_followups: Finding]]

Non-blocking items surfaced by the adversarial review of L0.5. Tracked as a discrete follow-up chunk (proposed **L0.6**, before the L1 docs rename). None affect the current green gate.

- category: testing
- related_to: [[#qmd61]]
- solution: harden the parity guarantee and broaden CLI coverage per the checklist below

### Subtask checklist [[qmd61_followups_checklist: text]]

Done in L0.6:

- [x] **Expanded the CLI corpus** to cover every common command: `parse` (stdin/file/`--format`/`--no-pretty`/`--no-comments`), `rebuild`, `query` (via a `Query`-object ref to avoid arg spaces), `workspace validate` — now 10/10/10. `workspace parse` omitted (machine-dependent absolute `root`; covered by the workspace suite).
- [x] **Removed the `lint` command** (py + ts): it was just a chained `parse | rebuild` and was Python/TypeScript-only (spec `commands.qmd.md` already documented it as non-Rust). The CLI doc now describes formatting via `qmdc parse … | qmdc rebuild`; READMEs + guide updated. This makes the common command set identical across all three parsers (`parse`, `rebuild`, `query`, `workspace parse/validate`).
- [x] **Fixed a real `rebuild` divergence**: ts emitted an extra trailing blank line (`console.log` double-newline) vs rs/py; aligned ts to `process.stdout.write` so `rebuild` is now byte-identical across all three.
- [x] **Tightened `workspace_conformance.rs` `objects_by_kind`** to group every object (incl. empty kind/id), matching Python.
- [x] **Aligned `errors: null` handling** — rust + py now skip on absent-or-null (ts already did).

Deferred (bigger than a tweak — rationale below):

- [ ] **Case-identity parity** — currently holds *by construction* (cli/workspace/sql iterate identical fixture dirs; parser enumerates the same QMD-4/QMD-7 with identical format logic). An explicit case-id-set diff needs a unified cross-language case-naming scheme (TS reports synthesize generic names; py parser names collide across its 3 sub-functions). Real but narrow value (catches enumeration drift at equal count); a normalization project of its own.
- [ ] **Array-field reference-line anchoring (py/ts)** — attempted via a shared monotonic cursor; it DROPPED refs in `spec` (an object's `depends_on` mis-attributed) and was reverted. Root cause is architectural: Rust extracts simple/array-field refs line-accurately *during parsing* and only heuristically searches multiline-text fields, whereas py/ts do all extraction via the post-hoc search. Correct fix = restructure py/ts to extract simple/array refs during parse (large), not a cursor change. Latent (no fixture triggers it; py↔ts consistent).
- [ ] **Remaining CLI runner enhancements** (lower priority): `-o` output-file comparison and a quoted-arg `cmd` parser would let the corpus cover output-to-file and inline-SQL `query`; error-exit cases need agreement on exit codes (click vs clap). `workspace parse` needs `root`-path normalization to be byte-comparable.
- [ ] **Document the int/float JSON-equality caveat** (rust `1 != 1.0`; py/ts equal) — note for future numeric fixtures.

## Execution plan (layers) [[qmd61_finding_layers: Finding]]

Independently-testable layers; each gated by a green sequential `make test` (not `test-fast` — it has an install race).

- category: docs
- related_to: [[#qmd61]]
- solution: sequence so the riskiest (fixtures, docs rename) land early and the seed is last

### Layers [[qmd61_layers_detail: text]]

- L0 — LFS/`.gitignore` fix + semantic regenerate target + verify ([[#qmd61_finding_lfs]]). Small, safe.
- L0.5 — unified test reporting + anti-vacuous-pass guards + captured baseline counts ([[#qmd61_finding_test_reporting]]). Prerequisite to L2.
- L0.6 — parity-hardening follow-ups ([[#qmd61_finding_parity_followups]]): case-identity parity, array-field line anchoring, broader CLI corpus. Non-blocking; before L1.
- L1 — `docs2/` → `docs/` rename + reference sweep ([[#qmd61_finding_docs_rename]]).
- L2 — fixture relocation ([[#qmd61_finding_fixtures]]). Riskiest; gated by the unified report (identical before/after counts).
- L3 — README rewrite + community/health files ([[#qmd61_finding_readme_community]]).
- L4 — CI + release/publish workflows + Cloudflare Workers docs deploy + `site_url` ([[#qmd61_finding_ci_pages]], [[#qmd61_finding_publish]]). **Done.**
- L5 — housekeeping ([[#qmd61_finding_housekeeping]]).
- L6 — deliver the operator orphan-seed command sequence + final pre-seed grep/secret/LFS verification ([[#qmd61_finding_seed]]). Operator runs the push.

## Test / verification plan [[qmd61_finding_test_plan: Finding]]

This is an infra/repo task; verification is gate-green + grep-clean rather than new unit tests.

- category: testing
- related_to: [[#qmd61]]
- solution: per-layer checks below; no new unit tests proposed (existing data-driven gate covers parser behaviour)

### Checks [[qmd61_test_plan_detail: text]]

- After every layer: `make test` (sequential) green.
- After L0.5: `make test` emits one unified report (per-suite passed/skipped/failed/discovered + grand total); every data-driven suite fails on `discovered==0`; baseline counts recorded.
- After fixture move (L2): the unified report shows identical per-suite `discovered`+`passed` counts vs the L0.5 baseline; `grep -r "tasks/QMD-" */tests` returns nothing.
- After docs rename (L1): `make site WS=./docs` builds; `validate-docs` passes; repo-wide `grep -rn docs2` returns nothing.
- CI (L4): the workflow mirrors `make test` across a Linux/macOS/Windows binary matrix + py/ts/docs; release publishes only on a `v*` tag after CI is green.
- Docs deploy (L4): `make site-deploy` builds strict (warnings fatal) and `wrangler deploy`s to `qmdc.mikilabs.io` (Cloudflare Workers); root-served, no subpath.
- Pre-seed (L6): curated tree greps clean of legacy internal codenames; secret scan; `git lfs ls-files` resolves; root commit contains no excluded dir.
