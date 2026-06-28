//! Core `validate` operation — workspace-level broken-link diagnostics.
//!
//! Single source of truth shared by the LSP (`compute_diagnostics`) and the MCP server.
//! Iterates each object's parser-extracted `__references` (NOT raw field text) and resolves
//! every target via the shared [`crate::core::resolve`] logic — exactly the resolution chain
//! the LSP uses (by `__id`, by `__local_id` + namespace, field-ref, hierarchical id).
//!
//! Supports scoping to a single file path.

use serde_json::{json, Value};

use crate::core::error::{ErrorCode, ErrorEnvelope};
use crate::core::fields::QmdcObject;
use crate::core::resolve::{extract_id_from_target, ObjectIndex, Resolution};
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
    let diagnostics: Vec<Value> = collect_reference_issues(objects, &iter_objects)
        .into_iter()
        .map(|i| {
            json!({
                "file": i.file,
                "line": i.line,
                "code": i.code,
                "message": i.message,
                "severity": i.severity,
            })
        })
        .collect();

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

/// One broken/ambiguous reference finding, with its source position.
///
/// The shared currency between the CLI/MCP `validate` envelope and the LSP's
/// `compute_diagnostics`: produced once by [`collect_reference_issues`], then
/// mapped to each caller's diagnostic shape.
pub struct RefIssue {
    pub file: String,
    pub line: i64,
    /// 0-based start column of the `[[...]]` reference span.
    pub start_col: u32,
    /// 0-based end column (exclusive).
    pub end_col: u32,
    pub ref_id: String,
    /// `"QMDC001"` (broken link) or `"QMDC002"` (ambiguous).
    pub code: &'static str,
    /// [`SEVERITY_ERROR`] or [`SEVERITY_WARNING`].
    pub severity: &'static str,
    pub message: String,
}

/// THE canonical broken/ambiguous reference scan — the single resolution rule
/// shared by the CLI/MCP `validate` and the LSP `compute_diagnostics`, so the two
/// can never drift again.
///
/// Builds the resolution index from `index_objects`, then for every object in
/// `iter_objects` resolves each of its parser `__references` using **that object's
/// own namespace** (`obj.namespace()`) — never a re-derived per-document namespace.
/// Reference positions (line + columns) come straight from the parser's
/// `__references`.
///
/// Callers split the two sets as needed: the CLI passes the whole workspace as the
/// index and the (optionally file-scoped) subset as the iter set; the LSP passes
/// `workspace ∪ open-doc` as the index and the open doc's (namespace-backfilled)
/// objects as the iter set, so a single open file is diagnosed against the whole
/// graph with the correct namespace.
pub fn collect_reference_issues(index_objects: &[Value], iter_objects: &[Value]) -> Vec<RefIssue> {
    let obj_index = ObjectIndex::build(index_objects);
    let mut issues = Vec::new();

    for obj in iter_objects {
        let obj_file = obj.file();
        let from_namespace = obj.namespace();

        for r in obj.references() {
            let target = match r.get("target").and_then(|v| v.as_str()) {
                Some(t) => t,
                None => continue,
            };
            let ref_id = extract_id_from_target(target);
            let line = r.get("line").and_then(|v| v.as_i64()).unwrap_or(0);
            let start_col = r.get("start_col").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
            let end_col = r.get("end_col").and_then(|v| v.as_u64()).unwrap_or(0) as u32;

            match obj_index.resolve(target, from_namespace) {
                Resolution::Resolved => {}
                Resolution::NotFound { hint } => issues.push(RefIssue {
                    file: obj_file.to_string(),
                    line,
                    start_col,
                    end_col,
                    message: format!("Object '{}' not found{}", ref_id, hint),
                    ref_id,
                    code: "QMDC001",
                    severity: SEVERITY_ERROR,
                }),
                Resolution::Ambiguous => issues.push(RefIssue {
                    file: obj_file.to_string(),
                    line,
                    start_col,
                    end_col,
                    message: format!(
                        "Ambiguous reference '{}' - multiple objects match by __local_id",
                        ref_id
                    ),
                    ref_id,
                    code: "QMDC002",
                    severity: SEVERITY_WARNING,
                }),
            }
        }
    }

    issues
}
