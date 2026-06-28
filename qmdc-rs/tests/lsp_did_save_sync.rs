//! Integration test for did_save SQLite synchronization (QMD-12)
//!
//! Tests that:
//! 1. After did_save, SQLite database is updated
//! 2. Objects added/removed in file are reflected in SQLite
//! 3. getWorkspaceTree returns correct data after save

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

/// Count objects via SQL query
async fn count_objects_via_sql(service: &mut LspService<Backend>, id: i64) -> usize {
    let sql_request = build_request(
        "workspace/executeCommand",
        json!({
            "command": "qmdc.runSqlQuery",
            "arguments": ["SELECT COUNT(*) as cnt FROM objects WHERE __kind NOT IN ('__Workspace', '__Namespace', '__Document', '__TextBlock')"]
        }),
        id,
    );

    if let Ok(Some(response)) = service.call(sql_request).await {
        if let Some(result) = response.result() {
            if let Some(rows) = result.get("rows").and_then(|r| r.as_array()) {
                if let Some(first_row) = rows.first() {
                    if let Some(first_col) = first_row.as_array().and_then(|r| r.first()) {
                        return first_col.as_u64().unwrap_or(0) as usize;
                    }
                }
            }
        }
    }
    0
}

/// Get workspace tree and count objects in smart mode
async fn count_objects_in_tree(service: &mut LspService<Backend>, id: i64) -> usize {
    let tree_request = build_request(
        "workspace/executeCommand",
        json!({
            "command": "qmdc.getWorkspaceTree",
            "arguments": [".", "smart"]
        }),
        id,
    );

    if let Ok(Some(response)) = service.call(tree_request).await {
        if let Some(result) = response.result() {
            return count_objects_recursive(result);
        }
    }
    0
}

fn count_objects_recursive(value: &Value) -> usize {
    let mut count = 0;

    if let Some(workspaces) = value.get("workspaces").and_then(|w| w.as_array()) {
        for ws in workspaces {
            if let Some(objects) = ws.get("objects").and_then(|o| o.as_array()) {
                count += count_objects_in_array(objects);
            }
        }
    }

    count
}

fn count_objects_in_array(objects: &[Value]) -> usize {
    let mut count = objects.len();
    for obj in objects {
        if let Some(children) = obj.get("children").and_then(|c| c.as_array()) {
            count += count_objects_in_array(children);
        }
    }
    count
}

/// Test that did_save updates SQLite (verified via SQL)
#[tokio::test]
async fn test_did_save_updates_sqlite() {
    let temp = TempDir::new().unwrap();
    let temp_path = temp.path().to_path_buf();

    // Create initial workspace with one object
    let readme_content = r#"# Test Workspace [[test_ws: __Workspace]]

## User [[user1: Entity]]

A test user.
"#;
    fs::write(temp_path.join("readme.qmd.md"), readme_content).unwrap();

    let ws_uri = Url::from_file_path(&temp_path).unwrap();
    let readme_uri = Url::from_file_path(temp_path.join("readme.qmd.md")).unwrap();

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

    // Open document
    let _ = service
        .call(build_notification(
            "textDocument/didOpen",
            json!({
                "textDocument": {
                    "uri": readme_uri.to_string(),
                    "languageId": "qmd",
                    "version": 1,
                    "text": readme_content,
                }
            }),
        ))
        .await;

    tokio::time::sleep(Duration::from_millis(100)).await;

    // Get initial object count
    let initial_sql_count = count_objects_via_sql(&mut service, 2).await;
    let initial_tree_count = count_objects_in_tree(&mut service, 3).await;
    println!(
        "Initial: SQL={}, Tree={}",
        initial_sql_count, initial_tree_count
    );
    assert!(
        initial_sql_count >= 1,
        "Should have at least 1 object in SQL, got {}",
        initial_sql_count
    );
    assert!(
        initial_tree_count >= 1,
        "Should have at least 1 object in Tree, got {}",
        initial_tree_count
    );

    // Modify file - add a new object
    let updated_content = r#"# Test Workspace [[test_ws: __Workspace]]

## User [[user1: Entity]]

A test user.

## Admin [[admin1: Entity]]

An admin user.
"#;
    fs::write(temp_path.join("readme.qmd.md"), updated_content).unwrap();

    // Send did_save
    let _ = service
        .call(build_notification(
            "textDocument/didSave",
            json!({
                "textDocument": {"uri": readme_uri.to_string()},
                "text": updated_content,
            }),
        ))
        .await;

    tokio::time::sleep(Duration::from_millis(200)).await;

    // Get updated counts
    let updated_sql_count = count_objects_via_sql(&mut service, 4).await;
    let updated_tree_count = count_objects_in_tree(&mut service, 5).await;
    println!(
        "Updated: SQL={}, Tree={}",
        updated_sql_count, updated_tree_count
    );

    assert!(
        updated_sql_count > initial_sql_count,
        "SQL count should increase: {} -> {}",
        initial_sql_count,
        updated_sql_count
    );
    assert!(
        updated_tree_count > initial_tree_count,
        "Tree count should increase: {} -> {}",
        initial_tree_count,
        updated_tree_count
    );

    println!("✓ did_save correctly updates both SQLite and Tree");
}

