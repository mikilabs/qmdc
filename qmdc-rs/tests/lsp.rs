//! LSP Microtest runner - tests LSP server directly via tower::Service trait.
//!
//! Based on how tower-lsp itself tests:
//! https://github.com/ebkalderon/tower-lsp/blob/master/src/service.rs#L247

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures::StreamExt;
use serde_json::{json, Value};
use tempfile::TempDir;
use tower::Service;
use tower_lsp::lsp_types::Url;

mod common;
use tower_lsp::LspService;

// Import our Backend from the library
use qmdc::lsp::server::Backend;

fn get_lsp_microtests_dir() -> PathBuf {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    PathBuf::from(manifest_dir)
        .parent()
        .unwrap()
        .join("tests/lsp/microtests")
}

fn get_lsp_sql_tests_dir() -> PathBuf {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    PathBuf::from(manifest_dir)
        .parent()
        .unwrap()
        .join("tests/lsp/sql")
}

/// Build a JSON-RPC request
fn build_request(method: &'static str, params: Value, id: i64) -> tower_lsp::jsonrpc::Request {
    tower_lsp::jsonrpc::Request::build(method)
        .params(params)
        .id(id)
        .finish()
}

/// Build a JSON-RPC notification (no id)
fn build_notification(method: &'static str, params: Value) -> tower_lsp::jsonrpc::Request {
    tower_lsp::jsonrpc::Request::build(method)
        .params(params)
        .finish()
}

/// Recursively copy directory contents (excluding already copied files)
fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), String> {
    for entry in fs::read_dir(src).map_err(|e| format!("Failed to read dir: {}", e))? {
        let entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
        let src_path = entry.path();
        let filename = src_path.file_name().unwrap();
        let dst_path = dst.join(filename);

        if src_path.is_dir() {
            fs::create_dir_all(&dst_path).map_err(|e| format!("Failed to create dir: {}", e))?;
            copy_dir_recursive(&src_path, &dst_path)?;
        } else if !dst_path.exists() {
            // Only copy if not already copied
            fs::copy(&src_path, &dst_path).map_err(|e| format!("Failed to copy file: {}", e))?;
        }
    }
    Ok(())
}

/// Convert real file URI back to test-relative path for comparison
fn uri_to_relative_path(uri_str: &str, ws_root: &Option<PathBuf>) -> String {
    if let Some(ws_root) = ws_root {
        if let Ok(uri) = Url::parse(uri_str) {
            if let Ok(path) = uri.to_file_path() {
                if let Ok(relative) = path.strip_prefix(ws_root) {
                    return format!("workspace/{}", relative.display());
                }
            }
        }
    }
    // Fallback: just return filename
    uri_str
        .rsplit('/')
        .next()
        .unwrap_or("input.qmd.md")
        .to_string()
}

