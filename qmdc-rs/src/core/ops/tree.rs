//! Core `tree` operation — workspace tree as a keyset-paginated node stream.
//!
//! A tree view that loads N nodes at a time and scrolls indefinitely. We emit the workspace
//! objects as a flat node stream, keyset-paginated by `__global_id` (the same stable cursor key
//! used by search/dump/find_references). Each node carries `level` + `parent` + `namespace` so
//! the client reconstructs the hierarchy incrementally regardless of stream order. Reliable:
//! the cursor resumes strictly after the last returned node (robust to edits between pages),
//! unlike offset/skip-take.
//!
//! Internal artifacts (`__TextBlock`, `__Document`) are excluded; `__Workspace`/`__Namespace`
//! remain as the tree's root nodes.

use serde_json::{json, Value};

use crate::core::envelope::cursor_page;
use crate::core::error::ErrorEnvelope;
use crate::core::fields::QmdcObject;
use crate::core::resolved_index::ResolvedIndex;

/// Kinds that are internal parser artifacts, not user-facing tree nodes.
const HIDDEN_KINDS: &[&str] = &["__TextBlock", "__Document"];

/// Produce a keyset-paginated node stream of the workspace.
///
/// # Arguments
/// * `index` — the resolved workspace index.
/// * `namespace_filter` — optional: only nodes in this namespace.
/// * `limit` — max nodes per page.
/// * `cursor` — opaque keyset cursor (`__global_id`); the stream resumes after it.
///
/// # Returns
/// - `Ok(Value)` — `{ success, nodes, truncated, [next_cursor] }`, where each node is
///   `{ id, global_id, kind, label, namespace, file, line, level, [parent] }`.
pub fn tree(
    index: &ResolvedIndex,
    namespace_filter: Option<&str>,
    limit: usize,
    cursor: Option<&str>,
) -> Result<Value, Value> {
    // Build (global_id, node) pairs — global_id is the stable, unique, totally-ordered cursor key.
    let mut keyed: Vec<(String, Value)> = Vec::new();

    for obj in index.objects() {
        let kind = obj.kind();
        if HIDDEN_KINDS.contains(&kind) {
            continue;
        }
        let ns = obj.namespace_id();
        if let Some(filter) = namespace_filter {
            if !ns.eq_ignore_ascii_case(filter) {
                continue;
            }
        }

        let mut node = json!({
            "id": obj.id(),
            "global_id": obj.global_id(),
            "kind": kind,
            "label": obj.label(),
            "namespace": ns,
            "file": obj.file(),
            "line": obj.line(),
            "level": obj.level(),
        });
        if let Some(parent) = obj.str_opt("__parent").filter(|p| !p.is_empty()) {
            node["parent"] = json!(parent);
        }

        keyed.push((obj.global_id(), node));
    }

    let page = cursor_page(keyed, limit, cursor);
    let mut body = json!({
        "nodes": page.items,
        "total": page.total,
        "truncated": page.truncated,
    });
    if let Some(c) = page.next_cursor {
        body["next_cursor"] = json!(c);
    }
    Ok(ErrorEnvelope::success(body))
}
