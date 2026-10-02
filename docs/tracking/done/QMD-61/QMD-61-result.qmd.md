# QMD-61: Result

## The public repository is live [[qmd61_result: Result]]

`mikilabs/qmdc` exists, is PUBLIC, and carries a fresh history: the orphan seed `6e89a28`
("Initial public release of QMD.md format and QMDC toolchain", 2026-06-28) has no parent, and
`public/main` has since grown to 16 commits. The documentation site is served at
<https://qmdc.mikilabs.io/> through the Cloudflare custom domain declared in `wrangler.toml`, and
all five registries carry a published package. The task is closed on measurement of the live
artifacts, not on the checklist: every item below was verified by querying GitHub, the registries
and the pushed tree.

One item is deliberately left open — branch protection on `main` — and one class of defect was
found by finally running the pre-seed gate against the tree that was already public.

- feature: [[#qmd61]]
- files_changed: [docs/tracking/active/QMD-61/QMD-61-task.qmd.md, docs/tracking/done/QMD-63/QMD-63-task.qmd.md, docs/tracking/done/QMD-76/QMD-76-result.qmd.md]
- tests_added: []

### What is live [[qmd61_result_live: text]]

- about: [[#qmd61_result]]

| surface | state |
| --- | --- |
| repository | `mikilabs/qmdc`, PUBLIC, default branch `main`, description + 14 topics + custom social preview |
| history | orphan root `6e89a28` (2026-06-28), no parent; 16 commits on `public/main` |
| docs site | <https://qmdc.mikilabs.io/> → 200, title "QMDC Documentation"; Cloudflare custom domain, assets-only Worker |
| PyPI | `qmdc 1.0.3`, `qmdc-mkdocs 1.0.0`, `qmdc-semantic 1.0.0` |
| npm | `@qmdc/cli-*@1.0.4` (7 platform packages) + launcher `@qmdc/qmdc@1.0.6` |
| crates.io | `qmdc 1.0.4` |
| VS Code Marketplace | `MiKiLabs.qmdc-vscode@1.0.11` |
| Open VSX | `qmdc-vscode 1.0.6` (6 platforms) |

The unscoped npm name `qmdc` is still unavailable and will stay that way: npm's similarity filter
rejected it and support declined the appeal, so the launcher ships under the scope already owned for
the platform packages.

### The pre-seed gate, run after the fact [[qmd61_result_gate: text]]

- about: [[#qmd61_result]]

The gate was the last unchecked hygiene item and was supposed to run *before* the seed commit. Run
now against `public/main`, three of its four checks pass outright:

- **Secrets** — no match for AWS keys, GitHub tokens, OpenAI keys, Slack tokens or PEM private-key
  headers anywhere in the tree.
- **LFS** — `docs/.qmdc-semantic/embeddings.db` is a pointer (132,202,496 bytes behind it), as is the
  mini-workspace fixture DB. The working tree holds no binaries, so the seed never carried the
  ~170 MB the strategy was written to avoid.
- **Exclusion list** — `zold_docs/`, `presentations/`, `reviews/`, `org-ai-kb/`,
  `method_iterations/` and `.playwright-mcp/` are all absent from the public tree. `.kiro/` is
  present but reduced to three steering files. `docs/tracking/` ships, per the triage decision to
  keep a public roadmap.

The fourth check failed, and it failed *because* tracking ships. Two leaks were live in the public
repository:

- `docs/tracking/active/QMD-61/QMD-61-task.qmd.md` — the exclusion list named the internal codename
  of the pitch deck it was excluding.
- `docs/tracking/done/QMD-63/QMD-63-task.qmd.md` — a pasted MCP error log carried an absolute local
  path, leaking both the operator's username and the private repository's name.

Both are redacted. A third, not yet public, was caught in the same sweep:
`docs/tracking/done/QMD-76/QMD-76-result.qmd.md` referenced a sibling package by absolute local
path; it now uses the package-relative path.

The lesson is structural, not incidental: "scrub legacy codenames from the tree" was checked off
against the *product* tree, while the decision to publish `docs/tracking/` turned every tracking
document into published material. Tracking prose is written for an internal reader and quotes
tool output verbatim, so it is the most likely place for a hostname, a home directory or a
codename to survive. Whatever runs this gate in future must cover `docs/tracking/` explicitly,
and the check belongs in CI rather than in a human checklist — `public/main` was published for two
months with the leak in it, and the checklist item that would have caught it was still unticked
the whole time.

`docs/.qmdc-semantic/embeddings.db` embeds the docs text, so it still holds the redacted strings
until the index is rebuilt. That happens on the next `make semantic-index WS=./docs`, which the
release flow runs anyway.

### Residual: branch protection [[qmd61_result_residual: text]]

- about: [[#qmd61_result]]

`GET repos/mikilabs/qmdc/branches/main/protection` answers 404 "Branch not protected". It is the
one go-public item genuinely not done. It is a GitHub setting with no code behind it, it cannot be
set from this repository, and on a single-maintainer repo nothing is currently broken by its
absence — so it is recorded here rather than holding the task open. It matters once external
contributors open pull requests, because the release workflow is tag-gated and a direct push to
`main` bypasses the CI matrix that `release.yml` depends on.

### What this task did not own [[qmd61_result_scope: text]]

- about: [[#qmd61_result]]

The publish pipeline's shape changed during the work and the task text was corrected in place
rather than at the end: a single tag-gated `release.yml` running idempotent `make publish`, instead
of the per-package dry-run scripts from QMD-60; Cloudflare Workers instead of GitHub Pages, so the
`/qmdc/` subpath decision from triage no longer applies and the site sits at a domain root.

Two structural pieces were large enough to be their own work and are recorded in the findings
rather than here: the `tasks/` → `tests/` fixture relocation with its unified test reporting and
cross-parser parity gate, and the `docs2/` → `docs/` rename. Both are complete. The parity gate in
particular has since earned its place — it is the mechanism that surfaced the parser divergences
now tracked as QMD-71, QMD-74 and QMD-75.
