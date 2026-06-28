//! CLI tests - test qmdc executable via subprocess

use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::Once;
use tempfile::TempDir;

fn get_qmdc_path() -> PathBuf {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    PathBuf::from(manifest_dir).join("target/debug/qmdc")
}

fn get_microtests_dir() -> PathBuf {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    PathBuf::from(manifest_dir)
        .parent()
        .unwrap()
        .join("tests/parser")
}

static BUILD_ONCE: Once = Once::new();

/// Build the qmdc binary once for all tests in this module
fn ensure_qmdc_built() {
    BUILD_ONCE.call_once(|| {
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let status = Command::new("cargo")
            .args(["build", "--bin", "qmdc"])
            .current_dir(manifest_dir)
            .status()
            .expect("Failed to build qmdc");

        assert!(status.success(), "Failed to build qmdc binary");
    });
}

#[test]
fn test_cli_parse_stdin() {
    ensure_qmdc_built();

    let mut child = Command::new(get_qmdc_path())
        .args(["parse"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to spawn qmdc");

    // Write to stdin
    {
        let stdin = child.stdin.as_mut().unwrap();
        stdin.write_all(b"## Test [[test]]").unwrap();
    }

    let output = child.wait_with_output().expect("Failed to read output");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let result: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("Failed to parse JSON output");

    assert!(result.is_array(), "Output should be array");
    let arr = result.as_array().unwrap();
    assert_eq!(arr.len(), 1, "Should have one object");
    assert_eq!(arr[0]["__id"], "test");
}

#[test]
fn test_cli_parse_file() {
    ensure_qmdc_built();

    let qmdc_file = get_microtests_dir().join("001-empty-object.qmd.md");

    let output = Command::new(get_qmdc_path())
        .args(["parse", "-i", qmdc_file.to_str().unwrap()])
        .output()
        .expect("Failed to run qmdc");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let result: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("Failed to parse JSON output");

    assert!(result.is_array(), "Output should be array");
    let arr = result.as_array().unwrap();
    assert_eq!(
        arr.len(),
        2,
        "Should have two objects (Document + TextBlock)"
    );
    assert_eq!(arr[0]["__kind"], "__Document");
    assert_eq!(arr[1]["__kind"], "__TextBlock");
}

#[test]
fn test_cli_parse_output_file() {
    ensure_qmdc_built();

    let tmpdir = TempDir::new().expect("Failed to create temp dir");
    let output_file = tmpdir.path().join("output.json");

    let mut child = Command::new(get_qmdc_path())
        .args(["parse", "-o", output_file.to_str().unwrap()])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to spawn qmdc");

    {
        let stdin = child.stdin.as_mut().unwrap();
        stdin.write_all(b"## Test [[test]]").unwrap();
    }

    let status = child.wait().expect("Failed to wait");
    assert!(status.success(), "Command failed");

    assert!(output_file.exists(), "Output file should exist");

    let content = fs::read_to_string(&output_file).expect("Failed to read output file");
    let result: serde_json::Value =
        serde_json::from_str(&content).expect("Failed to parse JSON output");

    assert!(result.is_array(), "Output should be array");
    let arr = result.as_array().unwrap();
    assert_eq!(arr.len(), 1, "Should have one object");
    assert_eq!(arr[0]["__id"], "test");
}

#[test]
fn test_cli_first_5_microtests() {
    ensure_qmdc_built();

    let microtests_dir = get_microtests_dir();

    for i in 1..=5 {
        let pattern = format!("{:03}-", i);
        let qmdc_file = fs::read_dir(&microtests_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .find(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| n.starts_with(&pattern) && n.ends_with(".qmd.md"))
                    .unwrap_or(false)
            })
            .unwrap_or_else(|| panic!("Test file {:03} not found", i));

        let expected_file = qmdc_file.with_extension("").with_extension("expected.json");

        let output = Command::new(get_qmdc_path())
            .args(["parse", "-i", qmdc_file.to_str().unwrap()])
            .output()
            .expect("Failed to run qmdc");

        assert!(
            output.status.success(),
            "Test {:03} failed: {}",
            i,
            String::from_utf8_lossy(&output.stderr)
        );

        let result: serde_json::Value = serde_json::from_slice(&output.stdout)
            .unwrap_or_else(|_| panic!("Failed to parse JSON output for test {:03}", i));

        let expected: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&expected_file).expect("Failed to read expected file"),
        )
        .expect("Failed to parse expected JSON");

        assert_eq!(result, expected, "Test {:03} output mismatch", i);
    }
}

