//! Data-driven SQL tests for QMD workspace
//!
//! Automatically discovers all directories with tests/ subdirectory containing .sql files.

use qmdc::db::QmdcDatabase;
use qmdc::{parse_all_workspaces, OutputFormat};
use std::fs;
use std::path::{Path, PathBuf};

mod common;

/// Load all workspaces from a directory into objects list.
fn load_workspaces(dir: &Path) -> Vec<serde_json::Value> {
    let result = parse_all_workspaces(dir, OutputFormat::Full);
    result.objects
}

/// Get paths to scan for test workspaces
fn get_scan_paths() -> Vec<PathBuf> {
    let project_root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    vec![project_root.join("tests/workspace")]
}

/// Get SQL tests from a workspace directory
fn get_sql_tests(workspace_path: &Path) -> Vec<(String, PathBuf, PathBuf)> {
    let tests_dir = workspace_path.join("tests");
    let mut tests = Vec::new();

    if !tests_dir.exists() {
        return tests;
    }

    let mut sql_files: Vec<_> = fs::read_dir(&tests_dir)
        .expect("Failed to read tests directory")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().map(|e| e == "sql").unwrap_or(false))
        .collect();

    sql_files.sort();

    for sql_file in sql_files {
        let file_name = sql_file.file_stem().unwrap().to_string_lossy().to_string();
        let expected_file = tests_dir.join(format!("{}.expected.json", file_name));

        if expected_file.exists() {
            tests.push((file_name, sql_file, expected_file));
        }
    }

    tests
}

/// Recursively find all directories containing tests/ with .sql files
fn find_test_workspaces(dir: &Path, prefix: &str) -> Vec<(String, PathBuf)> {
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

        let tests_dir = entry.join("tests");

        // Check if this directory has tests/
        if tests_dir.exists() && tests_dir.is_dir() {
            let has_sql = fs::read_dir(&tests_dir)
                .map(|rd| {
                    rd.filter_map(|e| e.ok()).any(|e| {
                        e.path()
                            .extension()
                            .map(|ext| ext == "sql")
                            .unwrap_or(false)
                    })
                })
                .unwrap_or(false);

            if has_sql {
                workspaces.push((workspace_name.clone(), entry.clone()));
            }
        }

        // Also check subdirectories (but not tests/ itself)
        if name != "tests" {
            workspaces.extend(find_test_workspaces(&entry, &workspace_name));
        }
    }

    workspaces
}

/// Expected result structure
#[derive(Debug, serde::Deserialize)]
struct ExpectedResult {
    columns: Vec<String>,
    rows: Vec<Vec<serde_json::Value>>,
}

fn run_sql_tests(
    workspace_path: &Path,
    workspace_name: &str,
    report: &mut common::CaseReport,
) -> (usize, usize) {
    let objects = load_workspaces(workspace_path);

    let db = QmdcDatabase::new().unwrap();
    db.sync_objects_from_vec(&objects).unwrap();

    let tests = get_sql_tests(workspace_path);

    if tests.is_empty() {
        return (0, 0);
    }

    println!("\n=== {} ({} SQL tests) ===\n", workspace_name, tests.len());

    let mut passed = 0;
    let mut failed = 0;

    for (name, sql_file, expected_file) in &tests {
        let case_t = std::time::Instant::now();
        let case_id = format!("{}/{}", workspace_name, name);
        print!("  {} ... ", name);

        let sql = fs::read_to_string(sql_file)
            .unwrap_or_else(|e| panic!("Failed to read {}: {}", sql_file.display(), e));

        let expected: ExpectedResult = serde_json::from_str(
            &fs::read_to_string(expected_file)
                .unwrap_or_else(|e| panic!("Failed to read {}: {}", expected_file.display(), e)),
        )
        .unwrap_or_else(|e| panic!("Failed to parse {}: {}", expected_file.display(), e));

        match db.query(&sql) {
            Ok(result) => {
                if result.columns != expected.columns {
                    println!("FAILED");
                    println!("    Columns mismatch:");
                    println!("      expected: {:?}", expected.columns);
                    println!("      actual:   {:?}", result.columns);
                    report.fail(&case_id, "columns mismatch", case_t.elapsed().as_secs_f64());
                    failed += 1;
                    continue;
                }

                if result.rows != expected.rows {
                    println!("FAILED");
                    println!("    Rows mismatch:");
                    println!("      expected: {:?}", expected.rows);
                    println!("      actual:   {:?}", result.rows);
                    println!("    Full result:");
                    println!("{}", result.to_table_string());
                    report.fail(&case_id, "rows mismatch", case_t.elapsed().as_secs_f64());
                    failed += 1;
                    continue;
                }

                println!("ok");
                report.pass(&case_id, case_t.elapsed().as_secs_f64());
                passed += 1;
            }
            Err(e) => {
                println!("FAILED");
                println!("    SQL error: {}", e);
                report.fail(&case_id, "sql error", case_t.elapsed().as_secs_f64());
                failed += 1;
            }
        }
    }

    (passed, failed)
}

