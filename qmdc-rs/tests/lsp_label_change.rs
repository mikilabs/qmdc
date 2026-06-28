//! QMD-35: Test that changing object label preserves object in tree
//!
//! Bug: When editing object label (just adding a space) and saving,
//! the object disappears from Explorer tree. It only reappears after
//! restarting the Language Server.
//!
//! This test verifies that after did_save with label change:
//! 1. Object remains in workspace index
//! 2. Object is visible via getWorkspaceTree
//! 3. Object is queryable via SQL

use std::fs;
use std::time::Duration;

use futures::StreamExt;
use serde_json::{json, Value};
use tempfile::TempDir;
use tower::Service;
use tower_lsp::lsp_types::Url;
use tower_lsp::LspService;

use qmdc::lsp::server::Backend;

fn build_request(method: &'static str, params: Value, id: i64) -> tower_lsp::jsonrpc::Request {
    tower_lsp::jsonrpc::Request::build(method)
        .params(params)
        .id(id)
        .finish()
}

fn build_notification(method: &'static str, params: Value) -> tower_lsp::jsonrpc::Request {
    tower_lsp::jsonrpc::Request::build(method)
        .params(params)
        .finish()
}

/// Query objects by ID via SQL - returns (id, label, kind, namespace)
async fn find_object_by_id(
    service: &mut LspService<Backend>,
    id: i64,
    object_id: &str,
) -> Option<Value> {
    let sql = format!(
        "SELECT __id, __label, __kind, __namespace FROM objects WHERE __id = '{}'",
        object_id
    );
    let sql_request = build_request(
        "workspace/executeCommand",
        json!({
            "command": "qmdc.runSqlQuery",
            "arguments": [sql]
        }),
        id,
    );

    if let Ok(Some(response)) = service.call(sql_request).await {
        if let Some(result) = response.result() {
            if let Some(rows) = result.get("rows").and_then(|r| r.as_array()) {
                if let Some(first_row) = rows.first() {
                    return Some(first_row.clone());
                }
            }
        }
    }
    None
}

