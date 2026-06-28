//! `namespace` mode — group objects by workspace → namespace → kind, plus a top-level
//! (no-namespace) grouping handled by [`super::by_namespace_toplevel`].

use std::collections::HashMap;

use crate::db::QmdcDatabase;

use super::builders::build_object_tree;
use super::by_namespace_toplevel::build_top_level_groups;
use super::Result;

/// Get workspace tree grouped by namespace (default mode)
pub fn get_tree_by_namespace(db: &QmdcDatabase) -> Result<Option<serde_json::Value>> {
    let ws_result = match super::query_workspaces(db) {
        Ok(r) => r,
        Err(j) => return Ok(Some(j)),
    };

    // Debug: check total objects count
    if let Ok(count_result) = db.query("SELECT COUNT(*) FROM objects") {
        if let Some(row) = count_result.rows.first() {
            let total_objects = row[0].as_i64().unwrap_or(0);
            eprintln!("[LSP Tree] Total objects in DB: {}", total_objects);
        }
    }

    let mut workspaces = Vec::new();

    for ws_row in &ws_result.rows {
        let ws_id = ws_row[0].as_str().unwrap_or("");
        let ws_label = ws_row[1].as_str().unwrap_or(ws_id);
        let ws_file = ws_row[2].as_str().unwrap_or("");
        let ws_line = ws_row[3].as_i64().unwrap_or(1);

        eprintln!("[LSP Tree] Processing workspace: {}", ws_id);

        // Build per-namespace kind groups. On a namespace-query failure the whole
        // workspace is skipped (preserves the original `continue` behaviour).
        let (namespaces, ns_ids) = match build_namespace_groups(db, ws_id) {
            Some(v) => v,
            None => continue,
        };

        // Build the top-level (no-namespace) objects and their kind groups.
        let (top_level_kind_groups, all_objects_len) = build_top_level_groups(db, ws_id, &ns_ids);

        let total_count = workspace_total_count(db, ws_id, all_objects_len, &namespaces);

        workspaces.push(serde_json::json!({
            "id": ws_id,
            "label": ws_label,
            "file": ws_file,
            "line": ws_line,
            "count": total_count,
            "kindGroups": top_level_kind_groups,
            "namespaces": namespaces,
        }));
    }

    Ok(Some(serde_json::json!({
        "success": true,
        "mode": "namespace",
        "workspaces": workspaces,
    })))
}