#[test]
fn test_all_sql_queries() {
    let mut total_passed = 0;
    let mut total_failed = 0;
    let mut report = common::CaseReport::new("sql", "rs-sql");

    // Find all test workspaces from scan paths
    for scan_path in get_scan_paths() {
        let workspaces = find_test_workspaces(&scan_path, "");

        for (workspace_name, workspace_path) in workspaces {
            let (p, f) = run_sql_tests(&workspace_path, &workspace_name, &mut report);
            total_passed += p;
            total_failed += f;
        }
    }

    eprintln!(
        "\n✓ {} SQL workspace tests passed ({} failed)\n",
        total_passed, total_failed
    );

    report.finish();
}

/// Test that SQL errors are handled properly
#[test]
fn test_sql_error_handling() {
    let db = QmdcDatabase::new().unwrap();

    let result = db.query("SELECT * FROM nonexistent_table");
    assert!(result.is_err(), "Expected error for invalid table");

    let result = db.query("INVALID SQL SYNTAX");
    assert!(result.is_err(), "Expected error for invalid syntax");
}

/// Expected object in workspace
#[derive(Debug, serde::Deserialize)]
struct ExpectedObject {
    __id: String,
    __kind: String,
    __label: String,
}

#[derive(Debug, serde::Deserialize)]
struct ExpectedObjects {
    objects: Vec<ExpectedObject>,
}

