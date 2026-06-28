# Releasing [[releasing:__Namespace]]

How each part of the `qmdc` monorepo is versioned, built, and published, and the rebuild
cascade between components. The repo-root `RELEASING.md` is a pointer to this page — this
is the source of truth.

## Overview [[release_overview: NarrativeDoc]]

- about: [[#parsers]], [[#ssg]], [[#semantic]], [[#extension]]

`qmdc` is one logical tool with three implementations ([[#python_parser]],
[[#typescript_parser]], [[#rust_parser]]), kept behaviour-identical by the shared
data-driven test corpus (see [[#testing]]). The fast CLI is the native Rust binary; it is
bundled into the npm `@qmdc/qmdc`, PyPI `qmdc`, and [[#extension]] packages.

### Components [[release_components: text]]

| Package | Dir | Registry | User installs with |
| --- | --- | --- | --- |
| `qmdc` (parser + CLI) | `qmdc-py`, `qmdc-ts`, `qmdc-rs` | PyPI, npm, crates.io | `uvx qmdc`, `npx @qmdc/qmdc`, `cargo install qmdc` |
| `qmdc-semantic` | `qmdc-semantic` | PyPI | `uvx qmdc-semantic` |
| `qmdc-mkdocs` | `qmdc-mkdocs` | PyPI | `uvx qmdc-mkdocs` |
| `qmdc-vscode` | `qmdc-vscode` | VS Code Marketplace | from the Marketplace |

### Versioning [[release_versioning: text]]

Each package versions **independently** — there is no lockstep. A bug in one parser is
fixed and released on its own; parity is enforced by tests, not by equal version numbers.

### Rebuild Cascade [[release_cascade: text]]

What you must (re)publish for a given change:

| Changed | Publish | Why |
| --- | --- | --- |
| `qmdc-py/**` | PyPI `qmdc` | Python lib lives only in the PyPI package |
| `qmdc-ts/**` | npm `@qmdc/qmdc` | TS lib lives only in the npm package |
| `qmdc-rs/**` (the binary) | crates `qmdc` + npm `@qmdc/qmdc` + PyPI `qmdc` + `qmdc-vscode` | all four embed the binary |
| `qmdc-vscode/**` | `qmdc-vscode` | leaf |
| `qmdc-semantic/**` | PyPI `qmdc-semantic` | leaf |
| `qmdc-mkdocs/**` | PyPI `qmdc-mkdocs` | leaf |

A **binary change** is the only one that fans out — every package that embeds the native
binary must rebuild. A change to [[#rust_parser]] therefore cascades to the npm/PyPI `qmdc`
packages and the [[#extension]]. Use the cascade target rather than bumping the four by hand:

```bash
make binary-bump          # patch-bump crates + PyPI qmdc + npm qmdc + vscode together
make binary-bump-minor    # same cascade at minor
make binary-bump-major    # same cascade at major
```

`qmdc-semantic` / `qmdc-mkdocs` depend on `qmdc` at runtime (`qmdc>=1.0.0`), resolved from
PyPI at install time, so the cascade deliberately leaves them untouched.

### Releasing a Component [[release_steps: text]]

One idempotent path: build, then upload with `--skip-existing` — re-running a release is
safe (already-published artifacts are skipped). Bump first; a registry won't accept the
same version twice. Tokens come from `.env.publish` (gitignored) or the environment — CI
uses GitHub Secrets of the same names.

**Everything (the `qmdc` binary + the four packages that embed it):**

```bash
make binary-bump      # cascade-bump crates + PyPI qmdc + npm qmdc + vscode (-minor / -major)
make publish          # build the full matrix, then idempotently publish ALL registries
make publish-check    # same but DRY — build + show what WOULD publish, upload nothing
```

In CI this is automatic: pushing a `v*` tag runs the cross-platform tests, then `make publish`.

**One leaf package at a time:**

| Package | Bump | Build + publish |
| --- | --- | --- |
| `qmdc-vscode` | `make ext-bump` | `make ext-release` |
| `qmdc-mkdocs` | `make mkdocs-bump` | `make mkdocs-release` |
| `qmdc-semantic` | `make semantic-bump` | `make semantic-release` |

The `qmdc` PyPI / npm / crates packages share the native binary, so they release together
via `make publish` (bump with `make py-bump` / `ts-bump` / `rs-bump`, or `make binary-bump`).

**Selective single registry** (advanced) — after `make dist`:

```bash
bash scripts/release-publish.sh --publish pypi|npm|crate|vscode
```

### Build Matrix [[release_matrix: text]]

- about: [[#ssg]], [[#extension]]

`make dist` cross-builds every artifact into `dist-release/` (no publishing): 7 native
`qmdc` binaries (darwin, linux gnu+musl, windows; via `cargo` / `cargo-zigbuild`), 7 npm
`@qmdc/cli-<platform>` packages + the main tarball, 7 PyPI platform wheels for `qmdc` plus
`qmdc-semantic` / `qmdc-mkdocs` sdist+wheel, and 6 `qmdc-vscode` VSIX. `cargo install qmdc`
is the universal fallback for any target not in the matrix.

### Semantic Artifacts [[release_semantic: text]]

- about: [[#semantic]], [[#semantic_commands]]

The semantic embeddings DB and hints are committed artifacts (precomputed before release),
so the docs site (built by [[#ssg]]) needs no embedding provider. Refresh and commit them
before a release with `make semantic-refresh WS=./docs` (`embeddings.db` via Git LFS,
`hints.json` plain).
