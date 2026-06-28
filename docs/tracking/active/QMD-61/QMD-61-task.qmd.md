# QMD-61: Stand up the public open-source repository

## Stand up the public qmdc repository [[qmd61: Feature]]

Everything needed to take the (renamed, release-ready) codebase from QMD-60 and turn it into a clean, public open-source repository at `https://github.com/mikilabs/qmdc` — fresh history, a proper README and community files, CI, a published documentation site, and a precise list of what does NOT ship publicly. QMD-60 makes the *packages* publishable; QMD-61 makes the *repository* publishable.

- status: in_progress
- priority: high
- category: docs

### Relationship to QMD-60 [[qmd61_vs_qmd60: text]]

QMD-60 (naming + release-readiness of packages) is the prerequisite. It produces: `qmdc`-named packages, working publish scripts (dry-run), filled manifests, a root `LICENSE`. QMD-61 consumes that state and builds the public-facing repository around it. Do NOT start QMD-61's "fresh history" step until QMD-60 is done and `make test-fast` is green — otherwise the initial public commit captures a half-renamed tree.

Overlap to avoid double-work: the root `LICENSE` file and the legacy-codename scrub are owned by QMD-60. QMD-61 assumes they are done and only verifies them.

### Git strategy: orphan branch as the public seed [[qmd61_git_strategy: text]]

Decision (operator-confirmed direction): the public repo gets a FRESH history, not the 6-month internal history (which carries WIP churn and ~170MB of binaries baked into old commits).

Mechanism clarification: a normal git branch does NOT give clean history — it still points into the existing commit graph. The right tool is an **orphan branch** (`git checkout --orphan public-main`): it starts with no parent, a single root commit containing only the public tree. That orphan branch is then pushed as `main` to the new `mikilabs/qmdc` repo.

- This repo stays the private working repo (full history, tracking, internal dirs). Nothing is archived.
- The orphan branch is the seed of the public repo; after the public repo exists and develops on its own, the orphan branch's job is done.
- All git operations here are mutating (`checkout --orphan`, `commit`, `push`, creating the remote) — per repo rules the OPERATOR runs them by hand; the task delivers the exact command sequence + the curated tree, not an agent-executed push.

### Public tree: what ships [[qmd61_ships: text]]

The public repo contains only the product:

- `qmdc-py`, `qmdc-ts`, `qmdc-rs`, `qmdc-semantic`, `qmdc-mkdocs`, `qmdc-vscode` (post-QMD-60 names)
- `docs2/` documentation source (minus `tracking/` — see exclusions) – rename into docs and check that no depencencies to docs2 are left
- the test fixtures the parsers need (currently under `tasks/` — see open question on relocation)
- root tooling: `Makefile`, `setup.sh`, the `qmdc`/`qmdc-*` wrappers, `README.md`, `LICENSE`, `.gitignore`, `.gitattributes`, `.markdownlint.json`
- new community + CI files (below)

### Public tree: what does NOT ship (exclusion list) [[qmd61_exclusions: text]]

Explicitly kept OUT of the public seed (they stay only in the private repo):

- `zold_docs/` — old Russian docs, superseded by `docs2/`.
- `presentations/` — internal pitch decks / "Frado" vision.
- `reviews/` — internal code-review notes.
- `org-ai-kb/` — internal AI-DLC methodology artifacts.
- `.kiro/` AI-DLC internals (skills, aidlc-common, specs) — decide per-subdir; steering that documents QMD format MAY stay, internal workflow tooling does not.
- `docs2/tracking/` — task tracking (QMD-N). Excluded from the published site already via `.qmdc-mkdocs.ignore`; also exclude from the public repo (or keep — operator decision; many OSS projects do keep a public roadmap).
- `method_iterations/` — single stale file.
- `.playwright-mcp/` — local logs/artifacts.
- any `tasks/` narrative artifacts (implementation plans, throwaway scripts, `.bak`) that are NOT test fixtures.

