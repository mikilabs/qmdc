import { defineConfig } from '@playwright/test';

export default defineConfig({
  testDir: './e2e',
  timeout: 30_000,
  // Console output + a JUnit report consumed by the unified gate
  // (scripts/test-report.py reads test-reports/*.xml; cwd is qmdc-vscode/).
  reporter: [['list'], ['junit', { outputFile: '../test-reports/vscode.xml' }]],
  // Fail the run if no test files are discovered — guards against a silent
  // vacuous pass if the suite is ever mis-pointed (L0.5 anti-skip invariant).
  forbidOnly: !!process.env.CI,
  use: {
    browserName: 'chromium',
  },
});
