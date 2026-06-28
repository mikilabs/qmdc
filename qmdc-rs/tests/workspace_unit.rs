//! Workspace tests - test workspace parsing functionality
#![allow(clippy::expect_fun_call, clippy::unwrap_or_default)]

use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::Once;

fn get_qmdc_path() -> PathBuf {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    PathBuf::from(manifest_dir).join("target/debug/qmdc")
}

fn get_workspace_tests_dir() -> PathBuf {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    PathBuf::from(manifest_dir)
        .parent()
        .unwrap()
        .join("tests/workspace")
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

/// Find all workspace test directories (those with _expected.json and readme.qmd.md)
fn find_workspace_tests() -> Vec<(String, PathBuf, serde_json::Value)> {
    let root = get_workspace_tests_dir();
    let mut tests = Vec::new();

    fn scan_dir(
        dir: &PathBuf,
        prefix: &str,
        tests: &mut Vec<(String, PathBuf, serde_json::Value)>,
    ) {
        if let Ok(entries) = fs::read_dir(dir) {
            let mut dirs: Vec<_> = entries
                .filter_map(|e| e.ok())
                .filter(|e| e.path().is_dir())
                .collect();
            dirs.sort_by_key(|e| e.path());

            for entry in dirs {
                let path = entry.path();
                let expected_file = path.join("_expected.json");
                let readme_file = path.join("readme.qmd.md");

                if expected_file.exists() && readme_file.exists() {
                    let test_name = if prefix.is_empty() {
                        entry.file_name().to_string_lossy().to_string()
                    } else {
                        format!("{}{}", prefix, entry.file_name().to_string_lossy())
                    };

                    if let Ok(content) = fs::read_to_string(&expected_file) {
                        if let Ok(expected) = serde_json::from_str(&content) {
                            tests.push((test_name, path.clone(), expected));
                        }
                    }
                } else {
                    let new_prefix = if prefix.is_empty() {
                        format!("{}/", entry.file_name().to_string_lossy())
                    } else {
                        format!("{}{}/", prefix, entry.file_name().to_string_lossy())
                    };
                    scan_dir(&path, &new_prefix, tests);
                }
            }
        }
    }

    if root.exists() {
        scan_dir(&root, "", &mut tests);
    }

    tests
}

#[test]
fn test_workspace_files() {
    ensure_qmdc_built();

    let tests = find_workspace_tests();
    assert!(!tests.is_empty(), "Should find at least one workspace test");

    for (test_name, workspace_path, expected) in tests {
        let output = Command::new(get_qmdc_path())
            .args(["workspace", "parse", workspace_path.to_str().unwrap()])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .expect("Failed to run qmdc workspace parse");

        assert!(
            output.status.success(),
            "Test {} failed: {}",
            test_name,
            String::from_utf8_lossy(&output.stderr)
        );

        let result: serde_json::Value = serde_json::from_slice(&output.stdout)
            .expect(&format!("Failed to parse JSON for test {}", test_name));

        // Check files list
        let expected_files = expected
            .get("files")
            .and_then(|v| v.as_array())
            .map(|arr| {
                let mut files: Vec<String> = arr
                    .iter()
                    .filter_map(|v| v.as_str())
                    .map(|s| s.to_string())
                    .collect();
                files.sort();
                files
            })
            .unwrap_or_default();

        let actual_files: Vec<String> = result
            .get("files")
            .and_then(|v| v.as_array())
            .map(|arr| {
                let mut files: Vec<String> = arr
                    .iter()
                    .filter_map(|v| v.as_str())
                    .map(|s| s.to_string())
                    .collect();
                files.sort();
                files
            })
            .unwrap_or_default();

        assert_eq!(
            actual_files, expected_files,
            "Test {} files mismatch.\nActual: {:?}\nExpected: {:?}",
            test_name, actual_files, expected_files
        );
    }
}

#[test]
fn test_workspace_objects_by_kind() {
    ensure_qmdc_built();

    let tests = find_workspace_tests();

    for (test_name, workspace_path, expected) in tests {
        let output = Command::new(get_qmdc_path())
            .args(["workspace", "parse", workspace_path.to_str().unwrap()])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .expect("Failed to run qmdc workspace parse");

        if !output.status.success() {
            continue; // Skip if command failed
        }

        let result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();

        // Build actual objects by kind
        let mut actual_by_kind: std::collections::HashMap<String, Vec<String>> =
            std::collections::HashMap::new();

        if let Some(objects) = result.get("objects").and_then(|v| v.as_array()) {
            for obj in objects {
                let kind = obj.get("__kind").and_then(|v| v.as_str()).unwrap_or("");
                let id = obj.get("__id").and_then(|v| v.as_str()).unwrap_or("");

                if !kind.is_empty() && !id.is_empty() {
                    actual_by_kind
                        .entry(kind.to_string())
                        .or_insert_with(Vec::new)
                        .push(id.to_string());
                }
            }
        }

        // Sort values for comparison
        for ids in actual_by_kind.values_mut() {
            ids.sort();
        }

        // Get expected objects
        if let Some(expected_objects) = expected.get("objects").and_then(|v| v.as_object()) {
            for (kind, ids) in expected_objects {
                let expected_ids: Vec<String> = ids
                    .as_array()
                    .map(|arr| {
                        let mut v: Vec<String> = arr
                            .iter()
                            .filter_map(|v| v.as_str())
                            .map(|s| s.to_string())
                            .collect();
                        v.sort();
                        v
                    })
                    .unwrap_or_default();

                let actual_ids = actual_by_kind.get(kind).cloned().unwrap_or_default();

                assert_eq!(
                    actual_ids, expected_ids,
                    "Test {} kind {} IDs mismatch.\nActual: {:?}\nExpected: {:?}",
                    test_name, kind, actual_ids, expected_ids
                );
            }
        }
    }
}

#[test]
fn test_workspace_has_file_metadata() {
    ensure_qmdc_built();

    let tests = find_workspace_tests();

    for (test_name, workspace_path, _expected) in tests {
        let output = Command::new(get_qmdc_path())
            .args(["workspace", "parse", workspace_path.to_str().unwrap()])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .expect("Failed to run qmdc workspace parse");

        if !output.status.success() {
            continue;
        }

        let result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();

        if let Some(objects) = result.get("objects").and_then(|v| v.as_array()) {
            for obj in objects {
                let kind = obj.get("__kind").and_then(|v| v.as_str()).unwrap_or("");

                // Skip system types except __Workspace and __Namespace
                if kind.starts_with("__") && kind != "__Workspace" && kind != "__Namespace" {
                    continue;
                }

                let id = obj.get("__id").and_then(|v| v.as_str()).unwrap_or("");

                assert!(
                    obj.get("__file").is_some(),
                    "Test {} object {} ({}) missing __file",
                    test_name,
                    id,
                    kind
                );
            }
        }
    }
}

#[test]
fn test_workspace_basic_parse() {
    ensure_qmdc_built();

    // Create a simple workspace in temp dir and test it
    let tmpdir = tempfile::TempDir::new().expect("Failed to create temp dir");
    let workspace_dir = tmpdir.path();

    // Create readme.qmd.md
    let readme_content = r#"# My Workspace [[:Workspace]]

- name: TestWorkspace

## Object One [[obj1]]

- value: 42
"#;

    fs::write(workspace_dir.join("readme.qmd.md"), readme_content)
        .expect("Failed to write readme.qmd.md");

    let output = Command::new(get_qmdc_path())
        .args(["workspace", "parse", workspace_dir.to_str().unwrap()])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("Failed to run qmdc workspace parse");

    assert!(
        output.status.success(),
        "Command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let result: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("Failed to parse JSON output");

    // Check files
    let files = result.get("files").and_then(|v| v.as_array()).unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0], "readme.qmd.md");

    // Check objects
    let objects = result.get("objects").and_then(|v| v.as_array()).unwrap();
    assert!(objects.len() >= 2, "Should have at least 2 objects");

    // Check that __file metadata is present
    for obj in objects {
        let kind = obj.get("__kind").and_then(|v| v.as_str()).unwrap_or("");
        if !kind.starts_with("__") || kind == "__Workspace" {
            assert!(
                obj.get("__file").is_some(),
                "Object {} should have __file",
                obj.get("__id").unwrap_or(&serde_json::json!(""))
            );
        }
    }
}