/// Expected edge in workspace  
#[derive(Debug, serde::Deserialize)]
struct ExpectedEdge {
    source_id: String,
    source_field: String,
    target_id: String,
    #[serde(default)]
    edge_type: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
struct ExpectedEdges {
    edges: Vec<ExpectedEdge>,
}

fn get_test_workspace_path() -> PathBuf {
    get_scan_paths()[0].join("test-workspace")
}

#[test]
fn test_workspace_objects_match_expected() {
    let workspace_path = get_test_workspace_path();
    let expected_file = workspace_path.join("expected.objects.json");

    if !expected_file.exists() {
        panic!("expected.objects.json not found");
    }

    let expected: ExpectedObjects =
        serde_json::from_str(&fs::read_to_string(&expected_file).unwrap()).unwrap();

    let objects = load_workspaces(&workspace_path);
    let db = QmdcDatabase::new().unwrap();
    db.sync_objects_from_vec(&objects).unwrap();

    let result = db
        .query(
            "SELECT __id, __kind, __label FROM objects 
         WHERE __kind NOT IN ('__Document', '__TextBlock') 
         ORDER BY __id",
        )
        .unwrap();

    let actual: Vec<(String, String, String)> = result
        .rows
        .iter()
        .map(|row| {
            (
                row[0].as_str().unwrap_or("").to_string(),
                row[1].as_str().unwrap_or("").to_string(),
                row[2].as_str().unwrap_or("").to_string(),
            )
        })
        .collect();

    let mut expected_sorted: Vec<_> = expected
        .objects
        .iter()
        .map(|o| (o.__id.clone(), o.__kind.clone(), o.__label.clone()))
        .collect();
    expected_sorted.sort();

    if actual != expected_sorted {
        println!("\nObjects mismatch!");
        println!("\nExpected ({}):", expected_sorted.len());
        for (id, kind, label) in &expected_sorted {
            println!("  {} [{}] {}", id, kind, label);
        }
        println!("\nActual ({}):", actual.len());
        for (id, kind, label) in &actual {
            println!("  {} [{}] {}", id, kind, label);
        }
        panic!("Objects don't match expected.objects.json");
    }

    println!("✓ {} objects match expected", actual.len());
}

#[test]
fn test_workspace_edges_match_expected() {
    let workspace_path = get_test_workspace_path();
    let expected_file = workspace_path.join("expected.edges.json");

    if !expected_file.exists() {
        panic!("expected.edges.json not found");
    }

    let expected: ExpectedEdges =
        serde_json::from_str(&fs::read_to_string(&expected_file).unwrap()).unwrap();

    let objects = load_workspaces(&workspace_path);
    let db = QmdcDatabase::new().unwrap();
    db.sync_objects_from_vec(&objects).unwrap();

    // Use JOIN to get __id from __global_id (edges.source_id/target_id contain __global_id)
    let result = db
        .query(
            "SELECT s.__id, e.source_field, t.__id, e.edge_type 
         FROM edges e
         JOIN objects s ON e.source_id = s.__global_id
         JOIN objects t ON e.target_id = t.__global_id
         WHERE s.__id NOT LIKE 'doc_%' AND s.__id NOT LIKE 'text_%'
         ORDER BY s.__id, e.source_field, t.__id, e.edge_type",
        )
        .unwrap();

    let actual: Vec<(String, String, String, String)> = result
        .rows
        .iter()
        .map(|row| {
            (
                row[0].as_str().unwrap_or("").to_string(),
                row[1].as_str().unwrap_or("").to_string(),
                row[2].as_str().unwrap_or("").to_string(),
                row[3].as_str().unwrap_or("").to_string(),
            )
        })
        .collect();

    let mut expected_sorted: Vec<_> = expected
        .edges
        .iter()
        .map(|e| {
            let edge_type = e
                .edge_type
                .clone()
                .unwrap_or_else(|| e.source_field.clone());
            (
                e.source_id.clone(),
                e.source_field.clone(),
                e.target_id.clone(),
                edge_type,
            )
        })
        .collect();
    expected_sorted.sort();

    if actual != expected_sorted {
        println!("\nEdges mismatch!");
        println!("\nExpected ({}):", expected_sorted.len());
        for (src, field, tgt, etype) in &expected_sorted {
            println!("  {} -[{}]-> {} (type: {})", src, field, tgt, etype);
        }
        println!("\nActual ({}):", actual.len());
        for (src, field, tgt, etype) in &actual {
            println!("  {} -[{}]-> {} (type: {})", src, field, tgt, etype);
        }
        panic!("Edges don't match expected.edges.json");
    }

    println!("✓ {} edges match expected", actual.len());
}

#[test]
fn test_parse_all_workspaces() {
    let test_dir = get_scan_paths()[0].clone();

    println!("\n=== Testing parse_all_workspaces ===");
    println!("Test directory: {}", test_dir.display());

    let case_t = std::time::Instant::now();
    let result = parse_all_workspaces(&test_dir, OutputFormat::Standard);

    let workspace_count = result
        .objects
        .iter()
        .filter(|obj| obj.get("__kind").and_then(|v| v.as_str()) == Some("__Workspace"))
        .count();

    println!("Found {} workspace objects", workspace_count);

    assert!(
        workspace_count >= 3,
        "Expected at least 3 workspaces, found {}",
        workspace_count
    );

    let workspace_ids: Vec<String> = result
        .objects
        .iter()
        .filter(|obj| obj.get("__kind").and_then(|v| v.as_str()) == Some("__Workspace"))
        .filter_map(|obj| {
            obj.get("__id")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        })
        .collect();

    println!("Workspace IDs: {:?}", workspace_ids);

    for expected in ["ecommerce", "backend", "frontend"] {
        assert!(
            workspace_ids.contains(&expected.to_string()),
            "Should find '{}' workspace",
            expected
        );
    }

    println!("✓ parse_all_workspaces test passed");

    // Conformance: count as one shared `sql` case (matches py/ts parse_all sanity).
    let mut report = common::CaseReport::new("sql", "rs-sql-parseall");
    report.pass("parse_all_workspaces", case_t.elapsed().as_secs_f64());
    report.finish();
}

/// QMD-34: Test that objects with same __id from different workspaces don't collide
///
/// Currently FAILING - this is the bug we're fixing.
/// After fix: both objects should exist in DB with different __workspace values.
#[test]
fn test_qmd34_multi_workspace_id_collision() {
    let test_dir = get_scan_paths()[0].join("multi-workspace-collision");

    if !test_dir.exists() {
        println!("Skipping test - multi-workspace-collision directory not found");
        return;
    }

    println!("\n=== QMD-34: Testing multi-workspace ID collision ===");
    println!("Test directory: {}", test_dir.display());

    // Parse all workspaces in the test directory
    let result = parse_all_workspaces(&test_dir, OutputFormat::Full);

    // Create DB and sync objects (using new method that handles workspace correctly)
    let db = QmdcDatabase::new().expect("Failed to create DB");

    // Use sync_objects_from_vec which properly handles workspace in PRIMARY KEY
    db.sync_objects_from_vec(&result.objects)
        .expect("Failed to sync objects");

    // Query: how many task_workflow objects exist?
    let query_result = db.query(
        "SELECT __id, __workspace FROM objects WHERE __id = 'task_workflow' ORDER BY __workspace"
    ).expect("Query failed");

    println!("Found {} task_workflow objects", query_result.rows.len());
    for row in &query_result.rows {
        let id = row[0].as_str().unwrap_or("?");
        let ws = row[1].as_str().unwrap_or("?");
        println!("  - __id={}, __workspace={}", id, ws);
    }

    // EXPECTED: 3 objects (two from ws1, one from ws2)
    // ACTUAL (BUG): 1 object (second overwrites first)
    assert_eq!(
        query_result.rows.len(),
        3,
        "BUG QMD-34: Objects with same __id from different workspaces should NOT collide! \
         Expected 2 task_workflow objects, found {}. \
         Fix: global_id should include __workspace.",
        query_result.rows.len()
    );

    println!("✓ QMD-34 test passed - no collision detected");
}