/// Test that removing objects via did_save updates SQLite
#[tokio::test]
async fn test_did_save_removes_objects() {
    let temp = TempDir::new().unwrap();
    let temp_path = temp.path().to_path_buf();

    // Create workspace with two objects
    let initial_content = r#"# Test Workspace [[test_ws: __Workspace]]

## User [[user1: Entity]]

## Admin [[admin1: Entity]]
"#;
    fs::write(temp_path.join("readme.qmd.md"), initial_content).unwrap();

    let ws_uri = Url::from_file_path(&temp_path).unwrap();
    let readme_uri = Url::from_file_path(temp_path.join("readme.qmd.md")).unwrap();

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
                "workspaceFolders": [{"uri": ws_uri.to_string(), "name": "test"}]
            }),
            1,
        ))
        .await;
    let _ = service
        .call(build_notification("initialized", json!({})))
        .await;

    // Open
    let _ = service
        .call(build_notification(
            "textDocument/didOpen",
            json!({
                "textDocument": {
                    "uri": readme_uri.to_string(),
                    "languageId": "qmd",
                    "version": 1,
                    "text": initial_content,
                }
            }),
        ))
        .await;

    tokio::time::sleep(Duration::from_millis(100)).await;

    // Get initial counts
    let initial_sql_count = count_objects_via_sql(&mut service, 2).await;
    let initial_tree_count = count_objects_in_tree(&mut service, 3).await;
    println!(
        "Initial: SQL={}, Tree={}",
        initial_sql_count, initial_tree_count
    );
    assert!(
        initial_sql_count >= 2,
        "Should have at least 2 objects in SQL, got {}",
        initial_sql_count
    );

    // Remove one object
    let updated_content = r#"# Test Workspace [[test_ws: __Workspace]]

## User [[user1: Entity]]
"#;
    fs::write(temp_path.join("readme.qmd.md"), updated_content).unwrap();

    // did_save
    let _ = service
        .call(build_notification(
            "textDocument/didSave",
            json!({
                "textDocument": {"uri": readme_uri.to_string()},
                "text": updated_content,
            }),
        ))
        .await;

    tokio::time::sleep(Duration::from_millis(200)).await;

    // Get updated counts
    let updated_sql_count = count_objects_via_sql(&mut service, 4).await;
    let updated_tree_count = count_objects_in_tree(&mut service, 5).await;
    println!(
        "Updated: SQL={}, Tree={}",
        updated_sql_count, updated_tree_count
    );

    assert!(
        updated_sql_count < initial_sql_count,
        "SQL count should decrease: {} -> {}",
        initial_sql_count,
        updated_sql_count
    );
    assert!(
        updated_tree_count < initial_tree_count,
        "Tree count should decrease: {} -> {}",
        initial_tree_count,
        updated_tree_count
    );

    println!("✓ did_save correctly removes objects from both SQLite and Tree");
}

