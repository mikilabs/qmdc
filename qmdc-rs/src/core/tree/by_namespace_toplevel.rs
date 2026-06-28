//! `namespace` mode — the top-level (no-namespace) object grouping.
//!
//! Objects that do not belong to any namespace are grouped by kind at the workspace root.
//! Filtering out namespace members and nested descendants is done with recursive SQL CTEs
//! built by [`build_parent_condition`].

use crate::db::QmdcDatabase;

use super::builders::build_object_tree_smart;

/// Build the top-level (no-namespace) kind groups for a workspace.
///
/// Returns `(top_level_kind_groups, all_objects_len)`. The second value is the number of
/// no-namespace objects loaded, used by the caller's fallback count estimate.
pub(super) fn build_top_level_groups(
    db: &QmdcDatabase,
    ws_id: &str,
    ns_ids: &[String],
) -> (Vec<serde_json::Value>, usize) {
    let parent_condition = build_parent_condition(ws_id, ns_ids);

    // Query ALL objects without namespace (not just top-level)
    // build_object_tree_smart will filter to only top-level objects and build the tree
    // Use parameterized query to prevent SQL injection
    let all_objects_result = match db.query_with_params(
        "SELECT __id, __kind, __label, __level, __file, __line, __parent, __workspace, __namespace FROM objects WHERE __workspace = ?1 AND __kind NOT IN ('__TextBlock', '__Document', '__Workspace', '__Namespace') AND (__namespace IS NULL OR __namespace = '') ORDER BY __file, __line, __id",
        &[&ws_id as &dyn rusqlite::ToSql]
    ) {
        Ok(r) => {
            eprintln!("[LSP Tree] Found {} objects without namespace for workspace {}", r.rows.len(), ws_id);
            r
        },
        Err(e) => {
            eprintln!("ERROR: Failed to query objects without namespace for workspace {}: {}", ws_id, e);
            eprintln!("Continuing with empty objects array - workspace may be incomplete");
            // Continue with empty QueryResult to allow workspace to be returned with namespaces only
            // Error is logged but not propagated to avoid breaking the entire response
            crate::db::QueryResult {
                columns: vec!["__id".to_string(), "__kind".to_string(), "__label".to_string(), "__level".to_string(), "__file".to_string(), "__line".to_string(), "__parent".to_string()],
                rows: Vec::new(),
            }
        }
    };

    // Get top-level object IDs for filtering (separate query needed due to recursive CTE complexity)
    // This is more efficient than filtering in application code after building the tree
    let top_level_ids_query = format!(
        "SELECT __id FROM objects WHERE __workspace = '{}' AND __kind NOT IN ('__TextBlock', '__Document', '__Workspace', '__Namespace') AND (__namespace IS NULL OR __namespace = '') AND {}",
        ws_id.replace('\'', "''"),
        parent_condition
    );
    let top_level_ids: std::collections::HashSet<String> = match db.query(&top_level_ids_query) {
        Ok(r) => {
            let ids: std::collections::HashSet<String> = r
                .rows
                .iter()
                .map(|row| row[0].as_str().unwrap_or("").to_string())
                .collect();
            eprintln!(
                "[LSP Tree] Found {} top-level object IDs for workspace {}",
                ids.len(),
                ws_id
            );
            ids
        }
        Err(e) => {
            eprintln!(
                "ERROR: Failed to query top-level object IDs for workspace {}: {}",
                ws_id, e
            );
            eprintln!("Continuing with empty top-level IDs set - all objects will be filtered out");
            std::collections::HashSet::new()
        }
    };

    // Build JSON objects for build_object_tree_smart (include ALL objects, not just top-level)
    let mut all_objects = Vec::new();
    for obj_row in &all_objects_result.rows {
        let obj_id = obj_row[0].as_str().unwrap_or("");
        let obj_kind = obj_row[1].as_str().unwrap_or("");
        let obj_label = obj_row[2].as_str().unwrap_or(obj_id);
        let obj_level = obj_row[3].as_i64().unwrap_or(1);
        let obj_file = obj_row[4].as_str().unwrap_or("");
        let obj_line = obj_row[5].as_i64().unwrap_or(1);
        let obj_parent = obj_row[6].as_str().unwrap_or("");
        let obj_workspace = obj_row[7].as_str().unwrap_or("");
        let obj_namespace = obj_row[8].as_str().unwrap_or("");

        let mut obj = serde_json::json!({
            "id": obj_id,
            "kind": obj_kind,
            "label": obj_label,
            "level": obj_level,
            "file": obj_file,
            "line": obj_line,
        });

        // Add parent, workspace, and namespace fields if they exist
        if let Some(obj_map) = obj.as_object_mut() {
            if !obj_parent.is_empty() {
                obj_map.insert("parent".to_string(), serde_json::json!(obj_parent));
            }
            if !obj_workspace.is_empty() {
                obj_map.insert("workspace".to_string(), serde_json::json!(obj_workspace));
            }
            if !obj_namespace.is_empty() {
                obj_map.insert("namespace".to_string(), serde_json::json!(obj_namespace));
            }
        }

        all_objects.push(obj);
    }
    let all_objects_len = all_objects.len();

    // Build tree using build_object_tree_smart (sorts with children first, then by label)
    // This will build the full tree, but we need to filter to only top-level objects
    let full_tree = build_object_tree_smart(&all_objects);

    // Filter to only top-level objects (those whose ID is in top_level_ids)
    let mut top_level_tree: Vec<serde_json::Value> = full_tree
        .into_iter()
        .filter(|obj| {
            obj.get("id")
                .and_then(|v| v.as_str())
                .map(|id| top_level_ids.contains(id))
                .unwrap_or(false)
        })
        .collect();

    // Remove 'parent' field from objects (not needed in namespace mode output, only in smart mode)
    fn remove_parent_field_recursive(obj: &mut serde_json::Value) {
        if let Some(obj_map) = obj.as_object_mut() {
            obj_map.remove("parent");

            // Recursively process children
            if let Some(children) = obj_map.get_mut("children") {
                if let Some(children_arr) = children.as_array_mut() {
                    for child in children_arr.iter_mut() {
                        remove_parent_field_recursive(child);
                    }
                }
            }
        }
    }

    for obj in top_level_tree.iter_mut() {
        remove_parent_field_recursive(obj);
    }

    // Group top-level objects by kind (similar to namespaces)
    let mut top_level_kind_groups = Vec::new();

    // Extract unique kinds from top_level_tree
    let mut kinds_set: std::collections::HashSet<String> = std::collections::HashSet::new();
    for obj in &top_level_tree {
        if let Some(kind) = obj.get("kind").and_then(|v| v.as_str()) {
            kinds_set.insert(kind.to_string());
        }
    }

    // Sort kinds alphabetically (case-insensitive) for stable ordering
    let mut kinds: Vec<String> = kinds_set.into_iter().collect();
    kinds.sort_by_key(|a| a.to_lowercase());

    // Group objects by kind
    for kind in kinds {
        let kind_objects: Vec<serde_json::Value> = top_level_tree
            .iter()
            .filter(|obj| obj.get("kind").and_then(|v| v.as_str()) == Some(kind.as_str()))
            .cloned()
            .collect();

        if !kind_objects.is_empty() {
            top_level_kind_groups.push(serde_json::json!({
                "kind": kind,
                "label": kind,
                "count": kind_objects.len(),
                "objects": kind_objects,
            }));
        }
    }

    (top_level_kind_groups, all_objects_len)
}

