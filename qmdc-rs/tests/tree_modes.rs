//! Data-driven tests for LSP tree modes (namespace, file, smart)
//! Automatically discovers test workspaces and expected files
#![allow(clippy::get_first)]

use qmdc::core::tree::{get_tree_by_file, get_tree_by_namespace, get_tree_by_smart};
use qmdc::db::QmdcDatabase;
use qmdc::{parse_all_workspaces, OutputFormat};
use std::fs;
use std::path::{Path, PathBuf};

/// Get paths to scan for test workspaces
fn get_scan_paths() -> Vec<PathBuf> {
    let project_root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    vec![project_root.join("tests/workspace")]
}

/// Recursively find all directories containing tests/tree-modes/ with .expected.json files
fn find_tree_mode_test_workspaces(dir: &Path, prefix: &str) -> Vec<(String, PathBuf)> {
    let mut workspaces = Vec::new();

    if !dir.exists() {
        return workspaces;
    }

    let mut entries: Vec<_> = fs::read_dir(dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();

    entries.sort();

    for entry in entries {
        let name = entry.file_name().unwrap().to_string_lossy().to_string();
        let workspace_name = if prefix.is_empty() {
            name.clone()
        } else {
            format!("{}/{}", prefix, name)
        };

        let tree_modes_dir = entry.join("tests/tree-modes");

        // Check if this directory has tests/tree-modes/ with expected files
        if tree_modes_dir.exists() && tree_modes_dir.is_dir() {
            let has_expected = fs::read_dir(&tree_modes_dir)
                .map(|rd| {
                    rd.filter_map(|e| e.ok()).any(|e| {
                        e.path()
                            .file_name()
                            .and_then(|n| n.to_str())
                            .map(|s| s.ends_with(".expected.json"))
                            .unwrap_or(false)
                    })
                })
                .unwrap_or(false);

            if has_expected {
                workspaces.push((workspace_name.clone(), entry.clone()));
            }
        }

        // Also check subdirectories (but not tests/ itself)
        if name != "tests" {
            workspaces.extend(find_tree_mode_test_workspaces(&entry, &workspace_name));
        }
    }

    workspaces
}

/// Get expected files for tree modes
fn get_tree_mode_expected_files(workspace_path: &Path) -> Vec<(String, PathBuf)> {
    let tree_modes_dir = workspace_path.join("tests/tree-modes");
    let mut expected_files = Vec::new();

    if !tree_modes_dir.exists() {
        return expected_files;
    }

    let modes = ["namespace", "file", "smart"];

    for mode in &modes {
        let expected_file = tree_modes_dir.join(format!("{}.expected.json", mode));
        if expected_file.exists() {
            expected_files.push((mode.to_string(), expected_file));
        }
    }

    expected_files
}

/// Deep comparison of JSON values
fn deep_equal(a: &serde_json::Value, b: &serde_json::Value) -> bool {
    use serde_json::Value;
    match (a, b) {
        (Value::Object(a_map), Value::Object(b_map)) => {
            if a_map.len() != b_map.len() {
                return false;
            }
            let a_keys: Vec<_> = a_map.keys().collect();
            let b_keys: Vec<_> = b_map.keys().collect();
            if a_keys != b_keys {
                return false;
            }
            for key in a_keys {
                if !deep_equal(&a_map[key], &b_map[key]) {
                    return false;
                }
            }
            true
        }
        (Value::Array(a_arr), Value::Array(b_arr)) => {
            if a_arr.len() != b_arr.len() {
                return false;
            }
            a_arr
                .iter()
                .zip(b_arr.iter())
                .all(|(a, b)| deep_equal(a, b))
        }
        _ => a == b,
    }
}

fn run_tree_mode_tests(workspace_path: &Path, workspace_name: &str) -> (usize, usize) {
    let objects = parse_all_workspaces(workspace_path, OutputFormat::Full).objects;

    let db = QmdcDatabase::new().unwrap();
    db.sync_objects_from_vec(&objects).unwrap();

    let expected_files = get_tree_mode_expected_files(workspace_path);

    if expected_files.is_empty() {
        return (0, 0);
    }

    println!(
        "\n=== {} ({} tree mode tests) ===\n",
        workspace_name,
        expected_files.len()
    );

    let mut passed = 0;
    let mut failed = 0;

    for (mode, expected_file) in &expected_files {
        print!("  {} ... ", mode);

        let expected: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(expected_file)
                .unwrap_or_else(|e| panic!("Failed to read {}: {}", expected_file.display(), e)),
        )
        .unwrap_or_else(|e| panic!("Failed to parse {}: {}", expected_file.display(), e));

        let result = match mode.as_str() {
            "namespace" => get_tree_by_namespace(&db),
            "file" => get_tree_by_file(&db),
            "smart" => get_tree_by_smart(&db),
            _ => {
                println!("FAILED (unknown mode)");
                failed += 1;
                continue;
            }
        };

        let actual_full = match result {
            Ok(Some(v)) => v,
            Ok(None) => {
                println!("FAILED (no result)");
                failed += 1;
                continue;
            }
            Err(e) => {
                println!("FAILED (error: {})", e);
                failed += 1;
                continue;
            }
        };

        // Extract workspace(s) from actual
        // If expected is an array, compare against full workspaces array (multi-workspace test)
        // If expected is an object, compare against workspaces[0] (single workspace test)
        let actual_workspaces = actual_full
            .get("workspaces")
            .and_then(|w| w.as_array())
            .expect("No workspaces in result");

        // Bless mode: regenerate expected files instead of comparing. Scoped by
        // UPDATE_EXPECTED — "1"/"all" blesses every workspace, otherwise only those
        // whose name contains the value (e.g. UPDATE_EXPECTED=tree-order-casefold).
        if let Ok(filter) = std::env::var("UPDATE_EXPECTED") {
            if filter == "1" || filter == "all" || workspace_name.contains(&filter) {
                let to_write = if expected.is_array() {
                    serde_json::Value::Array(actual_workspaces.clone())
                } else {
                    actual_workspaces
                        .first()
                        .cloned()
                        .unwrap_or(serde_json::Value::Null)
                };
                fs::write(
                    expected_file,
                    serde_json::to_string_pretty(&to_write).unwrap() + "\n",
                )
                .unwrap();
                println!("blessed");
                passed += 1;
                continue;
            }
        }

        let matches = if expected.is_array() {
            // Multi-workspace: compare full array
            deep_equal(
                &serde_json::Value::Array(actual_workspaces.clone()),
                &expected,
            )
        } else {
            // Single workspace: compare workspaces[0]
            actual_workspaces
                .first()
                .map(|actual| deep_equal(actual, &expected))
                .unwrap_or(false)
        };

        if matches {
            println!("ok");
            passed += 1;
        } else {
            println!("FAILED");
            eprintln!("\n=== Mode '{}' FAILED ===", mode);
            eprintln!("\n=== EXPECTED ===");
            eprintln!("{}", serde_json::to_string_pretty(&expected).unwrap());
            eprintln!("\n=== ACTUAL ===");
            if expected.is_array() {
                eprintln!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::Value::Array(
                        actual_workspaces.clone()
                    ))
                    .unwrap()
                );
            } else {
                eprintln!(
                    "{}",
                    serde_json::to_string_pretty(
                        &actual_workspaces
                            .first()
                            .unwrap_or(&serde_json::Value::Null)
                    )
                    .unwrap()
                );
            }
            failed += 1;
        }
    }

    (passed, failed)
}

#[test]
fn test_all_tree_modes_data_driven() {
    let mut total_passed = 0;
    let mut total_failed = 0;

    // Find all test workspaces from scan paths
    for scan_path in get_scan_paths() {
        let workspaces = find_tree_mode_test_workspaces(&scan_path, "");

        for (workspace_name, workspace_path) in workspaces {
            let (p, f) = run_tree_mode_tests(&workspace_path, &workspace_name);
            total_passed += p;
            total_failed += f;
        }
    }

    eprintln!(
        "\n✓ {} tree mode tests passed ({} failed)\n",
        total_passed, total_failed
    );

    if total_failed > 0 {
        panic!("{} tree mode test(s) failed", total_failed);
    }

    if total_passed == 0 {
        eprintln!(
            "⚠ No tree mode tests found (expected files in tests/tree-modes/*.expected.json)"
        );
    }
}
