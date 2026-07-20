//! Core `validate` operation — workspace-level diagnostics for the MCP surface.
//!
//! Reference diagnostics (broken_link / ambiguous_reference / ambiguous_field_reference)
//! come from the single shared engine [`crate::core::reference_scan`] — the same algorithm
//! the CLI and LSP use, so the three surfaces cannot drift. Duplicate ids
//! ([`collect_duplicate_issues`]) and the structural errors carried on the index's
//! `WorkspaceResult` are merged in so MCP reports the same set the CLI does.
//!
//! Supports scoping to a single file path.

use serde_json::{json, Value};

use crate::core::error::{ErrorCode, ErrorEnvelope};
use crate::core::fields::QmdcObject;
use crate::core::resolved_index::ResolvedIndex;

pub const SEVERITY_ERROR: &str = "error";
pub const SEVERITY_WARNING: &str = "warning";

/// Validate the workspace index for broken references.
///
/// # Arguments
/// * `index` — the resolved workspace index.
/// * `path` — optional relative file path to scope validation. When `None`, validates all files.
///
/// # Returns
/// - `Ok(Value)` — success envelope with `{ diagnostics: [...], count }`.
/// - `Err(Value)` — `out-of-root` if `path` escapes the workspace root.
pub fn validate(index: &ResolvedIndex, path: Option<&str>) -> Result<Value, Value> {
    // Path-scope containment check (INV-1) — fail-closed: a `file` scope that cannot be
    // canonicalized within the workspace root is denied rather than silently ignored.
    if let Some(p) = path {
        let p = p.trim();
        if !p.is_empty() {
            let canon_root = index.root.canonicalize().map_err(|_| {
                ErrorEnvelope::error(ErrorCode::OutOfRoot, "workspace root is not accessible")
            })?;
            let canon_target = index.root.join(p).canonicalize().map_err(|_| {
                ErrorEnvelope::error(
                    ErrorCode::OutOfRoot,
                    format!("path '{}' is not accessible within the workspace root", p),
                )
            })?;
            if !canon_target.starts_with(&canon_root) {
                return Err(ErrorEnvelope::error(
                    ErrorCode::OutOfRoot,
                    format!("path '{}' escapes the workspace root", p),
                ));
            }
        }
    }

    let objects = index.objects();
    // CLI/MCP scope: when a file path is given, only that file's objects are
    // scanned; the resolution index always spans the whole workspace.
    let iter_objects: Vec<Value> = match path {
        Some(p) if !p.trim().is_empty() => {
            let p = p.trim();
            objects.iter().filter(|o| o.file() == p).cloned().collect()
        }
        _ => objects.to_vec(),
    };
    let mut diagnostics: Vec<Value> =
        crate::core::reference_scan::reference_scan(objects, &iter_objects, None)
            .into_iter()
            .map(|f| {
                json!({
                    "file": f.file,
                    "line": f.line,
                    "code": f.kind.code(),
                    "message": f.message,
                    "severity": SEVERITY_ERROR,
                })
            })
            .collect();

    // Structural diagnostics (QMD-68): namespace-scoped duplicate ids (QMDC003).
    // The resolution index always spans the whole workspace; when a file scope is
    // given we report only the duplicates whose location is in that file.
    let scope_file: Option<&str> = match path {
        Some(p) if !p.trim().is_empty() => Some(p.trim()),
        _ => None,
    };
    for dup in collect_duplicate_issues(objects) {
        if let Some(f) = scope_file {
            if dup.file != f {
                continue;
            }
        }
        diagnostics.push(json!({
            "file": dup.file,
            "line": dup.line,
            "code": "QMDC003",
            "message": dup.message,
            "severity": SEVERITY_ERROR,
        }));
    }

    // Structural diagnostics (QMD-68 / B3): surface the remaining workspace-level and
    // parser-structural errors that the shared reference/duplicate scans do NOT cover —
    // broken_parent, nested_workspace, workspace_in_wrong_file, and every parser
    // `__ParsingError` kind (dangling_field, mixed_field_keys, multiple_definitions,
    // structured_in_textblock, ...). These are already computed correctly on the index's
    // `WorkspaceResult`; we reuse them so MCP matches the CLI. Reference types and
    // duplicate_id are excluded here because they are produced by the shared scans above.
    for err in &index.workspace.errors {
        match err.error_type.as_str() {
            "broken_link" | "ambiguous_reference" | "ambiguous_field_reference" => continue,
            // duplicate_id: the validator-produced findings carry `candidates` and were
            // already emitted above via `collect_duplicate_issues`; skip only those to
            // avoid double-reporting. Parser-owned same-file/same-kind duplicates are
            // emitted by the parser as `__ParsingError` with `candidates == None` and are
            // NOT covered by `collect_duplicate_issues`, so let them through — otherwise
            // MCP silently misses them (QMD-68 CR #1).
            "duplicate_id" if err.candidates.is_some() => continue,
            _ => {}
        }
        if let Some(f) = scope_file {
            if err.file.as_deref() != Some(f) {
                continue;
            }
        }
        let code = match err.error_type.as_str() {
            "workspace_in_wrong_file" => "QMDC004",
            "duplicate_id" => "QMDC003",
            other => other,
        };
        diagnostics.push(json!({
            "file": err.file,
            "line": err.line,
            "code": code,
            "message": err.message,
            "severity": err.severity,
        }));
    }

    // NFR-4: bound the diagnostics list, surfacing truncation alongside the domain key.
    let (diagnostics, truncated, remaining) =
        crate::core::envelope::bound_list(diagnostics, crate::core::envelope::DEFAULT_LIMIT);
    let mut body = json!({
        "diagnostics": diagnostics,
        "count": diagnostics.len(),
        "truncated": truncated,
    });
    if truncated {
        body["remaining"] = json!(remaining);
    }
    Ok(ErrorEnvelope::success(body))
}

