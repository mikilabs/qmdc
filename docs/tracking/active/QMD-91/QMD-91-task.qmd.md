# QMD-91: Relicense to Apache-2.0

## Change the project license to Apache-2.0 [[qmd91_apache_license: Feature]]

GitHub [#30](https://github.com/mikilabs/qmdc/issues/30). The project was under a copyleft license,
declared in the root and per-package `LICENSE` files, every package manifest and the READMEs. The
maintainer relicenses it to Apache-2.0. All commits on `main` are the maintainer's (plus the
release bot), so there is no outside contributor whose consent is needed.

Publishing with the new license is the next release's job, after merge.

- status: done_review
- priority: medium
- category: build
- findings: [[#qmd91_finding_scope]], [[#qmd91_finding_history]], [[#qmd91_finding_flake]]
- result: [[#qmd91_result]]

### Goals [[goals: [Goal]]]

#### A1: Every LICENSE file is the Apache-2.0 text [[qmd91_goal_a1]]

The root `LICENSE` and the per-package `LICENSE` files in qmdc-rs, qmdc-py, qmdc-ts, qmdc-vscode,
qmdc-semantic and qmdc-mkdocs are the canonical Apache-2.0 text. A root `NOTICE` names the
copyright holder.

- group: A_license
- done: true

#### A2: Every manifest declares Apache-2.0 [[qmd91_goal_a2]]

`Cargo.toml`, the three `pyproject.toml` files, both `package.json` files and their lock files,
and the two scripts that generate the `@qmdc/cli-*` npm manifests declare `Apache-2.0`.

- group: A_license
- done: true

#### A3: No mentions of the old license remain [[qmd91_goal_a3]]

The grep from the issue's Done criteria is empty: README badges and footers, `CONTRIBUTING.md` and the
docs index name Apache-2.0.

- group: A_license
- done: true
