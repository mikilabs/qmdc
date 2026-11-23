# QMD-89: vscode CI job cannot install Chromium on Ubuntu 26.04

## Playwright 1.58.2 has no Chromium build for ubuntu-26.04 runners [[qmd89_playwright_ubuntu26: Bug]]

GitHub [#25](https://github.com/mikilabs/qmdc/issues/25). The `vscode` CI job runs on
`ubuntu-latest`, which now resolves to Ubuntu 26.04. Its `npx playwright install --with-deps
chromium` step fails before any test runs: `Playwright does not support chromium on
ubuntu26.04-x64`. `qmdc-vscode` pins `@playwright/test` 1.58.2, whose platform table stops at
Ubuntu 24.04. Every PR that touches `qmdc-vscode/` gets a red `vscode` check whatever it changes.

The fix must not touch CI configuration.

- status: done_review
- priority: high
- category: build
- findings: [[#qmd89_finding_cause]], [[#qmd89_finding_fix]], [[#qmd89_finding_tests]]
- result: [[#qmd89_result]]

### Goals [[goals: [Goal]]]

#### A1: Reproduce the CI failure locally [[qmd89_goal_a1]]

With the host platform forced to `ubuntu26.04-x64`, a dry-run Chromium install shows no download
for 1.58.2 and a download for the new version.

- group: A_tests
- done: true

#### B1: Bump @playwright/test in qmdc-vscode [[qmd89_goal_b1]]

`qmdc-vscode/package.json` and its lock file move to a Playwright release that ships Chromium for
Ubuntu 26.04. No other dependency changes.

- group: B_fix
- done: true

#### B2: Preview tests pass on the new version [[qmd89_goal_b2]]

`make vscode-test` and `make test-fast` pass with the new Playwright.

- group: B_fix
- done: true
