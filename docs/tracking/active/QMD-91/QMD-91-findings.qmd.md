# QMD-91: Findings

## Where the license is declared [[qmd91_finding_scope: Finding]]

The grep from the issue's Done criteria matched 27 files on `main`: seven `LICENSE` files, six manifests,
two lock files, two release scripts that write the `@qmdc/cli-*` npm manifests
(`scripts/release-build.sh`, `qmdc-ts/scripts/publish.sh`), seven READMEs, `CONTRIBUTING.md`,
`docs/readme.md` and two done tracking records. `qmdc-vscode/LICENSE` held the MIT text, which
disagreed with its own manifest; it now matches the rest. No source file carries a license header.

- category: design
- related_to: [[#qmd91_apache_license]]
- solution: Copy the canonical Apache-2.0 text (apache.org, SHA-256 `cfc7749b…`) to every `LICENSE`, add a root `NOTICE`, and replace the SPDX id in manifests, lock files, scripts and READMEs.

## Historical tracking records [[qmd91_finding_history]]

QMD-61's task and findings recorded the original license choice by name. To keep that record
honest while meeting the empty-grep criterion, the three lines now say a copyleft license was
chosen at the time and point to this task for the change.

- category: design
- related_to: [[#qmd91_apache_license]]
- solution: Reword the three lines in `docs/tracking/done/QMD-61/` instead of deleting them.

## Unrelated LSP flake during verification [[qmd91_finding_flake]]

The first `make test-fast` run failed one Rust LSP microtest,
`diagnostics/036-table-in-primitive-array`, which got the same diagnostic twice. This diff
changes no code. `cargo test --test lsp` passed 3 runs out of 3 on its own, and the next full
`make test-fast` run exited 0. Tracked separately as GitHub
[#31](https://github.com/mikilabs/qmdc/issues/31).

- category: testing
- related_to: [[#qmd91_apache_license]]
- solution: Out of scope here; see #31.
