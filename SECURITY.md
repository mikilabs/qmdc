# Security Policy

## Supported versions

QMDC follows semantic versioning per package. Security fixes are applied to the latest released version of each package (`qmdc`, `qmdc-semantic`, `qmdc-mkdocs`, `qmdc-vscode`).

## Reporting a vulnerability

Please **do not** report security vulnerabilities through public GitHub issues.

Instead, use GitHub's [private vulnerability reporting](https://github.com/mikilabs/qmdc/security/advisories/new) ("Report a vulnerability" under the repository's **Security** tab). If that is unavailable, contact the maintainers privately at [security@mikilabs.io](mailto:security@mikilabs.io).

Please include:

- a description of the issue and its impact;
- steps to reproduce (a minimal `.qmd.md` fixture or command is ideal);
- affected package(s) and version(s);
- any suggested mitigation.

## What to expect

- We aim to acknowledge a report within **3 business days**.
- We will keep you informed of progress and coordinate a disclosure timeline with you.
- With your consent, we will credit you in the release notes once a fix is published.

## Scope notes

A few behaviors are intentional and not vulnerabilities on their own:

- The MCP server trusts caller-supplied paths under the local single-user model unless started with `--force-root`, which fail-closed restricts every operation to a root directory.
- The MkDocs content-regeneration command (`qmdc-mkdocs regenerate`) invokes an external agent driven by workspace document text and is an authoring-time tool; run it only on workspaces you trust. It is never part of a normal `build`/`serve`.
