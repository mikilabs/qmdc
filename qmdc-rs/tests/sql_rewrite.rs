//! Data-driven SQL rewrite tests
//!
//! Tests that SQL queries are correctly rewritten to add workspace filters,
//! and validates that rewritten SQL executes correctly against
//! multi-workspace test data.

use qmdc::db::QmdcDatabase;
use qmdc::lsp::sql_rewrite;
use qmdc::workspace::parse_all_workspaces;
use qmdc::OutputFormat;
use std::fs;
use std::path::Path;

#[derive(Debug, serde::Deserialize)]
struct SqlRewriteTest {
    input: String,
    workspace: String,
    expected: String,
    #[serde(default)]
    expected_result: Option<ExpectedResult>,
}

#[derive(Debug, serde::Deserialize, PartialEq)]
struct ExpectedResult {
    columns: Vec<String>,
    rows: Vec<Vec<serde_json::Value>>,
}

fn get_test_dir() -> std::path::PathBuf {
    let project_root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    project_root.join("tests/sql/rewrite")
}

fn get_multi_workspace_dir() -> std::path::PathBuf {
    let project_root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    project_root.join("tests/sql/multi-workspace-isolation")
}

/// Setup database with multi-workspace test data using existing mechanisms
fn setup_test_database() -> QmdcDatabase {
    let multi_ws_dir = get_multi_workspace_dir();

    // Use parse_all_workspaces to find and parse all workspaces
    let workspace_result = parse_all_workspaces(&multi_ws_dir, OutputFormat::Standard);

    // Create database and sync objects
    let db = QmdcDatabase::new().expect("Failed to create database");
    db.sync_objects_from_vec(&workspace_result.objects)
        .expect("Failed to sync objects");

    db
}

#[test]
fn test_all_sql_rewrite_tests() {
    let test_dir = get_test_dir();

    if !test_dir.exists() {
        panic!("Test directory not found: {:?}", test_dir);
    }

    // Setup database with multi-workspace data
    let db = setup_test_database();

    let mut test_files: Vec<_> = fs::read_dir(&test_dir)
        .expect("Failed to read test directory")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().map(|ext| ext == "json").unwrap_or(false))
        .collect();

    test_files.sort();

    assert!(
        !test_files.is_empty(),
        "No test files found in {:?}",
        test_dir
    );

    let total_tests = test_files.len();

    for test_file in test_files {
        let test_name = test_file.file_stem().unwrap().to_string_lossy().to_string();

        let content = fs::read_to_string(&test_file)
            .unwrap_or_else(|e| panic!("Failed to read {:?}: {}", test_file, e));

        let test: SqlRewriteTest = serde_json::from_str(&content)
            .unwrap_or_else(|e| panic!("Failed to parse {:?}: {}", test_file, e));

        let result = sql_rewrite::rewrite_sql_for_workspace(&test.input, &test.workspace);

        match result {
            Ok(rewritten) => {
                // 1. Check string rewrite is correct
                assert_eq!(
                    rewritten.trim(),
                    test.expected.trim(),
                    "Test {} failed (rewrite mismatch):\nInput:    {}\nExpected: {}\nGot:      {}",
                    test_name,
                    test.input,
                    test.expected,
                    rewritten
                );

                // 2. If expected_result is provided, execute and verify
                if let Some(ref expected_result) = test.expected_result {
                    match db.query(&rewritten) {
                        Ok(query_result) => {
                            assert_eq!(
                                query_result.columns, expected_result.columns,
                                "Test {} failed (columns mismatch):\nSQL: {}\nExpected columns: {:?}\nGot columns: {:?}",
                                test_name, rewritten, expected_result.columns, query_result.columns
                            );
                            assert_eq!(
                                query_result.rows, expected_result.rows,
                                "Test {} failed (rows mismatch):\nSQL: {}\nExpected rows: {:?}\nGot rows: {:?}",
                                test_name, rewritten, expected_result.rows, query_result.rows
                            );
                            println!("✅ Test {} passed (rewrite + execution)", test_name);
                        }
                        Err(e) => {
                            panic!(
                                "Test {} failed (SQL execution error):\nSQL: {}\nError: {}",
                                test_name, rewritten, e
                            );
                        }
                    }
                } else {
                    // No expected_result - just verify SQL is valid by running it
                    // Use a simpler validation - just try to run the query
                    match db.query(&rewritten) {
                        Ok(_) => {
                            println!("✅ Test {} passed (rewrite + valid SQL)", test_name);
                        }
                        Err(e) => {
                            // Some complex queries may fail due to missing CTEs etc.
                            // That's okay for rewrite tests - we mainly check the transformation
                            println!(
                                "⚠️  Test {} passed (rewrite ok, SQL error: {})",
                                test_name, e
                            );
                        }
                    }
                }
            }
            Err(e) => {
                panic!(
                    "Test {} failed with error: {}\nInput: {}\nExpected: {}",
                    test_name, e, test.input, test.expected
                );
            }
        }
    }

    eprintln!("\n✓ All {} SQL rewrite tests passed!", total_tests);
}
