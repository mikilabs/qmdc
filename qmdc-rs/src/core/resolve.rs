//! Shared reference-resolution logic — the single source of truth used by BOTH
//! the LSP handlers and the MCP server.
//!
//! Ported from the real `Backend` resolution helpers (`extract_id_from_target`,
//! `find_object_in_workspace_with_namespace`, field-ref resolution) that previously
//! lived inline in `src/lsp/server.rs`. Operates on parsed objects (`&[Value]`) which
//! carry `__id`, `__local_id`, `__namespace`, and `__references` — the same data the
//! LSP `Document`/`WorkspaceInfo` and the MCP `ResolvedIndex` both hold.

use std::collections::HashMap;

use serde_json::Value;

/// Extract the actual ID from a reference target.
///
/// e.g. `"#user"` -> `"user"`, `"User.admin"` -> `"admin"`, `"auth.user"` -> `"user"`,
/// `"file#id"` -> `"id"`, `"ns:Kind:id"` -> `"id"`.
///
/// This is a verbatim port of `Backend::extract_id_from_target`.
pub fn extract_id_from_target(target: &str) -> String {
    let s = target.strip_prefix('#').unwrap_or(target);

    // Handle file#id (cross-file reference)
    if s.contains('#') {
        return s.rsplit('#').next().unwrap_or(s).to_string();
    }

    // Handle Kind:id or Kind.id or ns.id
    if s.contains(':') {
        s.rsplit(':').next().unwrap_or(s).to_string()
    } else if s.contains('.') {
        s.rsplit('.').next().unwrap_or(s).to_string()
    } else {
        s.to_string()
    }
}

/// Split a dot-path field reference (`obj.field`) into `(obj_prefix, field_part)`.
/// Returns `None` when the target is not a field reference (no dot, or namespaced).
///
/// Verbatim port of `Backend::split_field_ref`.
pub fn split_field_ref(raw_target: &str) -> Option<(&str, &str)> {
    if !raw_target.contains('.') || raw_target.contains(':') {
        return None;
    }
    let last_dot = raw_target.rfind('.')?;
    let obj_prefix = &raw_target[..last_dot];
    let field_part = &raw_target[last_dot + 1..];
    if obj_prefix.is_empty() || field_part.is_empty() {
        return None;
    }
    Some((obj_prefix, field_part))
}

/// Parse an explicit namespace out of a reference target, if present.
///
/// Format `ns:id` or `ns:Kind:id` — the first segment is treated as a namespace
/// only when it does NOT start with an uppercase letter (uppercase ⇒ Kind).
/// Mirrors the namespace-parse block in `goto_definition` / `compute_diagnostics`.
pub fn parse_ref_namespace(target: &str) -> Option<String> {
    let s = target.strip_prefix('#').unwrap_or(target);
    if !s.contains(':') {
        return None;
    }
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() < 2 {
        return None;
    }
    let first = parts[0];
    let starts_upper = first
        .chars()
        .next()
        .map(|c| c.is_uppercase())
        .unwrap_or(false);
    if starts_upper {
        None
    } else {
        Some(first.to_string())
    }
}

/// An index over parsed objects providing the same lookup power the LSP `WorkspaceInfo`
/// had: by `__id`, by `__local_id`, and direct access for field checks.
pub struct ObjectIndex<'a> {
    by_id: HashMap<&'a str, Vec<&'a Value>>,
    by_local_id: HashMap<&'a str, Vec<&'a Value>>,
}

impl<'a> ObjectIndex<'a> {
    /// Build the lookup index from a flat object slice.
    pub fn build(objects: &'a [Value]) -> Self {
        let mut by_id: HashMap<&str, Vec<&Value>> = HashMap::new();
        let mut by_local_id: HashMap<&str, Vec<&Value>> = HashMap::new();

        for obj in objects {
            if let Some(id) = obj.get("__id").and_then(|v| v.as_str()) {
                by_id.entry(id).or_default().push(obj);
            }
            if let Some(lid) = obj.get("__local_id").and_then(|v| v.as_str()) {
                by_local_id.entry(lid).or_default().push(obj);
            }
        }

        Self { by_id, by_local_id }
    }

    /// Does an object with this exact `__id` exist?
    pub fn contains_id(&self, id: &str) -> bool {
        self.by_id.contains_key(id)
    }

