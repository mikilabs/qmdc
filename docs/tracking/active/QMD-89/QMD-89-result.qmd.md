# QMD-89: Result

## qmdc-vscode on Playwright 1.65.1 [[qmd89_result: Result]]

`@playwright/test` in `qmdc-vscode` moves from `^1.58.2` to `^1.65.1`, the latest release, whose
browser table includes Ubuntu 26.04. The lock file changes only the three Playwright
packages and drops two optional packages the old version pulled in. No CI configuration changes.

- feature: [[#qmd89_playwright_ubuntu26]]
- completed: 2026-11-23
- files_changed: [qmdc-vscode/package.json, qmdc-vscode/package-lock.json]
- tests_changed: []

### Verification [[qmd89_result_verification: text]]

- about: [[#qmd89_result]]

- `PLAYWRIGHT_HOST_PLATFORM_OVERRIDE=ubuntu26.04-x64 npx playwright install --dry-run chromium`:
  1.58.2 prints install locations with no download URL; 1.65.1 prints
  `cdn.playwright.dev/builds/cft/159.0.8154.6/linux64/chrome-linux64.zip`.
- `npm ci` in `qmdc-vscode` from the new lock file exits 0.
- `make vscode-test`: 65 passed on 1.58.2 before the bump, 65 passed on 1.65.1 after it
  (Chromium 1208 and 1255).
- `make test-fast` exits 0.
- Code review: self-review only; the change is a dependency bump with no source edits.