/// Test that did_change_watched_files updates SQLite (external file changes like git pull)
#[tokio::test]
async fn test_did_change_watched_files_updates_sqlite() {
    let temp = TempDir::new().unwrap();
    let temp_path = temp.path().to_path_buf();

    // Create initial workspace
    let readme_content = r#"# Test Workspace [[test_ws: __Workspace]]

## User [[user1: Entity]]
"#;
    fs::write(temp_path.join("readme.qmd.md"), readme_content).unwrap();

    let ws_uri = Url::from_file_path(&temp_path).unwrap();
    let readme_uri = Url::from_file_path(temp_path.join("readme.qmd.md")).unwrap();

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
                "workspaceFolders": [{"uri": ws_uri.to_string(), "name": "test"}]
            }),
            1,
        ))
        .await;
    let _ = service
        .call(build_notification("initialized", json!({})))
        .await;

    tokio::time::sleep(Duration::from_millis(100)).await;

    // Get initial count (no didOpen - simulating external file)
    let initial_sql_count = count_objects_via_sql(&mut service, 2).await;
    println!("Initial (external): SQL={}", initial_sql_count);
    assert!(
        initial_sql_count >= 1,
        "Should have at least 1 object initially"
    );

    // Simulate external change (like git pull) - modify file on disk
    let updated_content = r#"# Test Workspace [[test_ws: __Workspace]]

## User [[user1: Entity]]

## NewObject [[new_obj: Entity]]
"#;
    fs::write(temp_path.join("readme.qmd.md"), updated_content).unwrap();

    // Send didChangeWatchedFiles notification (simulating VS Code file watcher)
    let _ = service
        .call(build_notification(
            "workspace/didChangeWatchedFiles",
            json!({
                "changes": [{
                    "uri": readme_uri.to_string(),
                    "type": 2  // FileChangeType.Changed = 2
                }]
            }),
        ))
        .await;

    tokio::time::sleep(Duration::from_millis(200)).await;

    // Get updated count
    let updated_sql_count = count_objects_via_sql(&mut service, 3).await;
    println!("Updated (external): SQL={}", updated_sql_count);

    assert!(
        updated_sql_count > initial_sql_count,
        "SQL count should increase after external change: {} -> {}",
        initial_sql_count,
        updated_sql_count
    );

    println!("✓ didChangeWatchedFiles correctly updates SQLite for external changes");
}

/// Test that file deletion via didChangeWatchedFiles removes objects
#[tokio::test]
async fn test_did_change_watched_files_delete() {
    let temp = TempDir::new().unwrap();
    let temp_path = temp.path().to_path_buf();

    // Create workspace with main file and extra file
    let readme_content = r#"# Test Workspace [[test_ws: __Workspace]]
"#;
    let extra_content = r#"## Extra [[extra1: Entity]]
"#;
    fs::write(temp_path.join("readme.qmd.md"), readme_content).unwrap();
    fs::write(temp_path.join("extra.qmd.md"), extra_content).unwrap();

    let ws_uri = Url::from_file_path(&temp_path).unwrap();
    let extra_uri = Url::from_file_path(temp_path.join("extra.qmd.md")).unwrap();

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
                "workspaceFolders": [{"uri": ws_uri.to_string(), "name": "test"}]
            }),
            1,
        ))
        .await;
    let _ = service
        .call(build_notification("initialized", json!({})))
        .await;

    tokio::time::sleep(Duration::from_millis(100)).await;

    // Get initial count
    let initial_sql_count = count_objects_via_sql(&mut service, 2).await;
    println!("Initial (before delete): SQL={}", initial_sql_count);
    assert!(
        initial_sql_count >= 1,
        "Should have at least 1 object initially"
    );

    // Delete extra file on disk
    fs::remove_file(temp_path.join("extra.qmd.md")).unwrap();

    // Send didChangeWatchedFiles with delete type
    let _ = service
        .call(build_notification(
            "workspace/didChangeWatchedFiles",
            json!({
                "changes": [{
                    "uri": extra_uri.to_string(),
                    "type": 3  // FileChangeType.Deleted = 3
                }]
            }),
        ))
        .await;

    tokio::time::sleep(Duration::from_millis(200)).await;

    // Get updated count
    let updated_sql_count = count_objects_via_sql(&mut service, 3).await;
    println!("Updated (after delete): SQL={}", updated_sql_count);

    assert!(
        updated_sql_count < initial_sql_count,
        "SQL count should decrease after file deletion: {} -> {}",
        initial_sql_count,
        updated_sql_count
    );

    println!("✓ didChangeWatchedFiles correctly removes objects on file deletion");
}

