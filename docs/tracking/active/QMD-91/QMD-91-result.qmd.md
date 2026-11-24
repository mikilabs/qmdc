# QMD-91: Result

## The project is licensed Apache-2.0 [[qmd91_result: Result]]

Every `LICENSE` file is the Apache-2.0 text, a root `NOTICE` names the copyright holder, and
every manifest, lock file, release script and README declares `Apache-2.0`. No code changes.

- feature: [[#qmd91_apache_license]]
- completed: 2026-11-24
- files_changed: [LICENSE, NOTICE, CONTRIBUTING.md, README.md, docs/readme.md, scripts/release-build.sh, qmdc-rs/LICENSE, qmdc-rs/Cargo.toml, qmdc-rs/README.md, qmdc-py/LICENSE, qmdc-py/pyproject.toml, qmdc-py/README.md, qmdc-semantic/LICENSE, qmdc-semantic/pyproject.toml, qmdc-semantic/README.md, qmdc-mkdocs/LICENSE, qmdc-mkdocs/pyproject.toml, qmdc-mkdocs/README.md, qmdc-ts/LICENSE, qmdc-ts/package.json, qmdc-ts/package-lock.json, qmdc-ts/scripts/publish.sh, qmdc-ts/README.md, qmdc-vscode/LICENSE, qmdc-vscode/package.json, qmdc-vscode/package-lock.json, qmdc-vscode/README.md, docs/tracking/done/QMD-61/QMD-61-task.qmd.md, docs/tracking/done/QMD-61/QMD-61-findings.qmd.md]
- tests_changed: []

### Verification [[qmd91_result_verification: text]]

- about: [[#qmd91_result]]

- The grep from the issue's Done criteria exits 1 (no matches).
- All seven `LICENSE` files have the same SHA-256 as the apache.org text.
- `make test-fast`, `make validate-docs` and `make md-lint` exit 0.
- Code review: self-review only; the change touches no code.
