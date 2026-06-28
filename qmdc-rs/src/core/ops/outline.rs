//! Core `outline` operation — nested document-symbol tree for a file.
//!
//! Returns a tree of objects grouped by their position in the file (by line order),
//! respecting `__level` for nesting.

use serde_json::{json, Value};

use crate::core::error::{ErrorCode, ErrorEnvelope};
use crate::core::fields::QmdcObject;
use crate::core::resolved_index::ResolvedIndex;

/// Produce a nested document-symbol outline for a given file.
///
/// # Arguments
/// * `index` — the resolved workspace index.
/// * `file` — the relative file path within the workspace (e.g. `"schema.qmd.md"`).
///
/// # Returns
/// - `Ok(Value)` — success envelope with `{ file, symbols: [...] }`.
/// - `Err(Value)` — `invalid-argument` if file is empty; `not-found` if file not in workspace.
pub fn outline(index: &ResolvedIndex, file: &str) -> Result<Value, Value> {
    let file = file.trim();

    if file.is_empty() {
        return Err(ErrorEnvelope::error(
            ErrorCode::InvalidArgument,
            "file must not be empty",
        ));
    }

    // Check if the file is in the workspace
    let file_exists = index.files().iter().any(|f| f == file);
    if !file_exists {
        return Err(ErrorEnvelope::error(
            ErrorCode::NotFound,
            format!("file '{}' not found in workspace", file),
        ));
    }

    // Collect objects belonging to this file, sorted by line
    let mut file_objects: Vec<&Value> = index
        .objects()
        .iter()
        .filter(|obj| obj.get("__file").and_then(|v| v.as_str()) == Some(file))
        .collect();

    file_objects.sort_by_key(|obj| obj.line());

    // Build the nested outline using the SAME hierarchy builder the `get_tree` smart mode
    // uses (`__parent` → `__level` fallback), so the outline nests and orders identically to
    // the workspace tree (children-first, then alphabetical by name) instead of flattening.
    let normalized: Vec<Value> = file_objects.iter().map(|o| normalize_for_tree(o)).collect();
    let tree = crate::core::tree::build_object_tree_smart(&normalized);
    let symbols: Vec<Value> = tree.iter().map(to_symbol).collect();

    // NFR-4: bound the top-level symbol list, surfacing truncation alongside the domain key.
    let (symbols, truncated, remaining) =
        crate::core::envelope::bound_list(symbols, crate::core::envelope::DEFAULT_LIMIT);
    let mut body = json!({
        "file": file,
        "symbols": symbols,
        "truncated": truncated,
    });
    if truncated {
        body["remaining"] = json!(remaining);
    }
    Ok(ErrorEnvelope::success(body))
}

/// Normalize a parsed object into the flat shape `build_object_tree_smart` expects
/// (`id, kind, label, level, file, line, parent, namespace`).
fn normalize_for_tree(obj: &Value) -> Value {
    let id = obj.id();
    let label = obj.get("__label").and_then(|v| v.as_str()).unwrap_or(id);
    json!({
        "id": id,
        "kind": obj.kind(),
        "label": label,
        "level": obj.get("__level").and_then(|v| v.as_i64()).unwrap_or(1),
        "file": obj.file(),
        "line": obj.line(),
        "parent": obj.get("__parent").and_then(|v| v.as_str()),
        "namespace": obj.get("__namespace").and_then(|v| v.as_str()),
    })
}

/// Map a tree node (from `build_object_tree_smart`) to the outline symbol shape
/// `{ id, kind, name, line, level, children }`, recursively.
fn to_symbol(node: &Value) -> Value {
    let id = node.get("id").and_then(|v| v.as_str()).unwrap_or("");
    let name = node
        .get("label")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or(id);
    let children: Vec<Value> = node
        .get("children")
        .and_then(|c| c.as_array())
        .map(|arr| arr.iter().map(to_symbol).collect())
        .unwrap_or_default();
    json!({
        "id": id,
        "kind": node.get("kind").and_then(|v| v.as_str()).unwrap_or(""),
        "name": name,
        "line": node.get("line").and_then(|v| v.as_i64()).unwrap_or(0),
        "level": node.get("level").and_then(|v| v.as_i64()).unwrap_or(1),
        "children": children,
    })
}
