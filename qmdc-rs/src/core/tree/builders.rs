//! Pure flat-list → hierarchy tree builders. No DB access; operate on JSON object slices.

use std::collections::HashMap;

/// Build hierarchical object tree from flat list based on __level field
pub fn build_object_tree(objects: &[serde_json::Value]) -> Vec<serde_json::Value> {
    if objects.is_empty() {
        return Vec::new();
    }

    // Get the first object's level as the base level
    let base_level = objects[0]
        .get("level")
        .and_then(|v| v.as_i64())
        .unwrap_or(1);

    let mut result = Vec::new();
    let mut stack: Vec<(serde_json::Value, i64)> = Vec::new(); // (object, level)

    for obj in objects {
        let level = obj.get("level").and_then(|v| v.as_i64()).unwrap_or(1);

        // Pop stack until we find a parent (level < current)
        while let Some((_, parent_level)) = stack.last() {
            if *parent_level < level {
                break;
            }
            stack.pop();
        }

        let mut obj_with_children = obj.clone();
        if let Some(map) = obj_with_children.as_object_mut() {
            map.insert("children".to_string(), serde_json::json!([]));
        }

        if let Some((parent, _)) = stack.last_mut() {
            // Add as child to parent
            if let Some(parent_map) = parent.as_object_mut() {
                if let Some(children) = parent_map.get_mut("children") {
                    if let Some(children_arr) = children.as_array_mut() {
                        children_arr.push(obj_with_children.clone());
                    }
                }
            }
        } else if level == base_level {
            // Top-level object
            result.push(obj_with_children.clone());
        }

        stack.push((obj_with_children, level));
    }

    // Sort result by id for stable ordering (objects at same level)
    // Also sort children recursively
    fn sort_tree_recursive(obj: &mut serde_json::Value) {
        if let Some(obj_map) = obj.as_object_mut() {
            if let Some(children) = obj_map.get_mut("children") {
                if let Some(children_arr) = children.as_array_mut() {
                    // Sort children by id
                    children_arr.sort_by(|a, b| {
                        let a_id = a.get("id").and_then(|v| v.as_str()).unwrap_or("");
                        let b_id = b.get("id").and_then(|v| v.as_str()).unwrap_or("");
                        a_id.cmp(b_id)
                    });
                    // Recursively sort children's children
                    for child in children_arr.iter_mut() {
                        sort_tree_recursive(child);
                    }
                }
            }
        }
    }

    result.sort_by(|a, b| {
        let a_id = a.get("id").and_then(|v| v.as_str()).unwrap_or("");
        let b_id = b.get("id").and_then(|v| v.as_str()).unwrap_or("");
        a_id.cmp(b_id)
    });

    // Recursively sort children
    for obj in result.iter_mut() {
        sort_tree_recursive(obj);
    }

    result
}