/// Get all LSP microtest directories
fn get_lsp_test_dirs() -> Vec<(String, PathBuf, String)> {
    let base_dir = get_lsp_microtests_dir();
    let sql_tests_dir = get_lsp_sql_tests_dir();
    let mut tests = Vec::new();

    let categories = [
        "diagnostics",
        "completion",
        "hover",
        "definition",
        "references",
        "documentSymbol",
        "prepareRename",
        "rename",
        "workspaceSymbol",
    ];

    for category in categories {
        let cat_dir = base_dir.join(category);
        if !cat_dir.exists() {
            continue;
        }

        let mut entries: Vec<_> = fs::read_dir(&cat_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .collect();

        entries.sort();

        for test_dir in entries {
            let test_name = test_dir.file_name().unwrap().to_string_lossy().to_string();
            let full_name = format!("{}/{}", category, test_name);
            tests.push((full_name, test_dir, category.to_string()));
        }
    }

    // Add runSqlQuery tests from QMD-24
    if sql_tests_dir.exists() {
        let mut entries: Vec<_> = fs::read_dir(&sql_tests_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .collect();

        entries.sort();

        for test_dir in entries {
            // Only dirs with an LSP `request.json` are runSqlQuery cases. Dirs that carry only
            // MCP fixtures (`mcp-request.json`) are exercised by the MCP harness, not here.
            if !test_dir.join("request.json").exists() {
                continue;
            }
            let test_name = test_dir.file_name().unwrap().to_string_lossy().to_string();
            let full_name = format!("runSqlQuery/{}", test_name);
            tests.push((full_name, test_dir, "runSqlQuery".to_string()));
        }
    }

    tests
}

async fn run_lsp_test(test_dir: &Path, category: &str) -> Result<(), String> {
    // Read input - either single file or workspace directory
    let input_file = test_dir.join("input.qmd.md");
    let workspace_dir = test_dir.join("workspace");
    let is_workspace_test = workspace_dir.exists();

    // For workspace tests, create real temp files so workspace scanner can find them
    let _temp_dir: Option<TempDir>;
    let workspace_root: Option<PathBuf>;
    let primary_uri: String;

    if is_workspace_test {
        // Create temp directory with real files
        let temp = TempDir::new().map_err(|e| format!("Failed to create temp dir: {}", e))?;
        let temp_path = temp.path().to_path_buf();

        // Copy workspace files to temp dir
        for entry in
            fs::read_dir(&workspace_dir).map_err(|e| format!("Failed to read workspace: {}", e))?
        {
            let entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
            let src_path = entry.path();
            if src_path.extension().map(|e| e == "md").unwrap_or(false) {
                let filename = src_path.file_name().unwrap();
                let dest_path = temp_path.join(filename);
                fs::copy(&src_path, &dest_path).map_err(|e| format!("Failed to copy: {}", e))?;
            }
        }

        // Also copy subdirectories (for nested workspace tests)
        copy_dir_recursive(&workspace_dir, &temp_path)?;

        // Create readme.qmd.md with __Workspace if not exists
        let readme_path = temp_path.join("readme.qmd.md");
        if !readme_path.exists() {
            fs::write(&readme_path, "# Test Workspace [[test_ws:__Workspace]]\n")
                .map_err(|e| format!("Failed to write readme: {}", e))?;
        }

        let main_uri = Url::from_file_path(temp_path.join("main.qmd.md"))
            .map_err(|_| "Failed to create URI")?;
        primary_uri = main_uri.to_string();
        workspace_root = Some(temp_path);
        _temp_dir = Some(temp);
    } else if input_file.exists() {
        let temp = TempDir::new().map_err(|e| format!("Failed to create temp dir: {}", e))?;
        let temp_path = temp.path().to_path_buf();
        let dest_path = temp_path.join("input.qmd.md");
        fs::copy(&input_file, &dest_path).map_err(|e| format!("Failed to copy input: {}", e))?;

        let file_uri = Url::from_file_path(&dest_path).map_err(|_| "Failed to create URI")?;
        primary_uri = file_uri.to_string();
        workspace_root = None;
        _temp_dir = Some(temp);
    } else {
        return Err("No input file found".to_string());
    }

    // Read request
    let request_file = test_dir.join("request.json");
    let request: Value = if request_file.exists() {
        let req_str = fs::read_to_string(&request_file)
            .map_err(|e| format!("Failed to read request: {}", e))?;
        serde_json::from_str(&req_str).map_err(|e| format!("Failed to parse request: {}", e))?
    } else {
        Value::Object(serde_json::Map::new())
    };

    // Get request URI - resolve relative path to real temp path
    let request_uri = if let Some(ref ws_root) = workspace_root {
        request
            .get("uri")
            .and_then(|u| u.as_str())
            .and_then(|u| {
                // Convert workspace/xxx.qmd.md to real path
                let relative = u.strip_prefix("workspace/").unwrap_or(u);
                let real_path = ws_root.join(relative);
                Url::from_file_path(&real_path).ok()
            })
            .map(|u| u.to_string())
            .unwrap_or_else(|| primary_uri.clone())
    } else {
        primary_uri.clone()
    };

    // Read expected
    let expected_file = test_dir.join("expected.json");
    let expected: Value = {
        let exp_str = fs::read_to_string(&expected_file)
            .map_err(|e| format!("Failed to read expected: {}", e))?;
        serde_json::from_str(&exp_str).map_err(|e| format!("Failed to parse expected: {}", e))?
    };

    // Create LSP service
    let (mut service, socket) = LspService::new(Backend::new);

    // Shared storage for captured diagnostics
    let captured_diagnostics: Arc<Mutex<Vec<Value>>> = Arc::new(Mutex::new(Vec::new()));
    let diagnostics_for_task = captured_diagnostics.clone();

    // Spawn task to capture diagnostics from socket messages
    tokio::spawn(async move {
        let mut socket = socket;
        while let Some(msg) = socket.next().await {
            if let Ok(msg_str) = serde_json::to_string(&msg) {
                if msg_str.contains("publishDiagnostics") {
                    if let Ok(parsed) = serde_json::from_str::<Value>(&msg_str) {
                        if let Some(params) = parsed.get("params") {
                            if let Ok(mut diags) = diagnostics_for_task.lock() {
                                diags.push(params.clone());
                            }
                        }
                    }
                }
            }
        }
    });

    // Initialize with workspace folders for workspace tests
    let init_params = if let Some(ref ws_root) = workspace_root {
        let ws_uri = Url::from_file_path(ws_root).map_err(|_| "Failed to create workspace URI")?;

        json!({
            "capabilities": {},
            "workspaceFolders": [{
                "uri": ws_uri.to_string(),
                "name": "test-workspace"
            }]
        })
    } else {
        json!({"capabilities":{}})
    };
    let init_request = build_request("initialize", init_params, 1);
    let _ = service.call(init_request).await;

    // Send initialized notification
    let initialized = build_notification("initialized", json!({}));
    let _ = service.call(initialized).await;

    // Open all documents in workspace (for workspace tests) or just the primary document
    if let Some(ref ws_root) = workspace_root {
        // Collect all .qmd.md files in the workspace
        fn collect_qmdc_files(dir: &PathBuf, files: &mut Vec<PathBuf>) {
            if let Ok(entries) = fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() {
                        collect_qmdc_files(&path, files);
                    } else if path.to_string_lossy().contains(".qmd.")
                        && path.extension().map(|e| e == "md").unwrap_or(false)
                    {
                        files.push(path);
                    }
                }
            }
        }

        let mut qmdc_files = Vec::new();
        collect_qmdc_files(ws_root, &mut qmdc_files);

        // Open all files asynchronously
        for path in qmdc_files {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(file_uri) = Url::from_file_path(&path) {
                    let did_open = build_notification(
                        "textDocument/didOpen",
                        json!({
                            "textDocument": {
                                "uri": file_uri.to_string(),
                                "languageId": "qmd",
                                "version": 1,
                                "text": content,
                            }
                        }),
                    );
                    let _ = service.call(did_open).await;
                }
            }
        }
    } else {
        // Single file test - open just the primary document
        let open_uri = &primary_uri;
        let content = if let Some(temp) = _temp_dir.as_ref() {
            fs::read_to_string(temp.path().join("input.qmd.md")).unwrap_or_default()
        } else {
            String::new()
        };

        let did_open = build_notification(
            "textDocument/didOpen",
            json!({
                "textDocument": {
                    "uri": open_uri,
                    "languageId": "qmd",
                    "version": 1,
                    "text": content,
                }
            }),
        );
        let _ = service.call(did_open).await;
    }

    // Use request URI for the actual request
    let uri = &request_uri;

    // Keep temp_dir info for URI conversion later
    let ws_root_for_uri = workspace_root.clone();

    // Run test based on category
    let actual: Value = match category {
        "diagnostics" => {
            // Wait for diagnostics to be published (handler runs async after did_open returns)
            tokio::time::sleep(Duration::from_millis(200)).await;

            // Get captured diagnostics for this URI
            let diags = captured_diagnostics.lock().unwrap();

            // Find diagnostics for the request URI (or primary URI)
            let target_uri = request
                .get("uri")
                .and_then(|u| u.as_str())
                .unwrap_or("input.qmd.md");

            let mut result_diagnostics: Vec<Value> = Vec::new();

            for diag_notification in diags.iter() {
                // Check if this notification is for our target URI
                let notif_uri = diag_notification
                    .get("uri")
                    .and_then(|u| u.as_str())
                    .unwrap_or("");

                // Match by filename since URIs differ between test definition and actual
                let notif_filename = notif_uri.rsplit('/').next().unwrap_or("");
                let target_filename = target_uri.rsplit('/').next().unwrap_or("");

                if notif_filename == target_filename || target_filename == "input.qmd.md" {
                    if let Some(diagnostics) = diag_notification
                        .get("diagnostics")
                        .and_then(|d| d.as_array())
                    {
                        for d in diagnostics {
                            // Simplify diagnostic output
                            let mut simplified = json!({
                                "message": d.get("message"),
                                "range": d.get("range"),
                                "severity": d.get("severity"),
                            });
                            if let Some(code) = d.get("code") {
                                simplified["code"] = code.clone();
                            }
                            result_diagnostics.push(simplified);
                        }
                    }
                }
            }

            // Sort diagnostics by line number for deterministic output
            result_diagnostics.sort_by(|a, b| {
                let line_a = a
                    .get("range")
                    .and_then(|r| r.get("start"))
                    .and_then(|s| s.get("line"))
                    .and_then(|l| l.as_u64())
                    .unwrap_or(0);
                let line_b = b
                    .get("range")
                    .and_then(|r| r.get("start"))
                    .and_then(|s| s.get("line"))
                    .and_then(|l| l.as_u64())
                    .unwrap_or(0);
                line_a.cmp(&line_b)
            });

            json!({ "diagnostics": result_diagnostics })
        }
        "completion" => {
            let pos = request.get("position").ok_or("No position in request")?;
            let completion_request = build_request(
                "textDocument/completion",
                json!({
                    "textDocument": { "uri": uri },
                    "position": pos,
                }),
                2,
            );

            let response = service
                .call(completion_request)
                .await
                .map_err(|e| format!("Completion error: {:?}", e))?;

            if let Some(resp) = response {
                if let Some(result) = resp.result() {
                    // Simplify completion items
                    if let Some(items) = result.as_array() {
                        let simplified: Vec<Value> = items
                            .iter()
                            .map(|item| {
                                let mut s = json!({ "label": item.get("label") });
                                if let Some(detail) = item.get("detail") {
                                    s["detail"] = detail.clone();
                                }
                                if let Some(kind) = item.get("kind") {
                                    s["kind"] = kind.clone();
                                }
                                s
                            })
                            .collect();
                        json!({ "items": simplified })
                    } else {
                        json!({ "items": [] })
                    }
                } else {
                    json!({ "items": [] })
                }
            } else {
                json!({ "items": [] })
            }
        }
        "hover" => {
            let pos = request.get("position").ok_or("No position in request")?;
            let hover_request = build_request(
                "textDocument/hover",
                json!({
                    "textDocument": { "uri": uri },
                    "position": pos,
                }),
                2,
            );

            let response = service
                .call(hover_request)
                .await
                .map_err(|e| format!("Hover error: {:?}", e))?;

            if let Some(resp) = response {
                if let Some(result) = resp.result() {
                    if result.is_null() {
                        json!({ "contents": null })
                    } else {
                        let contents = result
                            .get("contents")
                            .and_then(|c| c.get("value"))
                            .cloned()
                            .unwrap_or(json!(null));
                        json!({ "contents": contents })
                    }
                } else {
                    json!({ "contents": null })
                }
            } else {
                json!({ "contents": null })
            }
        }
        "definition" => {
            let pos = request.get("position").ok_or("No position in request")?;
            let def_request = build_request(
                "textDocument/definition",
                json!({
                    "textDocument": { "uri": uri },
                    "position": pos,
                }),
                2,
            );

            let response = service
                .call(def_request)
                .await
                .map_err(|e| format!("Definition error: {:?}", e))?;

            if let Some(resp) = response {
                if let Some(result) = resp.result() {
                    if result.is_null() {
                        json!({ "location": null })
                    } else {
                        // Convert real URI back to relative path
                        let result_uri = result
                            .get("uri")
                            .and_then(|u| u.as_str())
                            .map(|u| uri_to_relative_path(u, &ws_root_for_uri))
                            .unwrap_or_else(|| "input.qmd.md".to_string());
                        json!({
                            "uri": result_uri,
                            "range": result.get("range"),
                        })
                    }
                } else {
                    json!({ "location": null })
                }
            } else {
                json!({ "location": null })
            }
        }
        "references" => {
            let pos = request.get("position").ok_or("No position in request")?;
            let include_decl = request
                .get("context")
                .and_then(|c| c.get("includeDeclaration"))
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            let refs_request = build_request(
                "textDocument/references",
                json!({
                    "textDocument": { "uri": uri },
                    "position": pos,
                    "context": { "includeDeclaration": include_decl },
                }),
                2,
            );

            let response = service
                .call(refs_request)
                .await
                .map_err(|e| format!("References error: {:?}", e))?;

            if let Some(resp) = response {
                if let Some(result) = resp.result() {
                    let mut simplified: Vec<Value> = result
                        .as_array()
                        .unwrap_or(&vec![])
                        .iter()
                        .map(|r| {
                            let result_uri = r
                                .get("uri")
                                .and_then(|u| u.as_str())
                                .map(|u| uri_to_relative_path(u, &ws_root_for_uri))
                                .unwrap_or_else(|| "input.qmd.md".to_string());
                            json!({
                                "uri": result_uri,
                                "range": r.get("range"),
                            })
                        })
                        .collect();
                    // Sort by URI for deterministic output
                    simplified.sort_by(|a, b| {
                        let uri_a = a.get("uri").and_then(|v| v.as_str()).unwrap_or("");
                        let uri_b = b.get("uri").and_then(|v| v.as_str()).unwrap_or("");
                        uri_a.cmp(uri_b)
                    });
                    json!({ "references": simplified })
                } else {
                    json!({ "references": [] })
                }
            } else {
                json!({ "references": [] })
            }
        }
        "documentSymbol" => {
            let symbol_request = build_request(
                "textDocument/documentSymbol",
                json!({
                    "textDocument": { "uri": uri },
                }),
                2,
            );

            let response = service
                .call(symbol_request)
                .await
                .map_err(|e| format!("DocumentSymbol error: {:?}", e))?;

            if let Some(resp) = response {
                if let Some(result) = resp.result() {
                    result.clone()
                } else {
                    json!([])
                }
            } else {
                json!([])
            }
        }
        "prepareRename" => {
            let pos = request.get("position").ok_or("No position in request")?;

            let prepare_request = build_request(
                "textDocument/prepareRename",
                json!({
                    "textDocument": { "uri": uri },
                    "position": pos,
                }),
                2,
            );

            let response = service
                .call(prepare_request)
                .await
                .map_err(|e| format!("PrepareRename error: {:?}", e))?;

            if let Some(resp) = response {
                if let Some(result) = resp.result() {
                    result.clone()
                } else {
                    Value::Null
                }
            } else {
                Value::Null
            }
        }
        "rename" => {
            let pos = request.get("position").ok_or("No position in request")?;
            let new_name = request
                .get("newName")
                .and_then(|v| v.as_str())
                .ok_or("No newName in request")?;

            let rename_request = build_request(
                "textDocument/rename",
                json!({
                    "textDocument": { "uri": uri },
                    "position": pos,
                    "newName": new_name,
                }),
                2,
            );

            let response = service
                .call(rename_request)
                .await
                .map_err(|e| format!("Rename error: {:?}", e))?;

            if let Some(resp) = response {
                if let Some(result) = resp.result() {
                    // Simplify URIs in changes
                    if let Some(changes) = result.get("changes").and_then(|c| c.as_object()) {
                        let mut simplified_changes: serde_json::Map<String, Value> =
                            serde_json::Map::new();
                        for (change_uri, edits) in changes {
                            let relative_uri = uri_to_relative_path(change_uri, &ws_root_for_uri);
                            simplified_changes.insert(relative_uri, edits.clone());
                        }
                        json!({ "changes": simplified_changes })
                    } else {
                        result.clone()
                    }
                } else {
                    Value::Null
                }
            } else {
                Value::Null
            }
        }
        "workspaceSymbol" => {
            let query = request.get("query").and_then(|v| v.as_str()).unwrap_or("");

            let symbol_request = build_request(
                "workspace/symbol",
                json!({
                    "query": query,
                }),
                2,
            );

            let response = service
                .call(symbol_request)
                .await
                .map_err(|e| format!("WorkspaceSymbol error: {:?}", e))?;

            if let Some(resp) = response {
                if let Some(result) = resp.result() {
                    // Simplify symbols - only keep name, kind, containerName
                    let symbols: Vec<Value> = result
                        .as_array()
                        .unwrap_or(&vec![])
                        .iter()
                        .map(|s| {
                            json!({
                                "name": s.get("name"),
                                "kind": s.get("kind"),
                                "containerName": s.get("containerName"),
                            })
                        })
                        .collect();
                    json!({ "symbols": symbols })
                } else {
                    json!({ "symbols": [] })
                }
            } else {
                json!({ "symbols": [] })
            }
        }
        "runSqlQuery" => {
            // Wait a bit for database to sync after didOpen
            // The execute_command handler will call sync_sqlite, but we need to ensure
            // workspace indexing is complete first
            tokio::time::sleep(Duration::from_millis(200)).await;

            let query_input = request
                .get("arguments")
                .and_then(|a| a.as_array())
                .and_then(|a| a.first())
                .and_then(|v| v.as_str())
                .ok_or("No query in arguments")?;

            let document_uri = request
                .get("arguments")
                .and_then(|a| a.as_array())
                .and_then(|a| a.get(1))
                .and_then(|v| v.as_str())
                .map(|u| {
                    // Convert workspace/xxx.qmd.md to real path
                    if let Some(ref ws_root) = workspace_root {
                        let relative = u.strip_prefix("workspace/").unwrap_or(u);
                        let real_path = ws_root.join(relative);
                        Url::from_file_path(&real_path)
                            .ok()
                            .map(|url| url.to_string())
                            .unwrap_or_else(|| primary_uri.clone())
                    } else {
                        primary_uri.clone()
                    }
                })
                .or_else(|| {
                    request
                        .get("documentUri")
                        .and_then(|u| u.as_str())
                        .map(|s| s.to_string())
                });

            let scope = request
                .get("arguments")
                .and_then(|a| a.as_array())
                .and_then(|a| a.get(2))
                .and_then(|v| v.as_str())
                .or_else(|| request.get("scope").and_then(|v| v.as_str()));

            // Build executeCommand request
            let mut cmd_args = vec![json!(query_input)];
            if let Some(ref uri) = document_uri {
                cmd_args.push(json!(uri));
            }
            if let Some(scope_val) = scope {
                cmd_args.push(json!(scope_val));
            }

            let cmd_request = build_request(
                "workspace/executeCommand",
                json!({
                    "command": "qmdc.runSqlQuery",
                    "arguments": cmd_args,
                }),
                2,
            );

            let response = service
                .call(cmd_request)
                .await
                .map_err(|e| format!("runSqlQuery error: {:?}", e))?;

            if let Some(resp) = response {
                if let Some(result) = resp.result() {
                    // Simplify result - only keep success, columns, rows
                    json!({
                        "success": result.get("success"),
                        "columns": result.get("columns"),
                        "rows": result.get("rows"),
                    })
                } else {
                    json!({
                        "success": false,
                        "error": "No result from command"
                    })
                }
            } else {
                json!({
                    "success": false,
                    "error": "No response from command"
                })
            }
        }

        _ => return Err(format!("Unknown category: {}", category)),
    };

    if !values_match(&expected, &actual) {
        return Err(format!(
            "Mismatch!\nExpected:\n{}\n\nActual:\n{}",
            serde_json::to_string_pretty(&expected).unwrap(),
            serde_json::to_string_pretty(&actual).unwrap()
        ));
    }

    Ok(())
}

