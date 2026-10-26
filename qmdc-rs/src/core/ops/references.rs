//! Core `find_references` operation — find all objects that reference a given object.
//!
//! Resolves the requested id to its object through the shared [`crate::core::resolve`]
//! logic, then reads the referrers from the index's resolved `edges` table (QMD-87): the
//! same edges `qmdc query`, `traverse` and `find_path` see, resolved by the same resolver
//! `workspace validate` agrees with. A reference in a field, in body text, a field path
//! into the target, and a parent's child field all make the parent a referrer.
//!
//! The LSP `references` handler is a different question — where in the source the target
//! is written — and still answers it from the parser's reference spans.

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
    // not strings. An id that names no object has no referrers.
    let target_global_id = obj_index
        .resolve_object(target_id, "")
        .map(|o| o.global_id());

    // QMD-87 (GitHub #18): referrers are the sources of the index's resolved edges — the
    // same table `qmdc query`, `traverse` and `find_path` read — so a reference in a field,
    // in body text, or a parent's child field all count, and every tool agrees.
    let referrer_ids = match &target_global_id {
        Some(gid) => index.db.referrer_global_ids(gid).map_err(|e| {
            ErrorEnvelope::error(
                ErrorCode::InternalError,
                format!("reading resolved edges failed: {}", e),
            )
        })?,
        None => Default::default(),
    };

    let mut refs: Vec<(String, Value)> = Vec::new();

    for obj in index.objects() {
        let global_id = obj.global_id();
        if referrer_ids.contains(&global_id) {
            let item = json!({
                "file": obj.file(),
                "line": obj.line(),
                "id": obj.id(),
                "kind": obj.kind(),
            });
            refs.push((global_id, item));
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
