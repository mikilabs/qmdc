# Releasing

The full, rendered guide is the source of truth:

- **Rendered:** <https://qmdc.mikilabs.io/releasing/>
- **Source:** [`docs/releasing/readme.qmd.md`](docs/releasing/readme.qmd.md)
- **Recording the demo casts/gifs/mp4s:** <https://qmdc.mikilabs.io/releasing/demos/>

This file is the practical quick reference for the publish + deploy commands.

## Versioning

Each package versions **independently** — they don't have to share a number:

- `qmdc-rs` (crate) · `qmdc-py` · `qmdc-ts` · `qmdc-vscode` · `qmdc-semantic` · `qmdc-mkdocs`
- The native binary is embedded in py/ts/vscode and shipped standalone via the crate.
  When the binary changes, rebuild the bundlers — `make binary-bump` bumps rs+py+ts+vscode together.
- The npm launcher's `optionalDependencies` are pinned automatically to the built cli
  version at pack time (see `scripts/release-build.sh`), so the launcher and its
  `@qmdc/cli-*` platform packages always match — no manual pin maintenance.

## Publishing packages

One idempotent path: **build the whole platform matrix, then upload everything**.
Re-running is safe — anything already in a registry is skipped.

```bash
make publish-check     # build the full matrix + DRY-RUN (uploads nothing; shows the plan)
make publish           # build the full matrix + REAL idempotent upload to ALL registries
```

Target a single registry (handy for bootstrap or partial re-runs):

```bash
bash scripts/release-publish.sh --publish pypi     # or: npm | crate | vscode
```

### Tokens

Copy the template and fill in the tokens (the file is gitignored):

```bash
cp .env.publish.example .env.publish
```

| Registry | Env var | Notes |
|----------|---------|-------|
| PyPI | `UV_PUBLISH_TOKEN` | `qmdc`, `qmdc-semantic`, `qmdc-mkdocs` |
| npm | `NODE_AUTH_TOKEN` | `@qmdc/qmdc` + `@qmdc/cli-*`; token must bypass 2FA (Automation/Granular) |
| crates.io | `CARGO_REGISTRY_TOKEN` | requires a verified email on the account |
| VS Code Marketplace | `VSCE_PAT` | Azure DevOps PAT, publisher `mikilabs` |
| Open VSX | `OVSX_TOKEN` | namespace `mikilabs` (auto-created on first publish) |

Idempotency per registry: PyPI `twine --skip-existing`, npm `npm view` pre-check,
crates.io version pre-check, vscode `--skip-duplicate`. The two registries are
independent — whichever token is set gets published; the other is skipped.

## Release a single component

`make publish` builds the **whole** platform matrix (slow, ~30 min cold). To ship
just one package, use its dedicated path — these reuse the cargo cache and never
rebuild the other packages.

**VS Code extension** (Marketplace + Open VSX):

```bash
make ext-bump      # bump the version (a registry won't accept the same version twice)
make ext-release   # build 6 platform .vsix → dist-release/vscode, then publish:
                   #   Open VSX (OVSX_TOKEN) always, VS Code Marketplace if VSCE_PAT is set
```

If you publish to the Marketplace by hand, upload the matching file from
`dist-release/vscode/`. The Rust binary is taken from the cargo cache, so this is
seconds-per-platform, not a cold cross-compile (unless the Rust source changed).

**qmdc-mkdocs** (pure-Python, PyPI):

```bash
make mkdocs-bump      # bump the version
make mkdocs-release   # build sdist+wheel, idempotent `twine upload --skip-existing`
```

**qmdc-semantic** (pure-Python, PyPI):

```bash
make semantic-bump      # bump the version
make semantic-release   # build sdist+wheel, idempotent `twine upload --skip-existing`
```

## CI & release (GitHub Actions)

- **`ci.yml`** — on every push/PR: the native binary (parser + LSP + MCP) is tested
  on Linux, macOS **and** Windows, plus Python, TypeScript and docs jobs.
- **`release.yml`** — on a version tag `v*` (or manual): runs the full CI matrix first
  (`needs: test`), then `make publish` on a macOS runner with tokens from GitHub Secrets
  (same names as the env vars above). Nothing reaches a registry unless every OS is green.

So the normal release is: bump the package(s) → commit → push a `v*` tag.

## Docs site

```bash
make site-build      # build into docs/_site (lenient, hermetic — uses committed hints.json)
make site-serve      # live-reload preview on :8800
make site-build-strict   # build with warnings FATAL (mkdocs WARNING / qmdc WARN:)
make site-deploy     # refresh semantic → strict build → `wrangler deploy` to Cloudflare
```

`site-build` is hermetic (no embedding provider needed — CI builds from the committed
`docs/.qmdc-semantic/hints.json`). `site-deploy` first runs `semantic-refresh` so the
deployed site ships fresh "similar objects" popovers + inferred edges, then a strict
build (refuses to deploy on any warning — e.g. a broken `[[#ref]]`), then deploys.

`site-deploy` preconditions: an embedding provider for the refresh (local Ollama on
`:11434` with `qwen3-embedding`, or `OPENROUTER_API_KEY`), and wrangler auth
(`CLOUDFLARE_API_TOKEN` + `CLOUDFLARE_ACCOUNT_ID`, or `wrangler login`). After a refresh,
commit the updated `docs/.qmdc-semantic/{embeddings.db,hints.json}` so CI stays in sync.