/// One duplicate-id finding (QMDC003), namespace-scoped per QMD-67.
///
/// The single source of truth for workspace-level duplicate detection, shared by the
/// CLI validator, the MCP `validate` op, and the LSP `compute_diagnostics`. Grouping is
/// by `(namespace, full __id)`; `same_file` distinguishes a cross-file duplicate from a
/// same-file/different-kind one so callers that already do their own same-file check
/// (the LSP) can take only the cross-file subset and avoid double-reporting.
pub struct DuplicateIssue {
    pub file: String,
    pub line: i64,
    pub id: String,
    pub message: String,
    /// `true` when both occurrences live in the same file (different kinds); `false`
    /// when the duplicate spans multiple files.
    pub same_file: bool,
    pub candidates: Vec<String>,
}

/// Detect duplicate object ids, scoped by `(namespace, full __id)` (QMD-67).
///
/// THE canonical duplicate-id scan. Mirrors the historical CLI logic exactly:
/// - duplicates across DIFFERENT files → one issue per extra occurrence
///   ("found in multiple files");
/// - same file, DIFFERENT kinds → "with different kinds: A and B";
/// - same file, SAME kind → skipped here (the parser already emits a
///   `__ParsingError{type=duplicate_id}` for that case, and re-detecting would also
///   false-positive on object-array skeleton children).
///
/// System/auto-generated kinds (`__Document`, `__TextBlock`, `__ParsingError`) are
/// excluded. The `__namespace` is read via [`QmdcObject::namespace_id`] so a
/// `[[#id]]`-wrapped value and a plain id compare equal.
pub fn collect_duplicate_issues(objects: &[Value]) -> Vec<DuplicateIssue> {
    use std::collections::HashMap;

    // (file, kind, line) — one occurrence of an id within a (namespace, id) group.
    type DupOccurrence = (String, String, i64);
    // (namespace, id) -> occurrences
    let mut groups: HashMap<(String, String), Vec<DupOccurrence>> = HashMap::new();
    for obj in objects {
        if crate::core::fields::is_system_kind(obj.kind()) {
            continue;
        }
        let id = obj.id();
        let file = obj.file();
        if id.is_empty() || file.is_empty() {
            continue;
        }
        let ns = obj.namespace_id().to_string();
        groups.entry((ns, id.to_string())).or_default().push((
            file.to_string(),
            obj.kind().to_string(),
            obj.line(),
        ));
    }

    // Deterministic order: sort group keys so emitted issues are stable across runs.
    let mut keys: Vec<&(String, String)> = groups.keys().collect();
    keys.sort();

    let mut issues = Vec::new();
    for key in keys {
        let locations = &groups[key];
        if locations.len() < 2 {
            continue;
        }
        let id = &key.1;
        let files: std::collections::HashSet<&String> =
            locations.iter().map(|(f, _, _)| f).collect();
        if files.len() > 1 {
            let candidates: Vec<String> = locations
                .iter()
                .map(|(f, _, l)| format!("{}:{}", f, l))
                .collect();
            for (file, _kind, line) in locations.iter().skip(1) {
                issues.push(DuplicateIssue {
                    file: file.clone(),
                    line: *line,
                    id: id.clone(),
                    message: format!("Duplicate ID '{}' found in multiple files", id),
                    same_file: false,
                    candidates: candidates.clone(),
                });
            }
        } else {
            let kinds: std::collections::HashSet<&String> =
                locations.iter().map(|(_, k, _)| k).collect();
            if kinds.len() > 1 {
                let first_kind = &locations[0].1;
                let candidates: Vec<String> = locations
                    .iter()
                    .map(|(f, k, l)| format!("{}:{}:{}", f, k, l))
                    .collect();
                for (file, kind, line) in locations.iter().skip(1) {
                    issues.push(DuplicateIssue {
                        file: file.clone(),
                        line: *line,
                        id: id.clone(),
                        message: format!(
                            "Duplicate ID '{}' with different kinds: {} and {}",
                            id, first_kind, kind
                        ),
                        same_file: true,
                        candidates: candidates.clone(),
                    });
                }
            }
            // same file, same kind — parser owns it (emits __ParsingError).
        }
    }
    issues
}