/// Test that objects after did_save have correct __namespace and appear in tree
/// This is the REAL test - objects must be in correct namespace, not just counted
#[tokio::test]
async fn test_did_save_preserves_namespace_in_tree() {
    let temp = TempDir::new().unwrap();
    let temp_path = temp.path().to_path_buf();

    // Create workspace with namespace structure
    let readme_content = r#"# Test Workspace [[test_ws: __Workspace]]
"#;
    // Create namespace folder
    fs::create_dir_all(temp_path.join("myns")).unwrap();
    let ns_readme = r#"# My Namespace [[myns: __Namespace]]
"#;
    let ns_file = r#"## Initial Object [[obj1: Entity]]

Some content.
"#;
    fs::write(temp_path.join("readme.qmd.md"), readme_content).unwrap();
    fs::write(temp_path.join("myns/readme.qmd.md"), ns_readme).unwrap();
    fs::write(temp_path.join("myns/objects.qmd.md"), ns_file).unwrap();

    let ws_uri = Url::from_file_path(&temp_path).unwrap();
    let objects_uri = Url::from_file_path(temp_path.join("myns/objects.qmd.md")).unwrap();

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
                "workspaceFolders": [{"uri": ws_uri.to_string(), "name": "test"}]
            }),
            1,
        ))
        .await;
    let _ = service
        .call(build_notification("initialized", json!({})))
        .await;

    // Open the file we'll modify
    let _ = service
        .call(build_notification(
            "textDocument/didOpen",
            json!({
                "textDocument": {
                    "uri": objects_uri.to_string(),
                    "languageId": "qmd",
                    "version": 1,
                    "text": ns_file,
                }
            }),
        ))
        .await;

    tokio::time::sleep(Duration::from_millis(100)).await;

    // Get initial tree and verify obj1 is in myns namespace
    let initial_tree = get_namespace_tree(&mut service, 2).await;
    let initial_ns_objects = find_objects_in_namespace(&initial_tree, "myns");
    println!("Initial objects in 'myns': {:?}", initial_ns_objects);
    assert!(
        initial_ns_objects.contains(&"obj1".to_string()),
        "obj1 should be in myns namespace initially, found: {:?}",
        initial_ns_objects
    );

    // Add new object via did_save
    let updated_content = r#"## Initial Object [[obj1: Entity]]

Some content.

## New Object [[obj2: Entity]]

New content.
"#;
    fs::write(temp_path.join("myns/objects.qmd.md"), updated_content).unwrap();

    let _ = service
        .call(build_notification(
            "textDocument/didSave",
            json!({
                "textDocument": {"uri": objects_uri.to_string()},
                "text": updated_content,
            }),
        ))
        .await;

    tokio::time::sleep(Duration::from_millis(200)).await;

    // Get updated tree and verify BOTH objects are in myns namespace
    let updated_tree = get_namespace_tree(&mut service, 3).await;
    let updated_ns_objects = find_objects_in_namespace(&updated_tree, "myns");
    println!("Updated objects in 'myns': {:?}", updated_ns_objects);

    assert!(
        updated_ns_objects.contains(&"obj1".to_string()),
        "obj1 should still be in myns namespace after save, found: {:?}",
        updated_ns_objects
    );
    assert!(
        updated_ns_objects.contains(&"obj2".to_string()),
        "obj2 should be in myns namespace after save, found: {:?}",
        updated_ns_objects
    );

    println!("✓ did_save preserves namespace correctly in tree");
}

/// Get workspace tree in namespace mode
async fn get_namespace_tree(service: &mut LspService<Backend>, id: i64) -> Value {
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
            return result.clone();
        }
    }
    json!({})
}