NOT excluded (correcting an earlier mistake): the `.qmdc-semantic/` databases and hints are KEPT and shipped — see [[#qmd61.qmd61_semantic_artifact]]. The only `.db` that leaves is `zold_docs/.qmdc-semantic/embeddings.db`, and only because `zold_docs/` itself is excluded.

Verification before the seed commit: grep the curated tree for any legacy internal codenames, and scan for secrets/PII; the public commit must be clean.

### README rewrite [[qmd61_readme: text]]

The current `README.md` is 152 lines, Russian, and describes the old `qmd-py`/`qmd-ts`/`qmd-rs` layout. Rewrite for an OSS audience in English:

- One-line what-is-QMD + the "Markdown → queryable graph" hook.
- Install/run via the published packages: `uvx qmdc`, `npx @qmdc/qmdc`, `cargo install qmdc`, VS Code Marketplace link.
- 60-second quickstart (create a `.qmd.md`, parse, query).
- Links to the docs site (GitHub Pages URL), the format spec, contributing.
- Badges: CI status, PyPI/npm/crates versions, license.
- Short "three parsers + LSP + SSG" architecture blurb.

### Community / health files [[qmd61_community: text]]

None of these exist yet. Add at repo root / `.github/`:

- `LICENSE` — produced by QMD-60; verify present.
- `CONTRIBUTING.md` — build from `make init`, run `make test-fast`, the data-driven test convention, PR norms.
- `CODE_OF_CONDUCT.md` — standard (Contributor Covenant).
- `CHANGELOG.md` — seed with the public 1.0 baseline.
- `SECURITY.md` — how to report vulnerabilities.
- `.github/ISSUE_TEMPLATE/` (bug + feature) and `PULL_REQUEST_TEMPLATE.md`.

### CI (GitHub Actions) [[qmd61_ci: text]]

No `.github/workflows` exists today. Add:

- **CI workflow**: on push/PR, run the equivalent of `make test-fast` across Python, TypeScript, Rust, mkdocs, plus lint. Matrix over the supported OSes for at least the Rust build.
- **Release workflow**: on tag, build the per-platform artifacts (PyPI platform wheels, npm `optionalDependencies` packages, crate, vsix) and publish via the QMD-60 publish scripts, using registry tokens from GitHub Secrets (never committed). This is the automation layer the QMD-60 publish scripts plug into.
- **Docs deploy workflow**: see below.

**Implemented (differs from the original plan above):** a single tag-gated `release.yml` (trigger `v*`, `needs:` the full `ci.yml` matrix) runs `make publish` — build the whole platform matrix then idempotently upload to PyPI, npm, crates.io, VS Code Marketplace and Open VSX (`scripts/release-publish.sh`, re-runnable / skip-existing). Not package-prefixed tags, not the per-package QMD-60 dry-run scripts. Docs deploy is **Cloudflare Workers** (`deploy-docs.yml` + `make site-deploy`, warnings fatal), not GitHub Pages. See [[#qmd61_finding_ci_pages]] and [[#qmd61_finding_publish]].

### Documentation site on GitHub Pages [[qmd61_pages: text]]

Publish the `qmdc-mkdocs`-built site for `docs2/` to GitHub Pages:

- Add `site_url` to `docs2/mkdocs.yml` (currently absent — needed for correct canonical links / sitemap).
- A GitHub Actions workflow builds the site (`make site WS=./docs2`, which runs `qmdc-mkdocs build`) and deploys to Pages on push to `main`.
- Decide the URL: project Pages `https://mikilabs.github.io/qmdc/` vs a custom domain. The `mkdocs.yml` `site_url` and any `base_url`/path config must match (project Pages serve under `/qmdc/`).
- Confirm the build is hermetic in CI (the `qmd` plugin needs the Python parser installed; semantic hints are precomputed and committed as an LFS artifact per [[#qmd61.qmd61_semantic_artifact]], so the CI build does NOT need an embedding provider — but the checkout MUST fetch LFS objects).

### Semantic DB + hints are checked-in artifacts (via Git LFS) [[qmd61_semantic_artifact: text]]

Correcting an earlier wrong call (I previously said to delete the semantic `.db` as bloat — that is wrong). The semantic embeddings DB and hints are first-class committed artifacts, and the repo is already set up for it:

- `.gitattributes` routes `*.db` through Git LFS (`filter=lfs diff=lfs merge=lfs -text`), and `git lfs ls-files` confirms `docs2/.qmdc-semantic/embeddings.db` and the mini-workspace fixture DB are tracked as LFS objects. So the working tree holds tiny pointer files, not the binary — there is NO 170MB problem in the seed.
- The intended model: BUILD/UPDATE the semantic index and hints as part of the release flow, then COMMIT them (the docs site's semantic-hint popovers read `docs2/.qmdc-semantic/hints.json`; shipping it precomputed means the Pages build needs no embedding provider at build time). This directly resolves the open question about hints in CI — they are precomputed and committed, not generated in CI.

Required fixes for this to work cleanly:

- **Remove the contradictory `.gitignore` lines 25-26** (`**/.qmdc-semantic/embeddings.db` and `**/.qmdc-semantic/hints.json`). They conflict with the LFS tracking: the files are already committed so gitignore doesn't drop them, but after a re-index someone would have to `git add -f` or the update silently won't stage. For a "build, update, check in" workflow these ignores must go.
- Make sure `hints.json` is also LFS-tracked or small enough to commit plain (it's JSON; check size — `.gitattributes` only covers `*.db`).
- Add a Make target / release step to regenerate both before a release: `make semantic-index WS=./docs2` then `make semantic-hints WS=./docs2` (targets already exist), and commit the updated LFS DB + hints.
- The public repo must have Git LFS enabled (GitHub supports it). Document that contributors need `git lfs install`; the CI docs-build checkout must fetch LFS objects.

Note this slightly revises the QMD-60 "uv cache / build" findings only in spirit — semantic stays a shipped artifact, not a build-time-only thing.

### Repo metadata [[qmd61_repo_meta: text]]

On the GitHub repo itself: description, topics/tags (`markdown`, `knowledge-graph`, `lsp`, `rust`, `python`, `typescript`), social-preview image, enable Discussions/Issues as desired, branch protection on `main`.

### Open Questions [[qmd61_open_questions: text]]

To resolve during triage:

1. `tasks/` fixtures: the parser tests depend on fixtures under `tasks/`. Either (a) relocate them to per-package `tests/fixtures/` (cleaner public layout, but touches ~20 test path constants — arguably its own task QMD-62), or (b) ship a pruned `tasks/` containing only fixtures. Decide before the seed commit.

a)

1. Keep `docs2/tracking/` in the public repo (public roadmap) or exclude it? It's already out of the built site.

Do housekeeping, so like analyze what's there, delete or, uh, move to declined folder or something, something we won't do ever or what was done in different ways, but check in everything else, I believe

1. Pages URL: project Pages (`/qmdc/` subpath) vs custom domain — affects `site_url` and link bases.

/qmdc/ subpath + some redirect at the /

1. License choice (MIT vs Apache-2.0) — nominally owned by QMD-60, but if undecided it blocks QMD-61 too.

AGPL

### Resolved during triage [[qmd61_resolved]]

- ~~Semantic hints in CI docs build~~ — RESOLVED: the semantic DB + `hints.json` are committed artifacts (Git LFS), precomputed before release, so CI builds the site without an embedding provider. See [[#qmd61.qmd61_semantic_artifact]]. Requires removing the contradictory `.gitignore` lines 25-26.

## Checklist

### Repo hygiene & structure

- [x] `docs2/` → `docs/` rename + full reference sweep
- [x] Fixture relocation `tasks/` → repo-root `tests/` (organized by purpose)
- [x] Unified test reporting + anti-vacuous guards + cross-parser parity gate
- [x] LFS / `.gitignore`: semantic artifacts committed (`embeddings.db` via LFS, `hints.json` plain)
- [x] Scrub legacy internal codenames from the tree
- [x] Housekeeping: tracking `declined/` folder + audit `planned/` tasks (QMD-62 is a live feature — kept)
- [ ] Final pre-seed verification gate (grep codenames / secrets / LFS) — run at seed time

### OSS front door

- [x] Community/health files: `LICENSE`, `CONTRIBUTING.md`, `CODE_OF_CONDUCT.md`, `CHANGELOG.md`, `SECURITY.md`, `.github/ISSUE_TEMPLATE/*`, `PULL_REQUEST_TEMPLATE.md`
- [x] `site_url` in `docs/mkdocs.yml`
- [x] README rewrite (English, OSS audience) — full rewrite with badges, install, quickstart, packages, CLI, build-from-source

### CI / release / publish

- [x] CI workflow (`ci.yml`): cross-platform matrix — native binary (parser+LSP+MCP) on Linux/macOS/Windows + Python + TypeScript + docs
- [x] Release workflow (`release.yml`): tag `v*`, gated by CI, runs idempotent `make publish`
- [x] Idempotent publisher `scripts/release-publish.sh` + `make publish` / `publish-check`, and per-leaf `make ext-release` / `mkdocs-release` / `semantic-release`
- [x] npm launcher `optionalDependencies` auto-pinned to the built cli version (`release-build.sh`)
- [x] Token config `.env.publish` (+ committed `.env.publish.example`); gitignored
- [x] Docs deploy: Cloudflare Workers (`deploy-docs.yml`) + `make site-deploy` (warnings fatal)

### First publish (bootstrap)

- [x] PyPI — `qmdc 1.0.3`, `qmdc-mkdocs 1.0.0`, `qmdc-semantic 1.0.0`
- [x] npm — `@qmdc/cli-*@1.0.4` (7 platform packages)
- [x] crates.io — `qmdc 1.0.4`
- [x] Open VSX — `qmdc-vscode 1.0.6` (6 platforms)
- [ ] npm main launcher — published as scoped `@qmdc/qmdc` (npm rejected the unscoped `qmdc` name via its similarity filter; support declined, so we use the scope already owned for `@qmdc/cli-*`)
- [ ] VS Code Marketplace — pending `VSCE_PAT`

### Go public (operator-run)

- [ ] Create empty `mikilabs/qmdc` repo on GitHub
- [ ] Orphan-branch seed + push (command sequence delivered)
- [ ] GitHub repo metadata: description, topics, branch protection, social preview
- [ ] DNS / custom-domain wiring for the docs site
