# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
The whole system is released under a single version following
[Semantic Versioning](https://semver.org/spec/v2.0.0.html); a release ships all
packages (`qmdc`, `qmdc-semantic`, `qmdc-mkdocs`, `qmdc-vscode`) together. This
file is maintained by hand.

## [1.0.1] - 2026-07-02

- MCP workspace resolution searches down then up, so any path (a repo or container dir) resolves the workspace inside it and a container with several workspaces returns an `ambiguous` error with candidates — bundled-binary bump shipping in [qmdc (PyPI)](https://pypi.org/project/qmdc/), [qmdc (crates.io)](https://crates.io/crates/qmdc), [@qmdc/qmdc (npm)](https://www.npmjs.com/package/@qmdc/qmdc), and [qmdc-vscode](https://marketplace.visualstudio.com/items?itemName=MiKiLabs.qmdc-vscode).

## [1.0.0] - 2026-06-13

Initial release.

[1.0.1]: https://github.com/mikilabs/qmdc/releases/tag/v1.0.1
[1.0.0]: https://github.com/mikilabs/qmdc/releases/tag/v1.0.0