/// Find all object IDs in a specific namespace
fn find_objects_in_namespace(tree: &Value, namespace_id: &str) -> Vec<String> {
    let mut objects = Vec::new();

    if let Some(workspaces) = tree.get("workspaces").and_then(|w| w.as_array()) {
        for ws in workspaces {
            if let Some(namespaces) = ws.get("namespaces").and_then(|n| n.as_array()) {
                for ns in namespaces {
                    let ns_id = ns.get("id").and_then(|v| v.as_str()).unwrap_or("");
                    if ns_id == namespace_id {
                        // Found our namespace, collect all objects from kindGroups
                        if let Some(kind_groups) = ns.get("kindGroups").and_then(|k| k.as_array()) {
                            for kg in kind_groups {
                                if let Some(objs) = kg.get("objects").and_then(|o| o.as_array()) {
                                    for obj in objs {
                                        if let Some(id) = obj.get("id").and_then(|v| v.as_str()) {
                                            objects.push(id.to_string());
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    objects
}

/// Test that creating a new .qmd.md file updates the tree
#[tokio::test]
async fn test_create_new_file_updates_tree() {
    let temp = TempDir::new().unwrap();
    let temp_path = temp.path().to_path_buf();

    // Create initial workspace
    let readme_content = r#"# Test Workspace [[test_ws: __Workspace]]

## User [[user1: Entity]]
"#;
    fs::write(temp_path.join("readme.qmd.md"), readme_content).unwrap();

    let ws_uri = Url::from_file_path(&temp_path).unwrap();

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
                "workspaceFolders": [{"uri": ws_uri.to_string(), "name": "test"}]
            }),
            1,
        ))
        .await;
    let _ = service
        .call(build_notification("initialized", json!({})))
        .await;

    tokio::time::sleep(Duration::from_millis(100)).await;

    // Get initial count
    let initial_sql_count = count_objects_via_sql(&mut service, 2).await;
    let initial_tree_count = count_objects_in_tree(&mut service, 3).await;
    println!(
        "Initial: SQL={}, Tree={}",
        initial_sql_count, initial_tree_count
    );
    assert!(
        initial_sql_count >= 1,
        "Should have at least 1 object initially"
    );

    // Create new file
    let new_file_content = r#"## New Object [[new_obj: Entity]]

A new object.
"#;
    let new_file_path = temp_path.join("new_file.qmd.md");
    fs::write(&new_file_path, new_file_content).unwrap();
    let new_file_uri = Url::from_file_path(&new_file_path).unwrap();

    // Send didChangeWatchedFiles with CREATED type
    let _ = service
        .call(build_notification(
            "workspace/didChangeWatchedFiles",
            json!({
                "changes": [{
                    "uri": new_file_uri.to_string(),
                    "type": 1  // FileChangeType.Created = 1
                }]
            }),
        ))
        .await;

    tokio::time::sleep(Duration::from_millis(200)).await;

    // Get updated counts
    let updated_sql_count = count_objects_via_sql(&mut service, 4).await;
    let updated_tree_count = count_objects_in_tree(&mut service, 5).await;
    println!(
        "Updated: SQL={}, Tree={}",
        updated_sql_count, updated_tree_count
    );

    assert!(
        updated_sql_count > initial_sql_count,
        "SQL count should increase after new file: {} -> {}",
        initial_sql_count,
        updated_sql_count
    );
    assert!(
        updated_tree_count > initial_tree_count,
        "Tree count should increase after new file: {} -> {}",
        initial_tree_count,
        updated_tree_count
    );

    println!("✓ Creating new file correctly updates tree");
}

/// Test that creating a new namespace updates the tree
#[tokio::test]
async fn test_create_new_namespace_updates_tree() {
    let temp = TempDir::new().unwrap();
    let temp_path = temp.path().to_path_buf();

    // Create initial workspace
    let readme_content = r#"# Test Workspace [[test_ws: __Workspace]]
"#;
    fs::write(temp_path.join("readme.qmd.md"), readme_content).unwrap();

    let ws_uri = Url::from_file_path(&temp_path).unwrap();

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
                "workspaceFolders": [{"uri": ws_uri.to_string(), "name": "test"}]
            }),
            1,
        ))
        .await;
    let _ = service
        .call(build_notification("initialized", json!({})))
        .await;

    tokio::time::sleep(Duration::from_millis(100)).await;

    // Get initial namespace count
    let initial_tree = get_namespace_tree(&mut service, 2).await;
    let initial_namespaces = count_namespaces(&initial_tree);
    println!("Initial namespaces: {}", initial_namespaces);

    // Create new namespace folder
    let ns_folder = temp_path.join("newns");
    fs::create_dir_all(&ns_folder).unwrap();

    let ns_readme = r#"# New Namespace [[newns: __Namespace]]
"#;
    fs::write(ns_folder.join("readme.qmd.md"), ns_readme).unwrap();

    // Create objects file in namespace
    let ns_objects_file = r#"## Object 1 [[obj1: Entity]]

First object in namespace.

## Object 2 [[obj2: Entity]]

Second object in namespace.
"#;
    fs::write(ns_folder.join("objects.qmd.md"), ns_objects_file).unwrap();

    let ns_readme_uri = Url::from_file_path(ns_folder.join("readme.qmd.md")).unwrap();

    // Send didChangeWatchedFiles with CREATED type for readme
    let _ = service
        .call(build_notification(
            "workspace/didChangeWatchedFiles",
            json!({
                "changes": [{
                    "uri": ns_readme_uri.to_string(),
                    "type": 1  // FileChangeType.Created = 1
                }]
            }),
        ))
        .await;

    tokio::time::sleep(Duration::from_millis(500)).await;

    // Get updated namespace count
    let updated_tree = get_namespace_tree(&mut service, 3).await;
    let updated_namespaces = count_namespaces(&updated_tree);
    println!("Updated namespaces: {}", updated_namespaces);

    assert!(
        updated_namespaces > initial_namespaces,
        "Namespace count should increase: {} -> {}",
        initial_namespaces,
        updated_namespaces
    );

    // Verify namespace appears in tree
    let ns_objects = find_objects_in_namespace(&updated_tree, "newns");
    println!("Objects in newns namespace: {:?}", ns_objects);

    // Verify objects are in namespace
    assert!(
        ns_objects.contains(&"obj1".to_string()),
        "obj1 should be in newns namespace, found: {:?}",
        ns_objects
    );
    assert!(
        ns_objects.contains(&"obj2".to_string()),
        "obj2 should be in newns namespace, found: {:?}",
        ns_objects
    );

    println!("✓ Creating new namespace correctly updates tree with objects");
}

/// Test edge case: creating file in namespace BEFORE readme.qmd.md
#[tokio::test]
async fn test_create_file_before_namespace_readme() {
    let temp = TempDir::new().unwrap();
    let temp_path = temp.path().to_path_buf();

    // Create initial workspace
    let readme_content = r#"# Test Workspace [[test_ws: __Workspace]]
"#;
    fs::write(temp_path.join("readme.qmd.md"), readme_content).unwrap();

    let ws_uri = Url::from_file_path(&temp_path).unwrap();

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
                "workspaceFolders": [{"uri": ws_uri.to_string(), "name": "test"}]
            }),
            1,
        ))
        .await;
    let _ = service
        .call(build_notification("initialized", json!({})))
        .await;

    tokio::time::sleep(Duration::from_millis(100)).await;

    // Create namespace folder
    let ns_folder = temp_path.join("newns");
    fs::create_dir_all(&ns_folder).unwrap();

    // Create objects file FIRST (before readme)
    let ns_objects_file = r#"## Early Object [[early_obj: Entity]]