/// The set of `(namespace, full __id)` keys that are duplicated across MORE THAN ONE
/// file — i.e. genuine cross-file duplicates (QMDC003).
///
/// Used by the LSP to flag EACH occurrence in the currently-open file whose key is in
/// this set, regardless of which occurrence the batch [`collect_duplicate_issues`] would
/// pick as the group anchor. This guarantees the open file is always flagged when it
/// participates in a cross-file duplicate. System/auto-generated kinds are excluded, and
/// the namespace is read via [`QmdcObject::namespace_id`].
pub fn cross_file_duplicate_keys(objects: &[Value]) -> std::collections::HashSet<(String, String)> {
    use std::collections::{HashMap, HashSet};

    // (namespace, id) -> set of files it appears in.
    let mut files_by_key: HashMap<(String, String), HashSet<String>> = HashMap::new();
    for obj in objects {
        if crate::core::fields::is_system_kind(obj.kind()) {
            continue;
        }
        let id = obj.id();
        let file = obj.file();
        if id.is_empty() || file.is_empty() {
            continue;
        }
        files_by_key
            .entry((obj.namespace_id().to_string(), id.to_string()))
            .or_default()
            .insert(file.to_string());
    }
    files_by_key
        .into_iter()
        .filter(|(_, files)| files.len() > 1)
        .map(|(key, _)| key)
        .collect()
}

#[cfg(test)]
mod duplicate_tests {
    //! Characterization tests pinning the shared duplicate-detection semantics
    //! (QMD-68 safety net for the upcoming CLI→core consolidation). These lock in
    //! today's behavior so the dedup refactor cannot silently change it.
    use super::{collect_duplicate_issues, cross_file_duplicate_keys};
    use serde_json::json;

    fn obj(id: &str, kind: &str, file: &str, ns: &str, line: i64) -> serde_json::Value {
        let mut o = json!({"__id": id, "__kind": kind, "__file": file, "__line": line});
        if !ns.is_empty() {
            o["__namespace"] = json!(ns);
        }
        o
    }

    #[test]
    fn cross_file_same_namespace_is_duplicate() {
        let objs = vec![
            obj("foo", "Thing", "a.qmd.md", "", 1),
            obj("foo", "Thing", "b.qmd.md", "", 1),
        ];
        let issues = collect_duplicate_issues(&objs);
        assert_eq!(issues.len(), 1);
        assert!(!issues[0].same_file);
        assert_eq!(issues[0].file, "b.qmd.md"); // reported on the 2nd occurrence
        assert!(issues[0].message.contains("found in multiple files"));

        let keys = cross_file_duplicate_keys(&objs);
        assert!(keys.contains(&(String::new(), "foo".to_string())));
    }

    #[test]
    fn same_id_different_namespaces_is_not_duplicate() {
        // QMD-67 semantics: namespace scopes identity.
        let objs = vec![
            obj("foo", "Thing", "r1/readme.qmd.md", "r1", 1),
            obj("foo", "Thing", "r2/readme.qmd.md", "r2", 1),
        ];
        assert!(collect_duplicate_issues(&objs).is_empty());
        assert!(cross_file_duplicate_keys(&objs).is_empty());
    }

    #[test]
    fn same_file_different_kind_is_duplicate_same_file() {
        let objs = vec![
            obj("foo", "Thing", "a.qmd.md", "", 1),
            obj("foo", "Other", "a.qmd.md", "", 5),
        ];
        let issues = collect_duplicate_issues(&objs);
        assert_eq!(issues.len(), 1);
        assert!(issues[0].same_file);
        assert!(issues[0].message.contains("different kinds"));
        // A same-file duplicate is NOT a cross-file key.
        assert!(cross_file_duplicate_keys(&objs).is_empty());
    }

    #[test]
    fn same_file_same_kind_is_owned_by_parser() {
        // The parser emits __ParsingError for this; the shared scan must NOT re-report.
        let objs = vec![
            obj("foo", "Thing", "a.qmd.md", "", 1),
            obj("foo", "Thing", "a.qmd.md", "", 5),
        ];
        assert!(collect_duplicate_issues(&objs).is_empty());
    }

    #[test]
    fn system_and_parsing_error_kinds_excluded() {
        let objs = vec![
            obj("doc", "__Document", "a.qmd.md", "", 1),
            obj("doc", "__Document", "b.qmd.md", "", 1),
            obj("err", "__ParsingError", "a.qmd.md", "", 1),
            obj("err", "__ParsingError", "b.qmd.md", "", 1),
        ];
        assert!(collect_duplicate_issues(&objs).is_empty());
        assert!(cross_file_duplicate_keys(&objs).is_empty());
    }

    #[test]
    fn three_files_report_each_extra_occurrence() {
        let objs = vec![
            obj("foo", "Thing", "a.qmd.md", "", 1),
            obj("foo", "Thing", "b.qmd.md", "", 1),
            obj("foo", "Thing", "c.qmd.md", "", 1),
        ];
        // First is the anchor; the other two are reported.
        assert_eq!(collect_duplicate_issues(&objs).len(), 2);
        assert!(cross_file_duplicate_keys(&objs).contains(&(String::new(), "foo".to_string())));
    }
}
