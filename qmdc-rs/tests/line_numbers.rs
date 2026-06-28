//! Tests for correct line number tracking in workspace parsing

use qmdc::db::QmdcDatabase;
use qmdc::{parse_all_workspaces, OutputFormat};
use std::fs;
use std::path::Path;
use tempfile::TempDir;

fn create_test_workspace() -> TempDir {
    let dir = TempDir::new().unwrap();

    let readme = "# Test Workspace [[test_ws: __Workspace]]\n\nDescription.\n\n## Namespace [[ns: __Namespace]]\n\nNamespace description.\n";
    fs::write(dir.path().join("readme.qmd.md"), readme).unwrap();

    let objects = "# Objects\n\n## First [[obj1: Feature]]\n\n- status: planned\n\nDesc.\n\n## Second [[obj2: Feature]]\n\n- status: done\n\nDesc.\n";
    fs::write(dir.path().join("objects.qmd.md"), objects).unwrap();

    dir
}

#[test]
fn test_line_numbers_not_all_one() {
    let workspace = create_test_workspace();
    let objects = parse_all_workspaces(workspace.path(), OutputFormat::Full).objects;

    let db = QmdcDatabase::new().unwrap();
    db.sync_objects_from_vec(&objects).unwrap();

    let result = db
        .query("SELECT __id, __line FROM objects WHERE __kind NOT IN ('__Document', '__TextBlock')")
        .unwrap();

    let all_lines: Vec<i64> = result
        .rows
        .iter()
        .filter_map(|row| row[1].as_i64())
        .collect();

    let all_ones = all_lines.iter().all(|&l| l == 1);
    assert!(!all_ones, "Bug: all objects have line=1");

    println!("✓ Line numbers are diverse");
}

#[test]
fn test_line_numbers_in_real_docs() {
    let docs_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("docs");

    if !docs_path.exists() {
        return;
    }

    let objects = parse_all_workspaces(&docs_path, OutputFormat::Full).objects;

    let db = QmdcDatabase::new().unwrap();
    db.sync_objects_from_vec(&objects).unwrap();

    let result = db.query(
        "SELECT __id, __line FROM objects WHERE __kind NOT IN ('__Document', '__TextBlock', '__Workspace', '__Namespace') LIMIT 10"
    ).unwrap();

    let has_non_one = result
        .rows
        .iter()
        .any(|row| row[1].as_i64().unwrap_or(0) > 1);
    assert!(has_non_one, "Real docs should have objects on lines > 1");
}