Object created before namespace readme.
"#;
    let objects_path = ns_folder.join("objects.qmd.md");
    fs::write(&objects_path, ns_objects_file).unwrap();
    let objects_uri = Url::from_file_path(&objects_path).unwrap();

    // Send didChangeWatchedFiles for objects file
    let _ = service
        .call(build_notification(
            "workspace/didChangeWatchedFiles",
            json!({
                "changes": [{
                    "uri": objects_uri.to_string(),
                    "type": 1  // FileChangeType.Created = 1
                }]
            }),
        ))
        .await;

    tokio::time::sleep(Duration::from_millis(500)).await;

    // Now create namespace readme
    let ns_readme = r#"# New Namespace [[newns: __Namespace]]
"#;
    let readme_path = ns_folder.join("readme.qmd.md");
    fs::write(&readme_path, ns_readme).unwrap();
    let ns_readme_uri = Url::from_file_path(&readme_path).unwrap();

    // Send didChangeWatchedFiles for readme
    let _ = service
        .call(build_notification(
            "workspace/didChangeWatchedFiles",
            json!({
                "changes": [{
                    "uri": ns_readme_uri.to_string(),
                    "type": 1  // FileChangeType.Created = 1
                }]
            }),
        ))
        .await;

    tokio::time::sleep(Duration::from_millis(500)).await;

    // Verify namespace and objects appear
    let updated_tree = get_namespace_tree(&mut service, 2).await;
    let ns_objects = find_objects_in_namespace(&updated_tree, "newns");
    println!(
        "Objects in newns namespace after both files: {:?}",
        ns_objects
    );

    assert!(
        ns_objects.contains(&"early_obj".to_string()),
        "early_obj should be in newns namespace after readme created, found: {:?}",
        ns_objects
    );

    println!("✓ File created before namespace readme is correctly associated");
}