#[test]
fn test_cli_rebuild_stdin() {
    ensure_qmdc_built();

    let mut child = Command::new(get_qmdc_path())
        .args(["rebuild"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to spawn qmdc");

    {
        let stdin = child.stdin.as_mut().unwrap();
        stdin
            .write_all(br#"[{"__id": "test", "__label": "Test", "__level": 2}]"#)
            .unwrap();
    }

    let output = child.wait_with_output().expect("Failed to read output");

    assert!(
        output.status.success(),
        "rebuild failed! stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("## Test [[test]]"),
        "rebuild should generate QMD output, got: {}",
        stdout
    );
}

#[test]
fn test_cli_parse_no_comments() {
    ensure_qmdc_built();

    let mut child = Command::new(get_qmdc_path())
        .args(["parse", "--no-comments"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to spawn qmdc");

    {
        let stdin = child.stdin.as_mut().unwrap();
        stdin
            .write_all(b"## Test [[test]]\n\n- name: Alice\n\nThis is a comment.")
            .unwrap();
    }

    let output = child.wait_with_output().expect("Failed to read output");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let result: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("Failed to parse JSON output");

    for obj in result.as_array().unwrap() {
        assert!(
            obj.get("__comments").is_none(),
            "Object {} should not have __comments",
            obj.get("__id").unwrap_or(&serde_json::json!("unknown"))
        );
    }
}

#[test]
fn test_cli_parse_no_pretty() {
    ensure_qmdc_built();

    let mut child = Command::new(get_qmdc_path())
        .args(["parse", "--no-pretty"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to spawn qmdc");

    {
        let stdin = child.stdin.as_mut().unwrap();
        stdin.write_all(b"## Test [[test]]").unwrap();
    }

    let output = child.wait_with_output().expect("Failed to read output");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let result: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("Failed to parse JSON output");

    assert!(result.is_array());
    let arr = result.as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["__id"], "test");
}

#[test]
fn test_cli_format_minimal() {
    ensure_qmdc_built();

    let mut child = Command::new(get_qmdc_path())
        .args(["parse", "-f", "minimal"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to spawn qmdc");

    {
        let stdin = child.stdin.as_mut().unwrap();
        stdin
            .write_all(b"## Test [[test]]\n\n- name: Alice")
            .unwrap();
    }

    let output = child.wait_with_output().expect("Failed to read output");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let result: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("Failed to parse JSON output");

    let arr = result.as_array().unwrap();
    assert_eq!(arr.len(), 1);
    // In minimal format, __level and __label should be absent
    assert!(
        arr[0].get("__level").is_none(),
        "Minimal should not have __level"
    );
    assert!(
        arr[0].get("__label").is_none(),
        "Minimal should not have __label"
    );
    // But should have data field
    assert_eq!(arr[0]["name"], "Alice");
}

/// CLI must detect a workspace whose __Workspace kind has a space after the colon.
/// `[[id: __Workspace]]` (spaced) is valid QMD and must be found by
/// `workspace parse` (exit 0), same as the unspaced form.
#[test]
fn test_cli_workspace_parse_spaced_kind() {
    ensure_qmdc_built();

    let tmpdir = TempDir::new().expect("Failed to create temp dir");
    let readme = tmpdir.path().join("readme.qmd.md");
    fs::write(
        &readme,
        "# Spaced Project [[spaced_proj: __Workspace]]\n\n\
         - description: workspace with a space after the colon\n\n\
         ## Thing [[thing]]\n\n\
         - value: 1\n",
    )
    .expect("Failed to write readme.qmd.md");

    let output = Command::new(get_qmdc_path())
        .args(["workspace", "parse", tmpdir.path().to_str().unwrap()])
        .output()
        .expect("Failed to run qmdc workspace parse");

    assert!(
        output.status.success(),
        "spaced __Workspace should be detected, got exit {:?}: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let result: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("Failed to parse JSON output");

    assert_eq!(result["workspace"], "spaced_proj");
}
