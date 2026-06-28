//! Core `locate` operation — find the defining location of an object by ref/id.
//!
//! Returns the defining location (file + line) and minimal identity (id, kind, namespace).
//! Uses the shared [`crate::core::resolve`] logic so namespaced (`ns:id`), hierarchical
//! (`a.b`), and `__local_id` references resolve exactly like the LSP `goto_definition`.

use serde_json::{json, Value};

use crate::core::error::{ErrorCode, ErrorEnvelope};
use crate::core::fields::QmdcObject;
use crate::core::resolve::ObjectIndex;
use crate::core::resolved_index::ResolvedIndex;

/// Locate an object by ref, returning its defining location and minimal identity.
///
/// # Returns
/// - `Ok(Value)` — success envelope with `{ file, line, id, kind, namespace }`.
/// - `Err(Value)` — `invalid-argument` if ref is empty; `not-found` if no match.
pub fn locate(index: &ResolvedIndex, ref_str: &str) -> Result<Value, Value> {
    let trimmed = ref_str.trim();
    if trimmed.is_empty() || trimmed == "#" {
        return Err(ErrorEnvelope::error(
            ErrorCode::InvalidArgument,
            "ref must not be empty",
        ));
    }

    let obj_index = ObjectIndex::build(index.objects());

    // Resolve with no referring-namespace context (top-level lookup).
    if let Some(obj) = obj_index.resolve_object(trimmed, "") {
        return Ok(ErrorEnvelope::success(json!({
            "file": obj.file(),
            "line": obj.line(),
            "id": obj.id(),
            "kind": obj.kind(),
            "namespace": obj.namespace(),
        })));
    }

    Err(ErrorEnvelope::error(
        ErrorCode::NotFound,
        format!("no object matching '{}' found in workspace", trimmed),
    ))
}