    /// Look up the first object with this exact `__id`.
    pub fn get_by_id(&self, id: &str) -> Option<&'a Value> {
        self.by_id.get(id).and_then(|v| v.first().copied())
    }

    /// Resolve a reference `target` (from an object in namespace `from_namespace`) to the
    /// actual target object, applying the same chain as [`Self::resolve`]: exact `__id`,
    /// then `__local_id` filtered by namespace. Returns `None` if unresolved or ambiguous.
    ///
    /// Used by `locate` / `describe` / `find_references` so they handle namespaced
    /// (`ns:id`), hierarchical (`a.b`), and `__local_id` references exactly like the LSP.
    pub fn resolve_object(&self, target: &str, from_namespace: &str) -> Option<&'a Value> {
        let raw_target = target.strip_prefix('#').unwrap_or(target);
        let ref_id = extract_id_from_target(target);
        let ref_namespace = parse_ref_namespace(target);

        // Hierarchical dotted id as a full id.
        if raw_target.contains('.') && !raw_target.contains(':') {
            if let Some(obj) = self.get_by_id(raw_target) {
                return Some(obj);
            }
        }

        // Field reference (obj.field) — resolve to the PREFIX object, so that
        // find_references counts the referrer and rename rewrites the prefix
        // inside field-path refs like `[[#team.config.env]]`. Mirrors the
        // field-ref branch in `resolve` (same precedence: hierarchical first).
        if let Some((obj_prefix, field_part)) = split_field_ref(raw_target) {
            if !field_part.starts_with("__") {
                if let Some(obj) = self.get_by_id(obj_prefix) {
                    if obj.get(field_part).is_some() {
                        return Some(obj);
                    }
                }
            }
        }

        // Exact `__id`.
        if let Some(obj) = self.get_by_id(&ref_id) {
            return Some(obj);
        }

        // `__local_id`, namespace-filtered.
        let target_namespace: Option<&str> =
            ref_namespace.as_deref().or(if from_namespace.is_empty() {
                None
            } else {
                Some(from_namespace)
            });

        if let Some(candidates) = self.by_local_id.get(ref_id.as_str()) {
            let filtered: Vec<&&Value> = candidates
                .iter()
                .filter(|obj| {
                    let obj_ns = obj
                        .get("__namespace")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    match target_namespace {
                        Some(ns) => obj_ns == ns,
                        None => obj_ns.is_empty(),
                    }
                })
                .collect();
            if filtered.len() == 1 {
                return Some(filtered[0]);
            }
        }
        None
    }

    /// Resolve a bare `id` (already extracted from a target) within an optional namespace,
    /// returning the matched object. Mirrors the LSP `find_object_in_workspace_with_namespace`
    /// matching: exact `__id` first, then `__local_id` filtered by namespace (preferring the
    /// given namespace, falling back to the first `__local_id` match).
    pub fn resolve_id(&self, id: &str, namespace: Option<&str>) -> Option<&'a Value> {
        if let Some(objs) = self.by_id.get(id) {
            if let Some(ns) = namespace {
                if let Some(obj) = objs
                    .iter()
                    .find(|o| o.get("__namespace").and_then(|v| v.as_str()).unwrap_or("") == ns)
                {
                    return Some(obj);
                }
            }
            if let Some(obj) = objs.first() {
                return Some(obj);
            }
        }
        // `__local_id` fallback: when a namespace is specified it is required (no first-fallback),
        // matching the LSP `find_object_in_workspace_with_namespace` behaviour.
        let candidates = self.by_local_id.get(id)?;
        if let Some(ns) = namespace {
            return candidates
                .iter()
                .find(|o| o.get("__namespace").and_then(|v| v.as_str()).unwrap_or("") == ns)
                .copied();
        }
        candidates.first().copied()
    }
}

#[cfg(test)]
mod namespaced_localid_tests {
    use super::*;
    use serde_json::json;

    /// Reproduces the reported diagnostics scenario: `[[#lsp:completion]]` referencing a
    /// hierarchical child object `lsp_completion.completion` (local id `completion`, namespace
    /// `lsp`). This must RESOLVE, not produce a "not found / did you mean" diagnostic.
    #[test]
    fn namespaced_ref_to_hierarchical_child_resolves() {
        let objects = vec![
            json!({ "__id": "lsp", "__kind": "__Namespace" }),
            json!({ "__id": "lsp_completion", "__local_id": "lsp_completion", "__namespace": "lsp", "__kind": "Category" }),
            json!({ "__id": "lsp_completion.completion", "__local_id": "completion", "__namespace": "lsp", "__parent": "lsp_completion", "__kind": "LSPFeature" }),
        ];
        let idx = ObjectIndex::build(&objects);
        // from_namespace = "" — the readme file that holds the ref only has the __Namespace decl.
        assert!(idx.resolve_object("#lsp:completion", "").is_some());
    }
}