/// Get namespace from object row
fn get_namespace_from_row(row: &Value) -> Option<String> {
    row.as_array()
        .and_then(|arr| arr.get(3))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

/// Get object label from workspace tree
async fn find_object_label_in_tree(
    service: &mut LspService<Backend>,
    id: i64,
    object_id: &str,
) -> Option<String> {
    let tree_request = build_request(
        "workspace/executeCommand",
        json!({
            "command": "qmdc.getWorkspaceTree",
            "arguments": [".", "namespace"]
        }),
        id,
    );

    if let Ok(Some(response)) = service.call(tree_request).await {
        if let Some(result) = response.result() {
            return find_label_recursive(result, object_id);
        }
    }
    None
}

fn find_label_recursive(value: &Value, target_id: &str) -> Option<String> {
    // Check workspaces
    if let Some(workspaces) = value.get("workspaces").and_then(|w| w.as_array()) {
        for ws in workspaces {
            // Check namespaces
            if let Some(namespaces) = ws.get("namespaces").and_then(|n| n.as_array()) {
                for ns in namespaces {
                    if let Some(label) = find_in_namespace(ns, target_id) {
                        return Some(label);
                    }
                }
            }
            // Check kindGroups (for smart/kind mode)
            if let Some(kind_groups) = ws.get("kindGroups").and_then(|k| k.as_array()) {
                for kg in kind_groups {
                    if let Some(objects) = kg.get("objects").and_then(|o| o.as_array()) {
                        if let Some(label) = find_in_objects(objects, target_id) {
                            return Some(label);
                        }
                    }
                }
            }
        }
    }
    None
}

fn find_in_namespace(ns: &Value, target_id: &str) -> Option<String> {
    if let Some(kind_groups) = ns.get("kindGroups").and_then(|k| k.as_array()) {
        for kg in kind_groups {
            if let Some(objects) = kg.get("objects").and_then(|o| o.as_array()) {
                if let Some(label) = find_in_objects(objects, target_id) {
                    return Some(label);
                }
            }
        }
    }
    None
}

fn find_in_objects(objects: &[Value], target_id: &str) -> Option<String> {
    for obj in objects {
        let obj_id = obj.get("id").and_then(|v| v.as_str()).unwrap_or("");
        if obj_id == target_id {
            return obj
                .get("label")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
        }
        // Check children
        if let Some(children) = obj.get("children").and_then(|c| c.as_array()) {
            if let Some(label) = find_in_objects(children, target_id) {
                return Some(label);
            }
        }
    }
    None
}

/// QMD-35: Test that changing label in a separate file preserves object
/// Exact reproduction of user scenario: docs/tracking/planned/QMD-10/QMD-10-task.qmd.md
#[tokio::test]
async fn test_qmd35_label_change_preserves_object() {
    let temp = TempDir::new().unwrap();
    let temp_path = temp.path().to_path_buf();

    // Create workspace structure exactly like the docs workspace:
    // docs/
    //   readme.qmd.md (workspace)
    //   tracking/
    //     readme.qmd.md (namespace)
    //     planned/
    //       QMD-10/
    //         QMD-10-task.qmd.md  <-- the file being edited

    fs::create_dir_all(temp_path.join("docs/tracking/planned/QMD-10")).unwrap();

    // Workspace readme
    let ws_readme = r#"# Docs [[docs: __Workspace]]

Documentation workspace.
"#;
    fs::write(temp_path.join("docs/readme.qmd.md"), ws_readme).unwrap();

    // Tracking namespace readme
    let tracking_readme = r#"# Tracking [[tracking: __Namespace]]

Task tracking namespace.
"#;
    fs::write(
        temp_path.join("docs/tracking/readme.qmd.md"),
        tracking_readme,
    )
    .unwrap();

    // The actual task file (exactly like QMD-10)
    let task_content = r#"## QMD-10: Bug title [[qmd10_bug: Bug]]

- status: planned
- priority: high
- category: parser
"#;
    fs::write(
        temp_path.join("docs/tracking/planned/QMD-10/QMD-10-task.qmd.md"),
        task_content,
    )
    .unwrap();

    let ws_uri = Url::from_file_path(temp_path.join("docs")).unwrap();
    let task_uri =
        Url::from_file_path(temp_path.join("docs/tracking/planned/QMD-10/QMD-10-task.qmd.md"))
            .unwrap();

    let (mut service, socket) = LspService::new(Backend::new);
    tokio::spawn(async move {
        let mut socket = socket;
        while let Some(_msg) = socket.next().await {}
    });

    // Initialize
    let _ = service
        .call(build_request(
            "initialize",
            json!({
                "capabilities": {},
                "workspaceFolders": [{"uri": ws_uri.to_string(), "name": "test-workspace"}]
            }),
            1,
        ))
        .await;
    let _ = service
        .call(build_notification("initialized", json!({})))
        .await;

    // Open task file
    let _ = service
        .call(build_notification(
            "textDocument/didOpen",
            json!({
                "textDocument": {
                    "uri": task_uri.to_string(),
                    "languageId": "qmd",
                    "version": 1,
                    "text": task_content,
                }
            }),
        ))
        .await;

    tokio::time::sleep(Duration::from_millis(300)).await;

    // Verify object exists initially WITH namespace
    let initial_obj = find_object_by_id(&mut service, 2, "qmd10_bug").await;
    println!("Initial object via SQL: {:?}", initial_obj);
    assert!(
        initial_obj.is_some(),
        "Object 'qmd10_bug' should exist initially"
    );

    let initial_namespace = get_namespace_from_row(initial_obj.as_ref().unwrap());
    println!("Initial namespace: {:?}", initial_namespace);
    assert!(
        initial_namespace.is_some(),
        "Object should have namespace initially"
    );
    assert_eq!(
        initial_namespace.as_deref(),
        Some("tracking"),
        "Object should be in 'tracking' namespace initially"
    );

    // ========================================
    // THE BUG: Change ONLY the label (add space)
    // ========================================
    let updated_content = r#"## QMD-10: Bug title  [[qmd10_bug: Bug]]

- status: planned
- priority: high
- category: parser
"#;

    fs::write(
        temp_path.join("docs/tracking/planned/QMD-10/QMD-10-task.qmd.md"),
        updated_content,
    )
    .unwrap();

    // Send did_save WITHOUT text (VS Code default - LSP must read from disk)
    let _ = service
        .call(build_notification(
            "textDocument/didSave",
            json!({
                "textDocument": {"uri": task_uri.to_string()}
            }),
        ))
        .await;

    tokio::time::sleep(Duration::from_millis(300)).await;

    // ========================================
    // VERIFY: Object should KEEP its namespace!
    // ========================================
    let after_save_obj = find_object_by_id(&mut service, 4, "qmd10_bug").await;
    println!("After save object via SQL: {:?}", after_save_obj);
    assert!(
        after_save_obj.is_some(),
        "QMD-35 BUG: Object 'qmd10_bug' DISAPPEARED from SQL!"
    );

    let after_namespace = get_namespace_from_row(after_save_obj.as_ref().unwrap());
    println!("After save namespace: {:?}", after_namespace);

    // THIS IS THE ACTUAL BUG CHECK:
    assert!(
        after_namespace.is_some(),
        "QMD-35 BUG: Object LOST its __namespace after did_save!"
    );
    assert_eq!(
        after_namespace.as_deref(),
        Some("tracking"),
        "QMD-35 BUG: Object should KEEP 'tracking' namespace after save, but got {:?}",
        after_namespace
    );

    println!("✓ QMD-35: Object correctly preserves namespace after label change");
}

/// Test: Change label multiple times, object should persist
#[tokio::test]
async fn test_qmd35_multiple_label_changes() {
    let temp = TempDir::new().unwrap();
    let temp_path = temp.path().to_path_buf();

    let readme_content = r#"# WS [[ws: __Workspace]]
"#;
    fs::write(temp_path.join("readme.qmd.md"), readme_content).unwrap();

    let initial_content = r#"# Task

## Feature One [[feat1: Feature]]

- status: planned
"#;
    fs::write(temp_path.join("task.qmd.md"), initial_content).unwrap();

    let ws_uri = Url::from_file_path(&temp_path).unwrap();
    let task_uri = Url::from_file_path(temp_path.join("task.qmd.md")).unwrap();

    let (mut service, socket) = LspService::new(Backend::new);
    tokio::spawn(async move {
        let mut socket = socket;
        while let Some(_msg) = socket.next().await {}
    });

    let _ = service
        .call(build_request(
            "initialize",
            json!({
                "capabilities": {},
                "workspaceFolders": [{"uri": ws_uri.to_string(), "name": "test"}]
            }),
            1,
        ))
        .await;
    let _ = service
        .call(build_notification("initialized", json!({})))
        .await;

    let _ = service
        .call(build_notification(
            "textDocument/didOpen",
            json!({
                "textDocument": {
                    "uri": task_uri.to_string(),
                    "languageId": "qmd",
                    "version": 1,
                    "text": initial_content,
                }
            }),
        ))
        .await;

    tokio::time::sleep(Duration::from_millis(200)).await;

    // Verify initial
    let obj = find_object_by_id(&mut service, 2, "feat1").await;
    assert!(obj.is_some(), "feat1 should exist initially");

    // Change 1: Add space
    let content_v2 = r#"# Task

## Feature One  [[feat1: Feature]]

- status: planned
"#;
    fs::write(temp_path.join("task.qmd.md"), content_v2).unwrap();
    let _ = service
        .call(build_notification(
            "textDocument/didSave",
            json!({
                "textDocument": {"uri": task_uri.to_string()},
                "text": content_v2,
            }),
        ))
        .await;
    tokio::time::sleep(Duration::from_millis(200)).await;

    let obj = find_object_by_id(&mut service, 3, "feat1").await;
    assert!(obj.is_some(), "QMD-35: feat1 disappeared after change 1");

    // Change 2: Different label
    let content_v3 = r#"# Task

## Feature Updated [[feat1: Feature]]

- status: planned
"#;
    fs::write(temp_path.join("task.qmd.md"), content_v3).unwrap();
    let _ = service
        .call(build_notification(
            "textDocument/didSave",
            json!({
                "textDocument": {"uri": task_uri.to_string()},
                "text": content_v3,
            }),
        ))
        .await;
    tokio::time::sleep(Duration::from_millis(200)).await;

    let obj = find_object_by_id(&mut service, 4, "feat1").await;
    assert!(obj.is_some(), "QMD-35: feat1 disappeared after change 2");

    // Change 3: Back to original
    let content_v4 = r#"# Task

## Feature One [[feat1: Feature]]

- status: planned
"#;
    fs::write(temp_path.join("task.qmd.md"), content_v4).unwrap();
    let _ = service
        .call(build_notification(
            "textDocument/didSave",
            json!({
                "textDocument": {"uri": task_uri.to_string()},
                "text": content_v4,
            }),
        ))
        .await;
    tokio::time::sleep(Duration::from_millis(200)).await;

    let obj = find_object_by_id(&mut service, 5, "feat1").await;
    assert!(obj.is_some(), "QMD-35: feat1 disappeared after change 3");

    let label = find_object_label_in_tree(&mut service, 6, "feat1").await;
    assert!(label.is_some(), "QMD-35: feat1 not in tree after changes");

    println!("✓ QMD-35: Multiple label changes preserve object");
}