/// Build the SQL `__parent`-filter condition that excludes namespace members and the
/// descendants of top-level objects, so only genuine top-level roots remain.
///
/// Uses recursive CTEs. Note: performance may degrade on large workspaces with deep parent
/// chains; consider indexes on `__parent` / `__namespace` for optimization.
fn build_parent_condition(ws_id: &str, ns_ids: &[String]) -> String {
    if ns_ids.is_empty() {
        // No namespaces: top-level objects are those with __parent IS NULL or __parent = workspace_id
        // But exclude objects whose parent is another top-level object (or its descendant)
        // Use two-step approach: first find top-level objects, then exclude their descendants
        format!(
            "(__parent IS NULL OR __parent = '{}' OR __parent NOT IN (
                WITH RECURSIVE top_level_roots(id) AS (
                    -- Start with top-level objects (NULL or workspace_id)
                    SELECT __id FROM objects 
                    WHERE __workspace = '{}' 
                      AND __kind NOT IN ('__TextBlock', '__Document', '__Workspace', '__Namespace')
                      AND (__namespace IS NULL OR __namespace = '')
                      AND (__parent IS NULL OR __parent = '{}')
                ),
                top_level_descendants(id) AS (
                    -- Start with top-level roots
                    SELECT id FROM top_level_roots
                    UNION
                    -- Recursively find all descendants of top-level objects
                    SELECT o.__id FROM objects o
                    INNER JOIN top_level_descendants tld ON o.__parent = tld.id
                    WHERE o.__workspace = '{}'
                )
                SELECT id FROM top_level_descendants
            ))",
            ws_id.replace('\'', "''"),
            ws_id.replace('\'', "''"),
            ws_id.replace('\'', "''"),
            ws_id.replace('\'', "''")
        )
    } else {
        let ns_ids_escaped: Vec<String> = ns_ids
            .iter()
            .map(|id| format!("'{}'", id.replace('\'', "''")))
            .collect();
        // Exclude: parent = namespace_id OR parent points to object in namespace
        // Also exclude top-level objects and their descendants (separate CTE to avoid circular reference)
        format!(
            "(__parent IS NULL OR __parent = '{}' OR __parent NOT IN (
                WITH RECURSIVE namespace_descendants(id) AS (
                    -- Start with namespaces
                    SELECT __id FROM objects 
                    WHERE __workspace = '{}' 
                      AND __kind = '__Namespace'
                      AND __id IN ({})
                    UNION
                    -- Recursively find all descendants of namespaces
                    SELECT o.__id FROM objects o
                    INNER JOIN namespace_descendants nd ON o.__parent = nd.id
                    WHERE o.__workspace = '{}'
                ),
                top_level_roots(id) AS (
                    -- Find top-level objects (NULL or workspace_id, not in namespace)
                    SELECT __id FROM objects 
                    WHERE __workspace = '{}' 
                      AND __kind NOT IN ('__TextBlock', '__Document', '__Workspace', '__Namespace')
                      AND (__namespace IS NULL OR __namespace = '')
                      AND (__parent IS NULL OR __parent = '{}')
                      AND __id NOT IN (SELECT id FROM namespace_descendants)
                ),
                top_level_descendants(id) AS (
                    -- Start with top-level roots
                    SELECT id FROM top_level_roots
                    UNION
                    -- Recursively find all descendants of top-level objects
                    SELECT o.__id FROM objects o
                    INNER JOIN top_level_descendants tld ON o.__parent = tld.id
                    WHERE o.__workspace = '{}'
                      AND o.__id NOT IN (SELECT id FROM namespace_descendants)
                ),
                excluded_parents(id) AS (
                    SELECT id FROM namespace_descendants
                    UNION
                    SELECT id FROM top_level_descendants
                )
                SELECT DISTINCT id FROM excluded_parents
            ))",
            ws_id.replace('\'', "''"),
            ws_id.replace('\'', "''"),
            ns_ids_escaped.join(", "),
            ws_id.replace('\'', "''"),
            ws_id.replace('\'', "''"),
            ws_id.replace('\'', "''"),
            ws_id.replace('\'', "''")
        )
    }
}