/// Test that creating a new workspace is detected
#[tokio::test]
async fn test_create_new_workspace_detected() {
    let temp = TempDir::new().unwrap();
    let temp_path = temp.path().to_path_buf();

    // Create initial workspace
    let readme_content = r#"# Test Workspace [[test_ws: __Workspace]]
"#;
    fs::write(temp_path.join("readme.qmd.md"), readme_content).unwrap();

    let ws_uri = Url::from_file_path(&temp_path).unwrap();

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
                "workspaceFolders": [{"uri": ws_uri.to_string(), "name": "test"}]
            }),
            1,
        ))
        .await;
    let _ = service
        .call(build_notification("initialized", json!({})))
        .await;

    tokio::time::sleep(Duration::from_millis(100)).await;

    // Get initial workspace count
    let initial_tree = get_namespace_tree(&mut service, 2).await;
    let initial_workspaces = count_workspaces(&initial_tree);
    println!("Initial workspaces: {}", initial_workspaces);
    assert_eq!(initial_workspaces, 1, "Should have 1 workspace initially");

    // Create new workspace folder
    let new_ws_folder = temp_path.join("new_workspace");
    fs::create_dir_all(&new_ws_folder).unwrap();

    let new_ws_readme = r#"# New Workspace [[new_ws: __Workspace]]

## Object [[obj1: Entity]]
"#;
    fs::write(new_ws_folder.join("readme.qmd.md"), new_ws_readme).unwrap();
    let new_ws_readme_uri = Url::from_file_path(new_ws_folder.join("readme.qmd.md")).unwrap();

    // Send didChangeWatchedFiles with CREATED type
    let _ = service
        .call(build_notification(
            "workspace/didChangeWatchedFiles",
            json!({
                "changes": [{
                    "uri": new_ws_readme_uri.to_string(),
                    "type": 1  // FileChangeType.Created = 1
                }]
            }),
        ))
        .await;

    // Rescan can take longer - wait more
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Get updated workspace count
    let updated_tree = get_namespace_tree(&mut service, 3).await;
    let updated_workspaces = count_workspaces(&updated_tree);
    println!("Updated workspaces: {}", updated_workspaces);

    assert!(
        updated_workspaces > initial_workspaces,
        "Workspace count should increase: {} -> {}",
        initial_workspaces,
        updated_workspaces
    );

    // Verify new workspace appears in tree
    let workspace_ids: Vec<String> = updated_tree
        .get("workspaces")
        .and_then(|w| w.as_array())
        .map(|w| {
            w.iter()
                .filter_map(|ws| ws.get("id").and_then(|v| v.as_str()).map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();

    println!("Workspace IDs in tree: {:?}", workspace_ids);
    assert!(
        workspace_ids.contains(&"new_ws".to_string()),
        "new_ws workspace should appear in tree, found: {:?}",
        workspace_ids
    );

    // Note: Objects might not appear immediately in tree due to namespace grouping
    // The important thing is that workspace is detected and rescan happened

    println!("✓ Creating new workspace correctly detected");
}

/// Count namespaces in tree
fn count_namespaces(tree: &Value) -> usize {
    let mut count = 0;
    if let Some(workspaces) = tree.get("workspaces").and_then(|w| w.as_array()) {
        for ws in workspaces {
            if let Some(namespaces) = ws.get("namespaces").and_then(|n| n.as_array()) {
                count += namespaces.len();
            }
        }
    }
    count
}

/// Count workspaces in tree
fn count_workspaces(tree: &Value) -> usize {
    tree.get("workspaces")
        .and_then(|w| w.as_array())
        .map(|w| w.len())
        .unwrap_or(0)
}