/// Compare JSON values with some flexibility
fn values_match(expected: &Value, actual: &Value) -> bool {
    match (expected, actual) {
        (Value::Object(e), Value::Object(a)) => {
            for (key, exp_val) in e {
                // Skip _comment fields - they're documentation only
                if key == "_comment" {
                    continue;
                }
                if let Some(act_val) = a.get(key) {
                    if !values_match(exp_val, act_val) {
                        return false;
                    }
                } else {
                    return false;
                }
            }
            true
        }
        (Value::Array(e), Value::Array(a)) => {
            if e.len() != a.len() {
                return false;
            }
            e.iter().zip(a.iter()).all(|(ev, av)| values_match(ev, av))
        }
        (Value::Null, Value::Null) => true,
        _ => expected == actual,
    }
}

#[tokio::test]
async fn test_all_lsp_microtests() {
    let tests = get_lsp_test_dirs();
    let mut passed = 0;
    let mut failed = 0;
    let mut failures = Vec::new();
    let mut report = common::CaseReport::new("lsp", "rs-lsp");

    for (name, test_dir, category) in &tests {
        let case_t = std::time::Instant::now();
        let outcome = run_lsp_test(test_dir, category).await;
        let secs = case_t.elapsed().as_secs_f64();
        match outcome {
            Ok(()) => {
                println!("✓ {}", name);
                report.pass(name, secs);
                passed += 1;
            }
            Err(e) => {
                println!("✗ {}", name);
                report.fail(name, &e, secs);
                failures.push((name.clone(), e));
                failed += 1;
            }
        }
    }

    eprintln!("\n✓ {} LSP microtests passed, ✗ {} failed", passed, failed);

    if !failures.is_empty() {
        eprintln!("\nFailures:");
        for (name, error) in &failures {
            eprintln!("\n=== {} ===\n{}", name, error);
        }
    }

    report.finish();
}