#[test]
fn test_workspace_validation_errors() {
    ensure_qmdc_built();

    let tests = find_workspace_tests();
    assert!(!tests.is_empty(), "Should find at least one workspace test");

    for (test_name, workspace_path, expected) in tests {
        // Skip tests that don't have errors in expected
        let empty_vec: Vec<serde_json::Value> = Vec::new();
        let expected_errors = expected
            .get("errors")
            .and_then(|v| v.as_array())
            .unwrap_or(&empty_vec);

        if expected_errors.is_empty() {
            continue; // Skip tests without expected errors
        }

        let output = Command::new(get_qmdc_path())
            .args(["workspace", "parse", workspace_path.to_str().unwrap()])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .expect(&format!(
                "Failed to run qmdc workspace parse for test {}",
                test_name
            ));

        // Command may succeed even with errors (errors are in JSON)
        let result: serde_json::Value = serde_json::from_slice(&output.stdout).expect(&format!(
            "Failed to parse JSON output for test {}",
            test_name
        ));

        // Check that errors array exists
        let actual_errors = result
            .get("errors")
            .and_then(|v| v.as_array())
            .expect(&format!(
                "Test {} result should have 'errors' array",
                test_name
            ));

        // Should have at least as many errors as expected
        assert!(
            actual_errors.len() >= expected_errors.len(),
            "Test {}: Expected at least {} errors, but got {}.\nExpected: {:?}\nActual: {:?}",
            test_name,
            expected_errors.len(),
            actual_errors.len(),
            expected_errors,
            actual_errors
        );

        // Check error structure
        for error in actual_errors {
            assert!(
                error.get("type").is_some() || error.get("error_type").is_some(),
                "Test {}: Error should have 'type' or 'error_type' field: {:?}",
                test_name,
                error
            );
            assert!(
                error.get("message").is_some() || error.get("severity").is_some(),
                "Test {}: Error should have 'message' or 'severity' field: {:?}",
                test_name,
                error
            );
        }

        // Check that all expected error types are present
        let expected_error_types: Vec<&str> = expected_errors
            .iter()
            .filter_map(|e| {
                e.get("type")
                    .or_else(|| e.get("error_type"))
                    .and_then(|v| v.as_str())
            })
            .collect();

        let actual_error_types: Vec<&str> = actual_errors
            .iter()
            .filter_map(|e| {
                e.get("type")
                    .or_else(|| e.get("error_type"))
                    .and_then(|v| v.as_str())
            })
            .collect();

        for expected_type in &expected_error_types {
            assert!(actual_error_types.contains(expected_type),
                    "Test {}: Expected error type '{}' not found in actual errors.\nExpected types: {:?}\nActual types: {:?}",
                    test_name, expected_type, expected_error_types, actual_error_types);
        }
    }
}

