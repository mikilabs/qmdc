//! Core `search` operation — substring match on workspace symbols.
//!
//! Performs case-insensitive substring matching on `__id` and `__label` (matching the LSP
//! `workspace_symbol` handler). Results are keyset-paginated by `__global_id` (NFR-4): a
//! `cursor` resumes strictly after the last returned object, robust to edits between pages.

use serde_json::{json, Value};

use crate::core::envelope::cursor_page;
use crate::core::error::{ErrorCode, ErrorEnvelope};
use crate::core::fields::QmdcObject;
use crate::core::resolved_index::ResolvedIndex;

/// Search workspace symbols by substring match.
///
/// # Arguments
/// * `index` — the resolved workspace index.
/// * `query` — case-insensitive substring matched against `__id` and `__label`.
/// * `namespace_filter` — optional: restrict results to this namespace.
/// * `limit` — max results per page.
/// * `cursor` — opaque keyset cursor; results resume after it (None = first page).
///
/// # Returns
/// - `Ok(Value)` — `{ success, items, truncated, [next_cursor] }`.
/// - `Err(Value)` — `invalid-argument` if query is empty.
pub fn search(
    index: &ResolvedIndex,
    query: &str,
    namespace_filter: Option<&str>,
    limit: usize,
    cursor: Option<&str>,
) -> Result<Value, Value> {
    let query = query.trim();

    if query.is_empty() {
        return Err(ErrorEnvelope::error(
            ErrorCode::InvalidArgument,
            "query must not be empty",
        ));
    }

    let query_lower = query.to_lowercase();

    // Build (sort_key, item) pairs for matching objects; sort_key = __global_id.
    let mut matches: Vec<(String, Value)> = Vec::new();

    for obj in index.objects() {
        let obj_id = obj.id();
        let obj_label = obj.label();
        let obj_namespace = obj.namespace_id();

        if let Some(filter) = namespace_filter {
            if !obj_namespace.eq_ignore_ascii_case(filter) {
                continue;
            }
        }

        let id_match = obj_id.to_lowercase().contains(&query_lower);
        let label_match = obj_label.to_lowercase().contains(&query_lower);

        if id_match || label_match {
            let name = if obj_label.is_empty() {
                obj_id
            } else {
                obj_label
            };
            let item = json!({
                "id": obj_id,
                "name": name,
                "kind": obj.kind(),
                "file": obj.file(),
                "line": obj.line(),
                "namespace": obj_namespace,
            });
            matches.push((obj.global_id(), item));
        }
    }

    let page = cursor_page(matches, limit, cursor);
    let mut body = json!({
        "items": page.items,
        "total": page.total,
        "truncated": page.truncated,
    });
    if let Some(c) = page.next_cursor {
        body["next_cursor"] = json!(c);
    }
    Ok(ErrorEnvelope::success(body))
}
