//! `file` mode — group objects by workspace → file, with per-file `__level` hierarchy.

use crate::db::QmdcDatabase;

use super::builders::build_object_tree;
use super::Result;

/// Get workspace tree grouped by file
pub fn get_tree_by_file(db: &QmdcDatabase) -> Result<Option<serde_json::Value>> {
    let ws_result = match super::query_workspaces(db) {
        Ok(r) => r,
        Err(j) => return Ok(Some(j)),
    };

    let mut workspaces = Vec::new();

    for ws_row in &ws_result.rows {
        let ws_id = ws_row[0].as_str().unwrap_or("");
        let ws_label = ws_row[1].as_str().unwrap_or(ws_id);
        let ws_file = ws_row[2].as_str().unwrap_or("");
        let ws_line = ws_row[3].as_i64().unwrap_or(1);

        // Query all files with objects (filter by __workspace field)
        // Use parameterized query to prevent SQL injection
        let file_result = match db.query_with_params(
            "SELECT DISTINCT __file FROM objects WHERE __kind NOT IN ('__TextBlock', '__Workspace', '__Document', '__Namespace') AND __workspace = ?1 ORDER BY __file",
            &[&ws_id as &dyn rusqlite::ToSql]
        ) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("Failed to query files for {}: {}", ws_id, e);
                continue;
            }
        };

        let mut file_groups = Vec::new();

        for file_row in &file_result.rows {
            let file_path = file_row[0].as_str().unwrap_or("");
            let file_name = file_path.rsplit('/').next().unwrap_or(file_path);

            // Query objects in this file
            // Use parameterized query to prevent SQL injection
            let obj_result = match db.query_with_params(
                "SELECT __id, __kind, __label, __level, __file, __line, __workspace, __namespace FROM objects WHERE __kind NOT IN ('__TextBlock', '__Workspace', '__Document', '__Namespace') AND __file = ?1 ORDER BY __line",
                &[&file_path as &dyn rusqlite::ToSql]
            ) {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("Failed to query objects for file {}: {}", file_path, e);
                    continue;
                }
            };

            let mut objects = Vec::new();

            for obj_row in &obj_result.rows {
                let obj_id = obj_row[0].as_str().unwrap_or("");
                let obj_kind = obj_row[1].as_str().unwrap_or("");
                let obj_label = obj_row[2].as_str().unwrap_or(obj_id);
                let obj_level = obj_row[3].as_i64().unwrap_or(1);
                let obj_file = obj_row[4].as_str().unwrap_or("");
                let obj_line = obj_row[5].as_i64().unwrap_or(1);
                let obj_workspace = obj_row[6].as_str().unwrap_or("");
                let obj_namespace = obj_row[7].as_str().unwrap_or("");

                let mut obj_json = serde_json::json!({
                    "id": obj_id,
                    "kind": obj_kind,
                    "label": obj_label,
                    "level": obj_level,
                    "file": obj_file,
                    "line": obj_line,
                });

                // Add workspace and namespace if present
                if let Some(obj_map) = obj_json.as_object_mut() {
                    if !obj_workspace.is_empty() {
                        obj_map.insert("workspace".to_string(), serde_json::json!(obj_workspace));
                    }
                    if !obj_namespace.is_empty() {
                        obj_map.insert("namespace".to_string(), serde_json::json!(obj_namespace));
                    }
                }

                objects.push(obj_json);
            }

            // Build hierarchy
            let tree = build_object_tree(&objects);

            file_groups.push(serde_json::json!({
                "file": file_path,
                "label": file_name,
                "count": tree.len(),
                "objects": tree,
            }));
        }

        workspaces.push(serde_json::json!({
            "id": ws_id,
            "label": ws_label,
            "file": ws_file,
            "line": ws_line,
            "fileGroups": file_groups,
        }));
    }

    Ok(Some(serde_json::json!({
        "success": true,
        "mode": "file",
        "workspaces": workspaces,
    })))
}