/// Build smart hierarchical tree using __parent field for logical hierarchy
/// This creates a tree based on explicit parent-child relationships (Column → Table)
pub fn build_object_tree_smart(objects: &[serde_json::Value]) -> Vec<serde_json::Value> {
    if objects.is_empty() {
        return Vec::new();
    }

    // Step 1: Prepare all objects with empty children arrays
    let mut objects_map: HashMap<String, serde_json::Value> = HashMap::new();
    let mut root_ids: std::collections::HashSet<String> = std::collections::HashSet::new();

    for obj in objects {
        if let Some(id) = obj.get("id").and_then(|v| v.as_str()) {
            let mut obj_with_children = obj.clone();
            if let Some(map) = obj_with_children.as_object_mut() {
                map.insert("children".to_string(), serde_json::json!([]));
            }
            objects_map.insert(id.to_string(), obj_with_children);
            root_ids.insert(id.to_string());
        }
    }

    // Step 2: Build parent → child relationships
    // Important: iterate over original objects vector to preserve SQL ORDER BY
    let mut parent_child_pairs: Vec<(String, String)> = Vec::new(); // (child_id, parent_id)

    for obj in objects {
        let child_id = match obj.get("id").and_then(|v| v.as_str()) {
            Some(id) => id,
            None => continue,
        };

        // Try __parent first, then __namespace as fallback
        // In database, __parent and __namespace are stored as plain ID (not [[#id]] format)
        let parent_ref = obj
            .get("parent")
            .and_then(|v| v.as_str())
            .or_else(|| obj.get("namespace").and_then(|v| v.as_str()));

        if let Some(parent_ref) = parent_ref {
            // Extract ID from "[[#parent_id]]" format if present, otherwise use as-is (already plain ID)
            let parent_id = if let Some(id_part) = parent_ref
                .strip_prefix("[[#")
                .and_then(|s| s.strip_suffix("]]"))
            {
                id_part.trim()
            } else {
                parent_ref.trim()
            };

            if !parent_id.is_empty() {
                // Check if parent is a workspace (workspaces are not in objects_map, but objects with __parent = workspace should be root)
                // Check if this parent_id is a workspace by querying the original objects
                let is_workspace_parent = objects.iter().any(|obj| {
                    obj.get("id").and_then(|v| v.as_str()) == Some(parent_id)
                        && obj.get("kind").and_then(|v| v.as_str()) == Some("__Workspace")
                });

                if is_workspace_parent {
                    // Parent is workspace - this object should be root (don't attach to workspace)
                    // Keep it as root (don't remove from root_ids)
                } else if objects_map.contains_key(parent_id) {
                    // Parent exists in our objects - attach as child
                    parent_child_pairs.push((child_id.to_string(), parent_id.to_string()));
                    // Mark child as non-root
                    root_ids.remove(child_id);
                }
                // If parent doesn't exist and is not workspace - keep as root (broken reference)
            }
        }
    }

    // Step 3: Build parent→children mapping
    let mut children_of: HashMap<String, Vec<String>> = HashMap::new(); // parent_id -> [child_ids]
    for (child_id, parent_id) in &parent_child_pairs {
        children_of
            .entry(parent_id.clone())
            .or_default()
            .push(child_id.clone());
    }

    // Step 3.5: Sort children for each parent
    // Pre-compute which IDs have children (for sorting)
    let ids_with_children: std::collections::HashSet<String> =
        children_of.keys().cloned().collect();
    let parent_ids: Vec<String> = children_of.keys().cloned().collect();
    for parent_id in &parent_ids {
        if let Some(children) = children_of.get_mut(parent_id) {
            children.sort_by(|a, b| {
                let a_has = ids_with_children.contains(a);
                let b_has = ids_with_children.contains(b);
                match (a_has, b_has) {
                    (true, false) => std::cmp::Ordering::Less,
                    (false, true) => std::cmp::Ordering::Greater,
                    _ => {
                        let a_label = objects_map
                            .get(a)
                            .and_then(|o| o.get("label"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        let b_label = objects_map
                            .get(b)
                            .and_then(|o| o.get("label"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        a_label.to_lowercase().cmp(&b_label.to_lowercase())
                    }
                }
            });
        }
    }

    // Step 4: Build tree recursively from objects_map
    fn build_subtree(
        id: &str,
        objects_map: &HashMap<String, serde_json::Value>,
        children_of: &HashMap<String, Vec<String>>,
    ) -> serde_json::Value {
        let mut obj = objects_map
            .get(id)
            .cloned()
            .unwrap_or(serde_json::json!({}));
        if let Some(obj_map) = obj.as_object_mut() {
            let children_arr = if let Some(child_ids) = children_of.get(id) {
                child_ids
                    .iter()
                    .map(|cid| build_subtree(cid, objects_map, children_of))
                    .collect()
            } else {
                Vec::new()
            };
            obj_map.insert("children".to_string(), serde_json::json!(children_arr));
        }
        obj
    }

    // Step 5: Return only root objects, preserving SQL order and stable sort
    let mut roots: Vec<serde_json::Value> = Vec::new();
    let mut seen_ids: std::collections::HashSet<String> = std::collections::HashSet::new();

    for obj in objects {
        if let Some(id) = obj.get("id").and_then(|v| v.as_str()) {
            if root_ids.contains(id) && !seen_ids.contains(id) {
                roots.push(build_subtree(id, &objects_map, &children_of));
                seen_ids.insert(id.to_string());
            }
        }
    }

    // Sort: objects with children first, then alphabetically by label (stable sort)
    roots.sort_by(|a, b| {
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

    roots
}
