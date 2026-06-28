//! Core `describe` operation — return the full object card or a specific field value.
//!
//! For a plain ref like `"users"`, returns the whole object card (all fields including
//! system fields like __id, __kind, __file, etc.).
//! For a dot-path ref like `"users.name"`, returns just that field's value and inferred type.

use serde_json::{json, Value};

use crate::core::error::{ErrorCode, ErrorEnvelope};
use crate::core::fields::QmdcObject;
use crate::core::resolve::ObjectIndex;
use crate::core::resolved_index::ResolvedIndex;

/// Describe an object or a field on an object.
///
/// # Arguments
/// * `index` — the resolved workspace index.
/// * `ref_str` — either a plain object ID (e.g. `"users"`) or a dot-path (e.g. `"users.name"`).
///   Leading `#` is stripped if present.
///
/// # Returns
/// - `Ok(Value)` — success envelope with the object card or field descriptor.
/// - `Err(Value)` — `invalid-argument` if ref is empty; `not-found` if no match.
pub fn describe(index: &ResolvedIndex, ref_str: &str) -> Result<Value, Value> {
    let normalized = normalize_ref(ref_str);

    if normalized.is_empty() {
        return Err(ErrorEnvelope::error(
            ErrorCode::InvalidArgument,
            "ref must not be empty",
        ));
    }

    let obj_index = ObjectIndex::build(index.objects());

    // First, try resolving the whole ref as an object (handles exact id, namespaced
    // `ns:id`, hierarchical `a.b.c`, and `__local_id`). This is the object card.
    if let Some(obj) = obj_index.resolve_object(normalized, "") {
        return Ok(object_card(obj));
    }

    // Otherwise, if it's a field dot-path (`obj.field`), describe the field.
    if let Some((obj_id, field_name)) = split_dot_path(normalized) {
        if let Some(obj) = obj_index.resolve_object(obj_id, "") {
            return describe_field(obj, obj_id, field_name);
        }
    }

    Err(ErrorEnvelope::error(
        ErrorCode::NotFound,
        format!("no object matching '{}' found in workspace", normalized),
    ))
}

/// Build the full object card for a resolved object.
fn object_card(obj: &Value) -> Value {
    let obj_id = obj.id();
    let file = obj.file();
    let line = obj.line();
    let kind = obj.kind();
    let namespace = obj.namespace();
    let workspace = obj
        .get("__workspace")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let parent = obj.get("__parent").and_then(|v| v.as_str()).unwrap_or("");

    let mut fields = json!({});
    if let Some(map) = obj.as_object() {
        for (k, v) in map {
            if !k.starts_with("__") {
                fields[k] = v.clone();
            }
        }
    }

    ErrorEnvelope::success(json!({
        "id": obj_id,
        "kind": kind,
        "file": file,
        "line": line,
        "namespace": namespace,
        "workspace": workspace,
        "parent": parent,
        "fields": fields,
    }))
}

/// Return the value and inferred type of a specific field on a resolved object.
fn describe_field(obj: &Value, obj_id: &str, field_name: &str) -> Result<Value, Value> {
    if field_name.starts_with("__") {
        return Err(ErrorEnvelope::error(
            ErrorCode::NotFound,
            format!(
                "field '{}' is a system field and cannot be accessed via dot-path",
                field_name
            ),
        ));
    }
    if let Some(value) = obj.get(field_name) {
        let value_type = infer_type(value);
        return Ok(ErrorEnvelope::success(json!({
            "object_id": obj_id,
            "field": field_name,
            "value": value,
            "type": value_type,
        })));
    }
    Err(ErrorEnvelope::error(
        ErrorCode::NotFound,
        format!("field '{}' not found on object '{}'", field_name, obj_id),
    ))
}

/// Infer a simple type string from a JSON value.
fn infer_type(value: &Value) -> &'static str {
    match value {
        Value::String(s) => {
            if s.contains("[[#") {
                "reference"
            } else {
                "string"
            }
        }
        Value::Number(_) => "number",
        Value::Bool(_) => "boolean",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
        Value::Null => "null",
    }
}

/// Split a dot-path into (object_id, field_name).
/// Returns None if no dot is present or if parts are empty.
fn split_dot_path(s: &str) -> Option<(&str, &str)> {
    // Don't treat strings with colons as dot-paths (namespaced IDs)
    if s.contains(':') {
        return None;
    }
    let last_dot = s.rfind('.')?;
    let obj_id = &s[..last_dot];
    let field_name = &s[last_dot + 1..];
    if obj_id.is_empty() || field_name.is_empty() {
        return None;
    }
    Some((obj_id, field_name))
}

/// Strip leading `#` from a ref string, trim whitespace.
fn normalize_ref(ref_str: &str) -> &str {
    let trimmed = ref_str.trim();
    trimmed.strip_prefix('#').unwrap_or(trimmed)
}
