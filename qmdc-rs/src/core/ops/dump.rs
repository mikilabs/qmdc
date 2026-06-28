//! Core `dump` operation — debug dump of the resolved index.
//!
//! Returns workspace metadata plus objects and files as a JSON value (data, not logged).
//! Objects are keyset-paginated by `__global_id` (with `truncated` + `next_cursor`) and may be
//! filtered by namespace/kind; the `files` list is independently capped at the same `limit`
//! (`files_truncated`/`files_remaining`) so neither half can produce an unbounded payload (NFR-4).

use serde_json::{json, Value};

use crate::core::envelope::{bound_list, cursor_page};
use crate::core::error::ErrorEnvelope;
use crate::core::fields::QmdcObject;
use crate::core::resolved_index::ResolvedIndex;

/// Dump the resolved index content for inspection, filtered + keyset-paginated.
///
/// # Returns
/// - `Ok(Value)` — `{ root, workspace_id, file_count, object_count, files, objects, truncated,
///   [next_cursor], files_truncated, [files_remaining] }`. `file_count` / `object_count` always
///   report the true totals (after filtering) even when the lists are truncated.
pub fn dump(
    index: &ResolvedIndex,
    namespace_filter: Option<&str>,
    kind_filter: Option<&str>,
    limit: usize,
    cursor: Option<&str>,
) -> Result<Value, Value> {
    let workspace_id = index.workspace.workspace_id.as_deref().unwrap_or("");

    // Filter, then build (sort_key, item) pairs keyed by __global_id.
    let keyed: Vec<(String, Value)> = index
        .objects()
        .iter()
        .filter(|obj| {
            if let Some(ns) = namespace_filter {
                if !obj.namespace_id().eq_ignore_ascii_case(ns) {
                    return false;
                }
            }
            if let Some(k) = kind_filter {
                if !obj.kind().eq_ignore_ascii_case(k) {
                    return false;
                }
            }
            true
        })
        .map(|obj| (obj.global_id(), obj.clone()))
        .collect();

    let total_objects = keyed.len();
    let file_count = index.file_count;

    // Bound files independently at the same limit so neither half is unbounded (NFR-4).
    let (files, files_truncated, files_remaining) =
        bound_list(index.files().iter().map(|f| json!(f)).collect(), limit);

    let page = cursor_page(keyed, limit, cursor);

    let mut body = json!({
        "root": index.root.to_string_lossy(),
        "workspace_id": workspace_id,
        "file_count": file_count,
        "object_count": total_objects,
        "files": files,
        "objects": page.items,
        "total": page.total,
        "truncated": page.truncated,
        "files_truncated": files_truncated,
    });
    if let Some(c) = page.next_cursor {
        body["next_cursor"] = json!(c);
    }
    if files_truncated {
        body["files_remaining"] = json!(files_remaining);
    }
    Ok(ErrorEnvelope::success(body))
}