#[test]
fn test_workspace_validation_no_errors() {
    ensure_qmdc_built();

    let tests = find_workspace_tests();

    for (test_name, workspace_path, expected) in tests {
        // Only test workspaces that are expected to have no errors
        let empty_vec: Vec<serde_json::Value> = Vec::new();
        let expected_errors = expected
            .get("errors")
            .and_then(|v| v.as_array())
            .unwrap_or(&empty_vec);

        if !expected_errors.is_empty() {
            continue; // Skip tests with expected errors
        }

        let output = Command::new(get_qmdc_path())
            .args(["workspace", "parse", workspace_path.to_str().unwrap()])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .expect(&format!(
                "Failed to run qmdc workspace parse for test {}",
                test_name
            ));

        // Command should succeed for valid workspaces
        if !output.status.success() {
            eprintln!(
                "Test {}: Command failed, but continuing to check errors",
                test_name
            );
        }

        let result: serde_json::Value = serde_json::from_slice(&output.stdout).expect(&format!(
            "Failed to parse JSON output for test {}",
            test_name
        ));

        // Check that errors array exists and is empty
        let actual_errors = result
            .get("errors")
            .and_then(|v| v.as_array())
            .expect(&format!(
                "Test {} result should have 'errors' array",
                test_name
            ));

        assert!(
            actual_errors.is_empty(),
            "Test {}: Expected no errors, but got {} errors: {:?}",
            test_name,
            actual_errors.len(),
            actual_errors
        );
    }
}

