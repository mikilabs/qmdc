# Contributing to QMDC

Thanks for your interest in contributing! This repository holds the QMD.md format and the QMDC toolchain: three parser implementations (Python, Rust, TypeScript), a VS Code extension, a semantic-search package, and a MkDocs integration.

## Getting set up

Prerequisites: Python 3.12+, Node.js 18+, Rust 1.70+, [`uv`](https://docs.astral.sh/uv/), and [Git LFS](https://git-lfs.com/) (the semantic search index ships as an LFS object).

```bash
git lfs install
git clone https://github.com/mikilabs/qmdc.git
cd qmdc
make init        # checks prerequisites, installs deps, builds everything, runs the tests
```

`make init` ends by running the full test suite, so a green run means your environment is ready.

## Development workflow

```bash
make test        # full sequential suite (the gate — run this before every PR)
make py-test     # Python only
make ts-test     # TypeScript only
make rs-test     # Rust only
make lint        # lint all three implementations
make format      # auto-format all three implementations
```

Always validate with the sequential `make test` before opening a PR. The parallel `make test-fast` is convenient locally but has an install race; CI and the merge gate use `make test`.

## The parity contract

The three parsers must behave identically. Conformance suites (`parser`, `workspace`, `sql`, `cli`) run the **same** fixture corpus under `tests/` against all three, and `make test` **fails** if the per-case counts diverge across languages.

Practically, this means:

- **Add fixtures, not bespoke per-language tests.** A test case is a folder under `tests/<suite>/` with an input document and an expected-output file. All three runners discover it automatically.
- **When you change parser behavior, change all three implementations** and keep their output byte-identical. The unified test report (printed by `make test`) shows the per-suite, per-language matrix.
- **Never hand-edit an expected file to make a test pass.** Regenerate it from the corrected parser output and confirm the change is intentional.

The shared corpus is purpose-organized: `tests/parser/`, `tests/workspace/`, `tests/sql/`, `tests/lsp/`, `tests/mcp/`, `tests/cli/`.

## Documentation

The docs site is built from the QMDC workspace in `docs/` with the MkDocs integration:

```bash
make site WS=./docs        # build the site
make site-serve WS=./docs  # build + live reload
```

Documentation content lives in `.qmd.md` files. Validate it with `./bin/qmdc-py workspace validate docs` (must return `[]`).

## Pull requests

- Keep PRs focused; one logical change per PR.
- Make sure `make test` is green and `make lint` is clean.
- Update docs and add or adjust fixtures when behavior changes.
- Write a clear description: what changed, why, and how you verified it.
- By contributing, you agree your contributions are licensed under the project's [AGPL-3.0-or-later](LICENSE) license.

## Reporting bugs and requesting features

Use the GitHub issue templates. For security issues, do **not** open a public issue — see [SECURITY.md](SECURITY.md).
