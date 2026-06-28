//! Core `find_references` operation — find all objects that reference a given object.
//!
//! Iterates each object's parser-extracted `__references` and resolves every target via
//! the shared [`crate::core::resolve`] logic. An object is a referrer when one of its
//! references resolves to the same target object — identity-based matching that handles
//! `ns:id`, hierarchical ids, and `__local_id` (not naive string equality). The LSP
//! `references` handler resolves through the same `core::resolve::ObjectIndex`, so both
//! transports agree on referrer membership.

use serde_json::{json, Value};

use crate::core::envelope::cursor_page;
use crate::core::error::{ErrorCode, ErrorEnvelope};
use crate::core::fields::QmdcObject;
use crate::core::resolve::ObjectIndex;
use crate::core::resolved_index::ResolvedIndex;

/// Find all objects that reference the given object id.
///
/// # Returns
/// - `Ok(Value)` — success envelope with `{ references: [{ file, line, id, kind }], count }`.
/// - `Err(Value)` — `invalid-argument` if id is empty; `not-found` if the target doesn't exist.
pub fn find_references(
    index: &ResolvedIndex,
    id: &str,
    limit: usize,
    cursor: Option<&str>,
) -> Result<Value, Value> {
    let trimmed = id.trim();
    let target_id = trimmed.strip_prefix('#').unwrap_or(trimmed);

    if target_id.is_empty() {
        return Err(ErrorEnvelope::error(
            ErrorCode::InvalidArgument,
            "id must not be empty",
        ));
    }

    let obj_index = ObjectIndex::build(index.objects());

    // Resolve the requested id to a concrete target object so we compare identities,
    // not strings. (Falls back to treating `target_id` as the canonical id if the
    // object isn't directly present — e.g. requesting references to a bare id.)
    let target_obj = obj_index.resolve_object(target_id, "");
    let canonical_id = target_obj
        .and_then(|o| o.get("__id").and_then(|v| v.as_str()))
        .unwrap_or(target_id)
        .to_string();

    let mut refs: Vec<(String, Value)> = Vec::new();

    for obj in index.objects() {
        let obj_id = obj.id();
        let obj_file = obj.file();
        let obj_line = obj.line();
        let obj_kind = obj.kind();
        let from_namespace = obj.namespace();

        let reference_arr = obj.references();

        // Does any reference on this object resolve to the target object?
        let references_target = reference_arr.iter().any(|r| {
            let target = match r.get("target").and_then(|v| v.as_str()) {
                Some(t) => t,
                None => return false,
            };
            match obj_index.resolve_object(target, from_namespace) {
                Some(resolved) => {
                    resolved.get("__id").and_then(|v| v.as_str()) == Some(canonical_id.as_str())
                }
                None => false,
            }
        });

        if references_target {
            let item = json!({
                "file": obj_file,
                "line": obj_line,
                "id": obj_id,
                "kind": obj_kind,
            });
            refs.push((obj.global_id(), item));
        }
    }

    // NFR-4: keyset-paginate the referrer list by __global_id. `count` is the true total.
    let total_count = refs.len();
    let page = cursor_page(refs, limit, cursor);
    let mut body = json!({
        "references": page.items,
        "count": total_count,
        "total": page.total,
        "truncated": page.truncated,
    });
    if let Some(c) = page.next_cursor {
        body["next_cursor"] = json!(c);
    }
    Ok(ErrorEnvelope::success(body))
}