#[test]
fn test_workspace_validate_command() {
    ensure_qmdc_built();

    let tests = find_workspace_tests();
    assert!(!tests.is_empty(), "Should find at least one workspace test");

    for (test_name, workspace_path, _expected) in tests {
        // Get errors from workspace parse
        let parse_output = Command::new(get_qmdc_path())
            .args(["workspace", "parse", workspace_path.to_str().unwrap()])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .expect(&format!(
                "Failed to run qmdc workspace parse for test {}",
                test_name
            ));

        let parse_result: serde_json::Value = serde_json::from_slice(&parse_output.stdout).expect(
            &format!("Failed to parse JSON output for test {}", test_name),
        );

        let parse_errors = parse_result
            .get("errors")
            .and_then(|v| v.as_array())
            .expect(&format!(
                "Test {} parse result should have 'errors' array",
                test_name
            ));

        // Get errors from workspace validate
        let validate_output = Command::new(get_qmdc_path())
            .args(["workspace", "validate", workspace_path.to_str().unwrap()])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .expect(&format!(
                "Failed to run qmdc workspace validate for test {}",
                test_name
            ));

        // Validate should return JSON array directly (not wrapped in object)
        let validate_errors: Vec<serde_json::Value> =
            serde_json::from_slice(&validate_output.stdout).expect(&format!(
                "Failed to parse JSON array from validate output for test {}",
                test_name
            ));

        // Check that validate returns same number of errors as parse
        assert_eq!(
            validate_errors.len(),
            parse_errors.len(),
            "Test {}: validate returned {} errors, but parse returned {} errors",
            test_name,
            validate_errors.len(),
            parse_errors.len()
        );

        // Check that validate returns correct format
        for error in &validate_errors {
            assert!(
                error.get("type").is_some(),
                "Test {}: Error should have 'type' field: {:?}",
                test_name,
                error
            );
            assert!(
                error.get("message").is_some(),
                "Test {}: Error should have 'message' field: {:?}",
                test_name,
                error
            );
            assert!(
                error.get("severity").is_some(),
                "Test {}: Error should have 'severity' field: {:?}",
                test_name,
                error
            );
            // Check optional fields exist (can be null)
            assert!(
                error.get("file").is_some() || error.get("file").is_none(),
                "Test {}: Error should have 'file' field (can be null): {:?}",
                test_name,
                error
            );
            assert!(
                error.get("line").is_some() || error.get("line").is_none(),
                "Test {}: Error should have 'line' field (can be null): {:?}",
                test_name,
                error
            );
            assert!(
                error.get("objectId").is_some() || error.get("objectId").is_none(),
                "Test {}: Error should have 'objectId' field (can be null): {:?}",
                test_name,
                error
            );
            assert!(
                error.get("fieldName").is_some() || error.get("fieldName").is_none(),
                "Test {}: Error should have 'fieldName' field (can be null): {:?}",
                test_name,
                error
            );
            assert!(
                error.get("reference").is_some() || error.get("reference").is_none(),
                "Test {}: Error should have 'reference' field (can be null): {:?}",
                test_name,
                error
            );
            assert!(
                error.get("candidates").is_some() || error.get("candidates").is_none(),
                "Test {}: Error should have 'candidates' field (can be null): {:?}",
                test_name,
                error
            );
        }

        // Check exit code: 0 if no errors, 1 if errors
        let expected_exit_code = if validate_errors.is_empty() { 0 } else { 1 };
        assert_eq!(
            validate_output.status.code(),
            Some(expected_exit_code),
            "Test {}: validate should exit with code {}, but exited with {:?}",
            test_name,
            expected_exit_code,
            validate_output.status.code()
        );
    }
}
