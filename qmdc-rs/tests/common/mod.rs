//! Shared per-case JUnit reporter for the Rust data-driven test suites.
//!
//! cargo-nextest reports one entry per `#[test]` function, so a data-driven test
//! that loops over a whole fixture directory shows up as a single test — hiding
//! how many fixture *cases* actually ran (and passing vacuously if the directory
//! is empty). This reporter mirrors `qmdc-ts/tests/_report.ts`: each loop-test
//! records per case and writes a JUnit file to `<repo>/test-reports/<report>.xml`
//! tagged with a canonical suite name, so the repo-root aggregator
//! (`scripts/test-report.py`) can build a per-case, cross-parser parity matrix.
//!
//! Allowing `dead_code` because each test binary only uses part of the surface.
#![allow(dead_code)]

use std::fs;
use std::path::PathBuf;

pub enum Status {
    Pass,
    Fail(String),
    Skip,
}

pub struct CaseReport {
    /// Canonical suite (becomes the JUnit `classname`, e.g. "parser").
    suite: String,
    /// Unique report file stem (e.g. "rs-microtests") — distinct per test binary
    /// so two binaries feeding the same suite do not overwrite each other.
    report: String,
    /// (name, status, seconds). The duration is the real wall-clock time the
    /// caller measured for that case's work (`Instant`), emitted as the JUnit
    /// `time` attribute so a slow individual fixture is visible — no averaging.
    cases: Vec<(String, Status, f64)>,
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

impl CaseReport {
    pub fn new(suite: &str, report: &str) -> Self {
        Self {
            suite: suite.to_string(),
            report: report.to_string(),
            cases: Vec::new(),
        }
    }

    pub fn pass(&mut self, name: &str, secs: f64) {
        self.cases.push((name.to_string(), Status::Pass, secs));
    }

    pub fn fail(&mut self, name: &str, message: &str, secs: f64) {
        self.cases
            .push((name.to_string(), Status::Fail(message.to_string()), secs));
    }

    pub fn skip(&mut self, name: &str, secs: f64) {
        self.cases.push((name.to_string(), Status::Skip, secs));
    }

    fn report_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("test-reports")
    }

    /// Write the JUnit file, then assert the suite was not vacuous and had no
    /// failures (so `make test` fails loudly on either).
    pub fn finish(self) {
        let total = self.cases.len();
        let failures = self
            .cases
            .iter()
            .filter(|(_, s, _)| matches!(s, Status::Fail(_)))
            .count();
        let skipped = self
            .cases
            .iter()
            .filter(|(_, s, _)| matches!(s, Status::Skip))
            .count();

        let mut body = String::new();
        let mut total_secs = 0.0_f64;
        for (name, status, secs) in &self.cases {
            total_secs += *secs;
            let cn = xml_escape(name);
            match status {
                Status::Pass => body.push_str(&format!(
                    "    <testcase name=\"{}\" classname=\"{}\" time=\"{:.6}\"/>\n",
                    cn, self.suite, secs
                )),
                Status::Skip => body.push_str(&format!(
                    "    <testcase name=\"{}\" classname=\"{}\" time=\"{:.6}\"><skipped/></testcase>\n",
                    cn, self.suite, secs
                )),
                Status::Fail(msg) => body.push_str(&format!(
                    "    <testcase name=\"{}\" classname=\"{}\" time=\"{:.6}\"><failure>{}</failure></testcase>\n",
                    cn,
                    self.suite,
                    secs,
                    xml_escape(msg)
                )),
            }
        }

        let xml = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<testsuites>\n  \
             <testsuite name=\"{}\" tests=\"{}\" failures=\"{}\" skipped=\"{}\" time=\"{:.6}\">\n{}  \
             </testsuite>\n</testsuites>\n",
            self.suite, total, failures, skipped, total_secs, body
        );

        let dir = Self::report_dir();
        fs::create_dir_all(&dir).expect("create test-reports dir");
        fs::write(dir.join(format!("{}.xml", self.report)), xml).expect("write junit report");

        assert!(
            total > 0,
            "VACUOUS SUITE '{}' ({}): 0 cases discovered — fixture path likely broken",
            self.suite,
            self.report
        );
        assert!(
            failures == 0,
            "{} case(s) failed in suite '{}' ({})",
            failures,
            self.suite,
            self.report
        );
    }
}
