//! Shared accessors for QMD.md object system fields.
//!
//! Every Core op was repeating `obj.get("__id").and_then(|v| v.as_str()).unwrap_or("")`
//! (and the `__line`/`__kind`/`__file`/… variants). This trait centralises those accessors
//! so the system-field names and default-handling live in exactly one place.

use serde_json::Value;

/// Convenience accessors for QMD.md object system fields on a `serde_json::Value`.
pub trait QmdcObject {
    /// String system field, or `""` if absent/non-string.
    fn str_field(&self, key: &str) -> &str;
    /// Integer system field, or `0` if absent/non-integer.
    fn i64_field(&self, key: &str) -> i64;
    /// String system field as `Option` (for `?`-style propagation).
    fn str_opt(&self, key: &str) -> Option<&str>;

    fn id(&self) -> &str {
        self.str_field("__id")
    }
    fn kind(&self) -> &str {
        self.str_field("__kind")
    }
    fn label(&self) -> &str {
        self.str_field("__label")
    }
    fn file(&self) -> &str {
        self.str_field("__file")
    }
    fn namespace(&self) -> &str {
        self.str_field("__namespace")
    }
    fn workspace(&self) -> &str {
        self.str_field("__workspace")
    }
    /// Namespace id with a `[[#id]]` wrapper stripped — matching the DB's `__namespace`
    /// extraction in `db::upsert_object` exactly (it strips only `[[#...]]`, not bare `[[...]]`),
    /// so `global_id()` mirrors the SQLite `__global_id` column rather than diverging from it.
    fn namespace_id(&self) -> &str {
        let ns = self.namespace();
        ns.strip_prefix("[[#")
            .and_then(|s| s.strip_suffix("]]"))
            .unwrap_or(ns)
    }
    /// Stable, unique, totally-ordered global id: `ws::id` (empty namespace) or `ws:ns:id`.
    /// Mirrors the SQLite `__global_id` generated column; used as the keyset cursor key.
    fn global_id(&self) -> String {
        let ns = self.namespace_id();
        if ns.is_empty() {
            format!("{}::{}", self.workspace(), self.id())
        } else {
            format!("{}:{}:{}", self.workspace(), ns, self.id())
        }
    }
    fn line(&self) -> i64 {
        self.i64_field("__line")
    }
    fn level(&self) -> i64 {
        self.i64_field("__level")
    }
    /// The parser-extracted structured references on this object (`__references`),
    /// or an empty slice if absent. The single accessor for reference iteration used by
    /// validate / find_references / rename_plan.
    fn references(&self) -> &[Value];
}

impl QmdcObject for Value {
    fn str_field(&self, key: &str) -> &str {
        self.get(key).and_then(|v| v.as_str()).unwrap_or("")
    }
    fn i64_field(&self, key: &str) -> i64 {
        self.get(key).and_then(|v| v.as_i64()).unwrap_or(0)
    }
    fn str_opt(&self, key: &str) -> Option<&str> {
        self.get(key).and_then(|v| v.as_str())
    }
    fn references(&self) -> &[Value] {
        self.get("__references")
            .and_then(|v| v.as_array())
            .map(|a| a.as_slice())
            .unwrap_or(&[])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn accessors_read_system_fields() {
        let obj = json!({"__id": "users", "__kind": "Table", "__line": 3, "__label": "Users"});
        assert_eq!(obj.id(), "users");
        assert_eq!(obj.kind(), "Table");
        assert_eq!(obj.label(), "Users");
        assert_eq!(obj.line(), 3);
    }

    #[test]
    fn missing_fields_use_defaults() {
        let obj = json!({});
        assert_eq!(obj.id(), "");
        assert_eq!(obj.line(), 0);
        assert_eq!(obj.str_opt("__id"), None);
    }
}
