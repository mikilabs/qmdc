# QMD-89: Findings

## The runner image moved past Playwright's platform table [[qmd89_finding_cause: Finding]]

The failing job log reports `Image: ubuntu-26.04`, version `20261116.202.1`. On 2026-10-26 the
same job passed on the previous image. Playwright picks browser builds by host platform, and
`playwright-core` 1.58.2 lists only `ubuntu20.04`, `ubuntu22.04` and `ubuntu24.04`, so on 26.04
it has no Chromium URL and `install` aborts. `playwright-core` 1.65.1 (latest) lists
`ubuntu26.04-x64` with a Chrome for Testing download.

- category: bug
- related_to: [[#qmd89_playwright_ubuntu26]]
- affected_files: [qmdc-vscode/package.json, qmdc-vscode/package-lock.json]
- solution: Bump `@playwright/test` to `^1.65.1`.

## Bump the dependency, leave CI alone [[qmd89_finding_fix: Finding]]

The other way out is pinning the job to `runs-on: ubuntu-24.04` in `.github/workflows/ci.yml`.
That is a CI configuration change, which this task does not make, and it would only postpone the
same failure. The bump changes three lock entries (`@playwright/test`, `playwright`,
`playwright-core`, all 1.58.2 to 1.65.1, Node floor `>=18` to `>=20`) and drops two optional
packages the old `playwright` pulled in (`fsevents` 2.3.2 and its `file-uri-to-path`). CI runs
the job on Node 20, so the new Node floor holds.

- category: design
- related_to: [[#qmd89_playwright_ubuntu26]]
- solution: `npm install -D @playwright/test@^1.65.1` in `qmdc-vscode`; `npm ci` from the new lock file succeeds.

## Tests [[qmd89_finding_tests: Finding]]

A test cannot usefully pin a dependency version, so the reproduction is the install itself with
Playwright's `PLAYWRIGHT_HOST_PLATFORM_OVERRIDE=ubuntu26.04-x64`: `install --dry-run chromium`
prints no download URL on 1.58.2 and the Chrome for Testing URL on 1.65.1. The existing 65
preview tests (`make vscode-test`) are the regression check that the upgrade breaks nothing.
The CI `vscode` job on the real 26.04 runner is the final check.

- category: testing
- related_to: [[#qmd89_playwright_ubuntu26]]
- solution: Existing tests only; no new test files.
