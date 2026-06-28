//! Data-driven CLI CONFORMANCE runner (shared corpus in `tests/cli/`).
//!
//! Each parser runs the same corpus, so the `cli` row of the unified report
//! reaches parity by construction. Impl-specific CLI tests live in `cli.rs`
//! (→ unit-rs). See `tests/cli/README.md` for the fixture format.

use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::Once;

mod common;

fn qmdc_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/debug/qmdc")
}

fn corpus_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("tests/cli")
}

static BUILD_ONCE: Once = Once::new();

fn ensure_qmdc_built() {
    BUILD_ONCE.call_once(|| {
        let status = Command::new("cargo")
            .args(["build", "--bin", "qmdc"])
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .status()
            .expect("Failed to build qmdc");
        assert!(status.success(), "Failed to build qmdc binary");
    });
}

#[test]
fn test_cli_conformance() {
    ensure_qmdc_built();
    let corpus = corpus_dir();
    let bin = qmdc_bin();

    let mut dirs: Vec<PathBuf> = fs::read_dir(&corpus)
        .expect("read tests/cli")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_dir() && p.join("cmd").exists())
        .collect();
    dirs.sort();

    let mut report = common::CaseReport::new("cli", "rs-cli");

    for dir in dirs {
        let case_t = std::time::Instant::now();
        let name = dir.file_name().unwrap().to_string_lossy().to_string();
        let cmd = fs::read_to_string(dir.join("cmd")).expect("read cmd");
        let args: Vec<&str> = cmd.split_whitespace().collect();
        let stdin_data = fs::read_to_string(dir.join("stdin")).ok();
        let exit_expected: i32 = fs::read_to_string(dir.join("exit"))
            .ok()
            .and_then(|s| s.trim().parse().ok())
            .unwrap_or(0);

        let mut child = Command::new(&bin)
            .args(&args)
            .current_dir(&dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn qmdc");
        if let Some(data) = &stdin_data {
            child
                .stdin
                .take()
                .unwrap()
                .write_all(data.as_bytes())
                .expect("write stdin");
        }
        let output = child.wait_with_output().expect("wait qmdc");
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let actual_exit = output.status.code().unwrap_or(-1);

        let mut problem: Option<String> = None;
        if actual_exit != exit_expected {
            problem = Some(format!(
                "exit {} != expected {}",
                actual_exit, exit_expected
            ));
        }

        let exp_json = dir.join("expected.json");
        let exp_txt = dir.join("expected.txt");
        if problem.is_none() && exp_json.exists() {
            let expected: serde_json::Value =
                serde_json::from_str(&fs::read_to_string(&exp_json).unwrap())
                    .expect("parse expected.json");
            match serde_json::from_str::<serde_json::Value>(&stdout) {
                Ok(actual) => {
                    if actual != expected {
                        problem = Some("stdout JSON mismatch".to_string());
                    }
                }
                Err(e) => problem = Some(format!("stdout not JSON: {}", e)),
            }
        } else if problem.is_none() && exp_txt.exists() {
            let expected = fs::read_to_string(&exp_txt).unwrap();
            // Normalize line endings before comparing: git may check out fixtures
            // as CRLF on Windows (core.autocrlf), while the CLI always emits LF.
            if stdout.replace("\r\n", "\n").trim() != expected.replace("\r\n", "\n").trim() {
                problem = Some("stdout text mismatch".to_string());
            }
        }

        let secs = case_t.elapsed().as_secs_f64();
        match problem {
            None => report.pass(&name, secs),
            Some(msg) => {
                eprintln!("✗ {}: {}\n  stdout: {}", name, msg, stdout);
                report.fail(&name, &msg, secs);
            }
        }
    }

    report.finish();
}
