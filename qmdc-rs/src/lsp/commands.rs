use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::Client;

use super::document::Document;
use super::sql_rewrite;
use super::workspace::WorkspaceIndex;
use crate::core::tree;
use crate::db::QmdcDatabase;

/// Handle qmdc.dumpIndex command
pub async fn handle_dump_index(
    client: &Client,
    workspaces: &Arc<RwLock<WorkspaceIndex>>,
    documents: &Arc<RwLock<HashMap<Url, Document>>>,
) -> Result<Option<serde_json::Value>> {
    let ws_index = workspaces.read().await;
    let docs = documents.read().await;

    let mut output = String::new();
    output.push_str("=== QMDC Workspace Index ===\n\n");

    if ws_index.by_uri.is_empty() {
        output.push_str("No workspaces found!\n");
    } else {
        for ws in ws_index.by_uri.values() {
            output.push_str(&format!("Workspace: '{}'\n", ws.id));
            output.push_str(&format!("  Root: {}\n", ws.root_path.display()));
            output.push_str(&format!("  Files: {}\n", ws.files.len()));
            output.push_str(&format!("  Objects ({}):\n", ws.objects.len()));

            for (id, objs) in &ws.objects {
                for obj in objs {
                    let kind = obj.get("__kind").and_then(|v| v.as_str()).unwrap_or("?");
                    let file = obj.get("__file").and_then(|v| v.as_str()).unwrap_or("?");
                    output.push_str(&format!("    - {} [{}] in {}\n", id, kind, file));
                }
            }
            output.push('\n');
        }
    }

    output.push_str("=== Open Documents ===\n\n");
    for (uri, doc) in docs.iter() {
        output.push_str(&format!("Document: {}\n", uri));
        output.push_str(&format!("  Objects: {}\n", doc.objects.len()));
        for obj in &doc.objects {
            let id = obj.get("__id").and_then(|v| v.as_str()).unwrap_or("?");
            let kind = obj.get("__kind").and_then(|v| v.as_str()).unwrap_or("?");
            output.push_str(&format!("    - {} [{}]\n", id, kind));
        }
    }

    client.log_message(MessageType::INFO, output).await;
    Ok(None)
}

/// Handle qmdc.getWorkspaceTree command  
pub fn handle_get_workspace_tree(
    db: &QmdcDatabase,
    grouping_mode: &str,
) -> Result<Option<serde_json::Value>> {
    // Core tree builders are the single source of truth; they never return Err
    // (errors are surfaced in the JSON), so map the String-result to LSP's result.
    let built = match grouping_mode {
        "file" => tree::get_tree_by_file(db),
        "smart" => tree::get_tree_by_smart(db),
        _ => tree::get_tree_by_namespace(db),
    };
    Ok(built.unwrap_or(None))
}

/// Handle qmdc.runSqlQuery command
pub fn handle_run_sql_query(
    db: &QmdcDatabase,
    query_input: &str,
    document_uri: Option<&str>,
    scope: Option<&str>,
    workspaces: &WorkspaceIndex,
) -> Result<Option<serde_json::Value>> {
    if query_input.is_empty() {
        return Ok(Some(serde_json::json!({
            "error": "No SQL query provided"
        })));
    }

    // Resolve #query_id to actual SQL
    let sql = if let Some(query_id) = query_input.strip_prefix('#') {
        // Find Query object by ID and extract sql field
        match db.query(&format!(
            "SELECT json_extract(data, '$.sql') as sql FROM objects WHERE __id = '{}' AND __kind = 'Query'",
            query_id.replace('\'', "''") // Escape single quotes
        )) {
            Ok(result) if !result.rows.is_empty() => {
                match &result.rows[0][0] {
                    serde_json::Value::String(s) => s.clone(),
                    _ => {
                        return Ok(Some(serde_json::json!({
                            "success": false,
                            "error": format!("Query object '{}' has no 'sql' field", query_id)
                        })));
                    }
                }
            }
            Ok(_) => {
                return Ok(Some(serde_json::json!({
                    "success": false,
                    "error": format!("Query object '{}' not found", query_id)
                })));
            }
            Err(e) => {
                return Ok(Some(serde_json::json!({
                    "success": false,
                    "error": format!("Failed to resolve query '{}': {}", query_id, e)
                })));
            }
        }
    } else {
        query_input.to_string()
    };

    // Apply workspace filter if document_uri is provided and scope != "all"
    let final_sql = if let Some(uri_str) = document_uri {
        if scope != Some("all") {
            // Find workspace for this document URI
            if let Ok(uri) = tower_lsp::lsp_types::Url::parse(uri_str) {
                if let Ok(file_path) = uri.to_file_path() {
                    // Find workspace whose root contains this file
                    if let Some(ws) = workspaces
                        .by_uri
                        .values()
                        .find(|ws| file_path.starts_with(&ws.root_path))
                    {
                        eprintln!(
                            "[LSP] Found workspace '{}' for file: {}",
                            ws.id,
                            file_path.display()
                        );
                        // Rewrite SQL to add workspace filter
                        match sql_rewrite::rewrite_sql_for_workspace(&sql, &ws.id) {
                            Ok(rewritten) => {
                                eprintln!("[LSP] SQL rewritten for workspace '{}'", ws.id);
                                eprintln!("[LSP] Original SQL: {}", sql);
                                eprintln!("[LSP] Rewritten SQL: {}", rewritten);
                                rewritten
                            }
                            Err(e) => {
                                // If rewrite fails, log warning but continue with original SQL
                                eprintln!("[LSP] SQL rewrite failed: {}, using original SQL", e);
                                sql
                            }
                        }
                    } else {
                        // No workspace found, use original SQL
                        eprintln!(
                            "[LSP] No workspace found for file: {} (workspaces: {})",
                            file_path.display(),
                            workspaces.by_uri.len()
                        );
                        eprintln!("[LSP] Available workspace roots:");
                        for ws in workspaces.by_uri.values() {
                            eprintln!("[LSP]   - {} (root: {})", ws.id, ws.root_path.display());
                        }
                        sql
                    }
                } else {
                    // Invalid file path, use original SQL
                    eprintln!("[LSP] Failed to convert URI to file path: {}", uri_str);
                    sql
                }
            } else {
                // Invalid URI, use original SQL
                eprintln!("[LSP] Failed to parse URI: {}", uri_str);
                sql
            }
        } else {
            // scope == "all", no rewrite
            eprintln!("[LSP] Scope is 'all', skipping workspace filter");
            sql
        }
    } else {
        // No document_uri, use original SQL (raw mode)
        eprintln!("[LSP] No document_uri provided, using raw SQL");
        sql
    };

    match db.query(&final_sql) {
        Ok(result) => {
            let obj_count = db.object_count().unwrap_or(0);
            let edge_count = db.edge_count().unwrap_or(0);

            Ok(Some(serde_json::json!({
                "success": true,
                "columns": result.columns,
                "rows": result.rows,
                "row_count": result.rows.len(),
                "table": result.to_table_string(),
                "stats": {
                    "objects": obj_count,
                    "edges": edge_count
                }
            })))
        }
        Err(e) => Ok(Some(serde_json::json!({
            "success": false,
            "error": e
        }))),
    }
}