/// Build the `namespaces` array for one workspace and return it alongside the namespace ids.
///
/// Returns `None` when the namespace query fails — the caller skips the whole workspace,
/// matching the original control flow.
fn build_namespace_groups(
    db: &QmdcDatabase,
    ws_id: &str,
) -> Option<(Vec<serde_json::Value>, Vec<String>)> {
    // Query namespaces in this workspace (filter by __workspace field, not file path)
    // Use parameterized query to prevent SQL injection
    let ns_result = match db.query_with_params(
        "SELECT __id, __label, __file, __line FROM objects WHERE __kind = '__Namespace' AND __workspace = ?1 ORDER BY __label COLLATE NOCASE",
        &[&ws_id as &dyn rusqlite::ToSql]
    ) {
        Ok(r) => {
            eprintln!("[LSP Tree] Found {} namespaces for workspace {}", r.rows.len(), ws_id);
            r
        },
        Err(e) => {
            eprintln!("Failed to query namespaces for {}: {}", ws_id, e);
            return None;
        }
    };

    let mut namespaces = Vec::new();

    for ns_row in &ns_result.rows {
        let ns_id = ns_row[0].as_str().unwrap_or("");
        let ns_label = ns_row[1].as_str().unwrap_or(ns_id);
        let ns_file = ns_row[2].as_str().unwrap_or("");
        let ns_line = ns_row[3].as_i64().unwrap_or(1);

        // Query all kinds in this namespace
        // Use parameterized query to prevent SQL injection
        let kind_result = match db.query_with_params(
            "SELECT DISTINCT __kind FROM objects WHERE __kind NOT IN ('__TextBlock', '__Workspace', '__Document', '__Namespace') AND __workspace = ?1 AND (__namespace = ?2 OR __parent = ?2) ORDER BY __kind COLLATE NOCASE",
            &[&ws_id as &dyn rusqlite::ToSql, &ns_id as &dyn rusqlite::ToSql]
        ) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("Failed to query kinds for namespace {}: {}", ns_id, e);
                continue;
            }
        };

        let mut kind_groups = Vec::new();

        // For each kind, query objects and build hierarchy
        for kind_row in &kind_result.rows {
            let kind = kind_row[0].as_str().unwrap_or("");

            // Query objects of this kind in this namespace
            // Use parameterized query to prevent SQL injection
            let obj_result = match db.query_with_params(
                "SELECT __id, __kind, __label, __level, __file, __line, __workspace, __namespace FROM objects WHERE __kind = ?1 AND __workspace = ?2 AND (__namespace = ?3 OR __parent = ?3) ORDER BY __file, __line, __id",
                &[&kind as &dyn rusqlite::ToSql, &ws_id as &dyn rusqlite::ToSql, &ns_id as &dyn rusqlite::ToSql]
            ) {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("Failed to query objects for kind {} in namespace {}: {}", kind, ns_id, e);
                    continue;
                }
            };

            let mut objects = Vec::new();
            let mut objects_by_file: HashMap<String, Vec<serde_json::Value>> = HashMap::new();

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

                let obj = obj_json;

                objects_by_file
                    .entry(obj_file.to_string())
                    .or_default()
                    .push(obj);
            }

            // Build hierarchy for each file (sort files for stable order)
            let mut sorted_files: Vec<_> = objects_by_file.iter().collect();
            sorted_files.sort_by_key(|(file, _)| file.as_str());
            for (_file, file_objects) in sorted_files {
                // Sort objects within file by line, then by id for stable order
                let mut sorted_objs = file_objects.clone();
                sorted_objs.sort_by(|a, b| {
                    let a_line = a.get("line").and_then(|v| v.as_i64()).unwrap_or(0);
                    let b_line = b.get("line").and_then(|v| v.as_i64()).unwrap_or(0);
                    let line_cmp = a_line.cmp(&b_line);
                    if line_cmp != std::cmp::Ordering::Equal {
                        line_cmp
                    } else {
                        // If same line, sort by id for stability
                        let a_id = a.get("id").and_then(|v| v.as_str()).unwrap_or("");
                        let b_id = b.get("id").and_then(|v| v.as_str()).unwrap_or("");
                        a_id.cmp(b_id)
                    }
                });
                let tree = build_object_tree(&sorted_objs);
                objects.extend(tree);
            }

            // Sort all objects: children first, then alphabetically by label (consistent with top-level objects)
            objects.sort_by(|a, b| {
                let a_has_children = a
                    .get("children")
                    .and_then(|c| c.as_array())
                    .map(|arr| !arr.is_empty())
                    .unwrap_or(false);
                let b_has_children = b
                    .get("children")
                    .and_then(|c| c.as_array())
                    .map(|arr| !arr.is_empty())
                    .unwrap_or(false);

                // First sort by presence of children (with children first)
                match (a_has_children, b_has_children) {
                    (true, false) => std::cmp::Ordering::Less,
                    (false, true) => std::cmp::Ordering::Greater,
                    _ => {
                        // Then sort alphabetically by label (case-insensitive)
                        let a_label = a.get("label").and_then(|v| v.as_str()).unwrap_or("");
                        let b_label = b.get("label").and_then(|v| v.as_str()).unwrap_or("");
                        a_label.to_lowercase().cmp(&b_label.to_lowercase())
                    }
                }
            });

            kind_groups.push(serde_json::json!({
                "kind": kind,
                "label": kind,
                "count": objects.len(),
                "objects": objects,
            }));
        }

        namespaces.push(serde_json::json!({
            "id": ns_id,
            "label": ns_label,
            "file": ns_file,
            "line": ns_line,
            "kindGroups": kind_groups,
        }));
    }

    let ns_ids: Vec<String> = ns_result
        .rows
        .iter()
        .map(|row| row[0].as_str().unwrap_or("").to_string())
        .collect();

    Some((namespaces, ns_ids))
}

/// Total object count for a workspace, with a fallback estimate when the COUNT query fails.
fn workspace_total_count(
    db: &QmdcDatabase,
    ws_id: &str,
    all_objects_len: usize,
    namespaces: &[serde_json::Value],
) -> usize {
    // Calculate total count: all objects in workspace (including children like Column objects)
    // Use parameterized query to prevent SQL injection
    match db.query_with_params(
        "SELECT COUNT(*) FROM objects WHERE __workspace = ?1 AND __kind NOT IN ('__TextBlock', '__Document', '__Workspace', '__Namespace')",
        &[&ws_id as &dyn rusqlite::ToSql]
    ) {
        Ok(result) => {
            if let Some(row) = result.rows.first() {
                row[0].as_i64().unwrap_or(0) as usize
            } else {
                0
            }
        },
        Err(e) => {
            eprintln!("ERROR: Failed to count objects for workspace {}: {}", ws_id, e);
            // Fallback: estimate count from already loaded data (may be inaccurate, doesn't include all children)
            // This is better than failing completely, but count may be lower than actual
            let mut fallback_count = all_objects_len;
            for ns in namespaces {
                if let Some(kind_groups) = ns.get("kindGroups").and_then(|v| v.as_array()) {
                    for kg in kind_groups {
                        if let Some(count) = kg.get("count").and_then(|v| v.as_i64()) {
                            fallback_count += count as usize;
                        }
                    }
                }
            }
            eprintln!("WARNING: Using fallback count {} for workspace {} (may be inaccurate)", fallback_count, ws_id);
            fallback_count
        }
    }
}
