/**
 * Shared test reporter for the TypeScript suites.
 *
 * The TS runners are bespoke (no jest/vitest), so this writes a minimal JUnit XML
 * to `<repo>/test-reports/<suite>.xml` that the repo-root aggregator
 * (`scripts/test-report.py`) reads alongside the pytest/nextest reports.
 *
 * It also enforces an anti-vacuous guard: a suite that executed fewer than
 * `minExpected` cases (default 1) fails loudly, so a mispointed fixture path can
 * never pass silently with zero tests discovered.
 */
import { writeFileSync, mkdirSync } from 'fs';
import { resolve, dirname } from 'path';
import { fileURLToPath } from 'url';

const REPORT_DIR = resolve(dirname(fileURLToPath(import.meta.url)), '../../test-reports');

function xmlEscape(s: string): string {
  return s
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;');
}

type CaseStatus = 'pass' | 'fail' | 'skip';

interface CaseResult {
  name: string;
  status: CaseStatus;
  /** Wall-clock seconds for this case's work — emitted as the JUnit `time`. */
  sec: number;
  message?: string;
}

/**
 * Per-case JUnit reporter, mirroring the Rust `CaseReport` (qmdc-rs/tests/common/mod.rs).
 *
 * Each case records its real measured duration (no averaging), so a slow individual
 * fixture is visible in the report. `time(name, fn)` is the ergonomic path for the
 * throw-on-failure runners; `pass/fail/skip` are for explicit branches.
 */
export class CaseReport {
  private cases: CaseResult[] = [];

  constructor(private readonly suite: string) {}

  pass(name: string, sec: number): void {
    this.cases.push({ name, status: 'pass', sec });
  }

  fail(name: string, message: string, sec: number): void {
    this.cases.push({ name, status: 'fail', sec, message });
  }

  skip(name: string, sec = 0): void {
    this.cases.push({ name, status: 'skip', sec });
  }

  /** Run `fn`, timing it; record `pass`, or `fail` if it throws. */
  time(name: string, fn: () => void): void {
    const t0 = performance.now();
    try {
      fn();
      this.cases.push({ name, status: 'pass', sec: (performance.now() - t0) / 1000 });
    } catch (e) {
      this.cases.push({
        name,
        status: 'fail',
        sec: (performance.now() - t0) / 1000,
        message: e instanceof Error ? e.message : String(e),
      });
    }
  }

  /** Write the JUnit file, enforce the vacuous guard, and exit non-zero on failures. */
  finish(minExpected = 1): void {
    const total = this.cases.length;
    const failed = this.cases.filter((c) => c.status === 'fail').length;
    const skipped = this.cases.filter((c) => c.status === 'skip').length;
    mkdirSync(REPORT_DIR, { recursive: true });

    let totalSec = 0;
    const lines = this.cases.map((c) => {
      totalSec += c.sec;
      const attrs = `name="${xmlEscape(c.name)}" classname="${xmlEscape(this.suite)}" time="${c.sec.toFixed(6)}"`;
      if (c.status === 'skip') return `    <testcase ${attrs}><skipped/></testcase>`;
      if (c.status === 'fail') {
        const body = c.message ? `<failure>${xmlEscape(c.message)}</failure>` : '<failure/>';
        return `    <testcase ${attrs}>${body}</testcase>`;
      }
      return `    <testcase ${attrs}/>`;
    });

    const xml =
      `<?xml version="1.0" encoding="UTF-8"?>\n<testsuites>\n` +
      `  <testsuite name="${xmlEscape(this.suite)}" tests="${total}" ` +
      `failures="${failed}" skipped="${skipped}" time="${totalSec.toFixed(6)}">\n` +
      `${lines.join('\n')}\n  </testsuite>\n</testsuites>\n`;

    writeFileSync(resolve(REPORT_DIR, `${this.suite}.xml`), xml);

    if (total < minExpected) {
      console.error(
        `\n✗ VACUOUS SUITE: ${this.suite} executed ${total} tests (expected >= ${minExpected}). ` +
          `A fixture path is likely broken.`
      );
      process.exit(1);
    }
    if (failed > 0) process.exit(1);
  }
}
