//! Core `rename_plan` operation — computes a diff-only rename plan (INV-3).
//!
//! This operation NEVER writes to disk. It only reads the resolved index and produces a
//! list of proposed text edits. References are taken from the parser's structured
//! `__references` (NOT naive field-text scanning) and resolved via the shared
//! `core::resolve` logic, consistent with `find_references`.
//!
//! Renames **cascade**: renaming `team` rewrites the `team` definition anchor, every direct
//! reference to `team`, and every reference to a descendant (`team.config`, …). Descendant
//! *definitions* need no edit (they are anchored by `__local_id`; the parser recomposes the
//! hierarchical id from the renamed parent).

use serde_json::{json, Value};

use crate::core::error::{ErrorCode, ErrorEnvelope};
use crate::core::fields::QmdcObject;
use crate::core::resolve::ObjectIndex;
use crate::core::resolved_index::ResolvedIndex;

/// A single proposed rename edit — the transport-agnostic unit of a rename plan.
///
/// `line` is 1-based. `old_text`/`new_text` are the full token texts (e.g. the reference's
/// `raw` or the definition anchor); the LSP narrows these to a minimal character range, while
/// MCP/CLI surface them verbatim.
#[derive(Debug, Clone)]
pub struct RenameEdit {
    pub file: String,
    pub line: i64,
    pub old_text: String,
    pub new_text: String,
    pub kind: &'static str,
}

/// Pure, transport-agnostic rename planner — the **single source of truth** for rename shared
/// by Core/MCP, the CLI, and the LSP.
///
/// Given the flat object set (each carrying `__references`), `old_id`, `new_id`, and a
/// `read_line(file, line_1based)` that returns the source line (Core reads from disk; the LSP
/// reads from its in-memory buffer), it returns the cascade of edits identity-resolved against
/// the object set. No I/O of its own — all file access goes through `read_line`.
///
/// # Invariant
/// - **INV-3**: diff-only. Produces edit data; performs no writes.
pub fn plan_rename_edits<F>(
    objects: &[Value],
    old_id: &str,
    new_id: &str,
    read_line: &F,
) -> Result<Vec<RenameEdit>, Value>
where
    F: Fn(&str, i64) -> Option<String>,
{
    validate_new_id(new_id)?;
    if old_id.is_empty() {
        return Err(ErrorEnvelope::error(
            ErrorCode::InvalidArgument,
            "old_id must not be empty",
        ));
    }

    let obj_index = ObjectIndex::build(objects);

    // Resolve old_id to its canonical object so reference matching is identity-based.
    let canonical_id = match obj_index
        .resolve_id(old_id, None)
        .and_then(|o| o.get("__id").and_then(|v| v.as_str()))
    {
        Some(id) => id.to_string(),
        None => {
            return Err(ErrorEnvelope::error(
                ErrorCode::NotFound,
                format!("no object matching '{}' found in workspace", old_id),
            ))
        }
    };

    // Cascade: rewrite a reference when it resolves to the target OR a descendant (hierarchical
    // id under `canonical_id.`). Descendant *definitions* need no edit — they are anchored by
    // `__local_id`; the parser recomposes the hierarchical id from the renamed parent.
    let descendant_prefix = format!("{}.", canonical_id);

    let mut edits: Vec<RenameEdit> = Vec::new();

    for obj in objects {
        let obj_id = obj.id();
        let obj_file = obj.file();
        let obj_line = obj.line();
        let from_namespace = obj.namespace();

        // Definition edit: the object whose __id is the canonical target.
        if obj_id == canonical_id {
            let local = obj.str_opt("__local_id").unwrap_or(obj_id);
            let (old_text, new_text) =
                definition_edit_texts(read_line, obj_file, obj_line, local, new_id);
            edits.push(RenameEdit {
                file: obj_file.to_string(),
                line: obj_line,
                old_text,
                new_text,
                kind: "definition",
            });
        }

        // Reference edits: each structured reference that resolves to the target or a descendant.
        for r in obj.references() {
            let target = match r.get("target").and_then(|v| v.as_str()) {
                Some(t) => t,
                None => continue,
            };
            let applies = match obj_index
                .resolve_object(target, from_namespace)
                .and_then(|o| o.get("__id").and_then(|v| v.as_str()))
            {
                Some(id) => id == canonical_id || id.starts_with(&descendant_prefix),
                None => false,
            };
            if !applies {
                continue;
            }
            let raw = r.get("raw").and_then(|v| v.as_str()).unwrap_or("");
            let new_text = rewrite_reference_raw(raw, old_id, new_id);
            // Skip references whose surface text doesn't contain `old_id` (e.g. a relative
            // local-id reference): there is nothing to rewrite even though it resolves.
            if new_text == raw {
                continue;
            }
            let line = r.get("line").and_then(|v| v.as_i64()).unwrap_or(obj_line);
            edits.push(RenameEdit {
                file: obj_file.to_string(),
                line,
                old_text: raw.to_string(),
                new_text,
                kind: "reference",
            });
        }
    }

    // Deterministic ordering (file, line) so output is stable across transports.
    edits.sort_by(|a, b| {
        a.file
            .cmp(&b.file)
            .then(a.line.cmp(&b.line))
            .then(a.old_text.cmp(&b.old_text))
    });

    Ok(edits)
}

/// Compute a rename plan for `old_id` → `new_id` across the workspace (MCP/CLI surface).
///
/// Thin wrapper over [`plan_rename_edits`] that injects a disk-backed `read_line` and wraps the
/// edits in the shared success envelope (NFR-4 bounded).
///
/// # Returns
/// - `Ok(Value)` — `{ old_id, new_id, edits: [{file,line,old_text,new_text,kind}], edit_count, truncated }`.
/// - `Err(Value)` — `invalid-argument` / `not-found`.
pub fn rename_plan(index: &ResolvedIndex, old_id: &str, new_id: &str) -> Result<Value, Value> {
    let root = index.root.clone();
    let read_line = move |file: &str, line_1based: i64| -> Option<String> {
        if line_1based < 1 {
            return None;
        }
        let content = std::fs::read_to_string(root.join(file)).ok()?;
        content
            .lines()
            .nth((line_1based - 1) as usize)
            .map(|s| s.to_string())
    };

    let edits = plan_rename_edits(index.objects(), old_id, new_id, &read_line)?;

    let edit_values: Vec<Value> = edits
        .iter()
        .map(|e| {
            json!({
                "file": e.file,
                "line": e.line,
                "old_text": e.old_text,
                "new_text": e.new_text,
                "kind": e.kind,
            })
        })
        .collect();

    // NFR-4: bound the edit list, surfacing truncation alongside the domain key.
    let (edit_values, truncated, remaining) =
        crate::core::envelope::bound_list(edit_values, crate::core::envelope::DEFAULT_LIMIT);
    let mut body = json!({
        "old_id": old_id,
        "new_id": new_id,
        "edits": edit_values,
        "edit_count": edit_values.len()
    });
    if truncated {
        body["truncated"] = json!(true);
        body["remaining"] = json!(remaining);
    } else {
        body["truncated"] = json!(false);
    }
    Ok(ErrorEnvelope::success(body))
}

/// Compute the `(old_text, new_text)` pair for a definition-anchor edit.
///
/// Uses `read_line` to obtain the definition's source line (disk for Core, buffer for the LSP)
/// and extracts the **complete** `[[…]]` anchor (e.g. `[[user]]` or `[[users:Table]]`) so the
/// edit is a full, unambiguous token. Only the id portion is swapped; any `:Kind` / `: [Type]`
/// suffix and the closing `]]` are preserved.
///
/// `local` is the source anchor id (the object's `__local_id`, or `__id` for top-level objects).
/// Falls back to the simple complete anchor `[[local]]` → `[[new_id]]` when the source line is
/// unavailable or the anchor cannot be located (never emits a partial token).
fn definition_edit_texts<F>(
    read_line: &F,
    file: &str,
    line_1based: i64,
    local: &str,
    new_id: &str,
) -> (String, String)
where
    F: Fn(&str, i64) -> Option<String>,
{
    let fallback = || (format!("[[{}]]", local), format!("[[{}]]", new_id));

    if line_1based < 1 {
        return fallback();
    }
    let line = match read_line(file, line_1based) {
        Some(l) => l,
        None => return fallback(),
    };

    // Locate `[[<local>` where the char after the id is an anchor boundary (`]`, `:`, or
    // whitespace) — avoids matching `[[username]]` when renaming `user`.
    let needle = format!("[[{}", local);
    let mut search_from = 0;
    while let Some(rel) = line[search_from..].find(&needle) {
        let start = search_from + rel;
        let after = start + needle.len();
        let boundary_ok = line[after..]
            .chars()
            .next()
            .map(|c| c == ']' || c == ':' || c.is_whitespace())
            .unwrap_or(false);
        if boundary_ok {
            if let Some(close_rel) = line[start..].find("]]") {
                let anchor = &line[start..start + close_rel + 2];
                let rest = &anchor[needle.len()..]; // suffix + closing, e.g. ":Table]]" or "]]"
                return (anchor.to_string(), format!("[[{}{}", new_id, rest));
            }
        }
        search_from = after;
    }

    fallback()
}

/// Validate the new ID: non-empty, reasonable characters (alphanumeric, dash, underscore, dot).
fn validate_new_id(new_id: &str) -> Result<(), Value> {
    if new_id.is_empty() {
        return Err(ErrorEnvelope::error(
            ErrorCode::InvalidArgument,
            "new_id must not be empty",
        ));
    }
    let valid = new_id
        .chars()
        .all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == '.');
    if !valid {
        return Err(ErrorEnvelope::error(
            ErrorCode::InvalidArgument,
            format!(
                "new_id contains invalid characters: '{}'. Only alphanumeric, dash, underscore, and dot are allowed.",
                new_id
            ),
        ));
    }
    Ok(())
}

/// Rewrite the leading `old_id` id-token in a reference's raw text with `new_id`, preserving
/// any `[[`, `#`, `ns:` prefix and `.child` / `:Kind` / `]]` suffix.
///
/// `old_id` is replaced only where it appears as a *bounded* id token: preceded by `#`, `:`,
/// `[`, or start-of-string, and followed by an id boundary (`.`, `:`, `]`, whitespace, or end).
/// This rewrites both a direct anchor (`[[#team]]` → `[[#squad]]`) and a descendant prefix
/// (`[[#team.config]]` → `[[#squad.config]]`) without the over-replacement of a naive
/// `str::replace` (which would also corrupt `[[#team.steam]]`). Returns the original string
/// unchanged when no bounded `old_id` token is present.
fn rewrite_reference_raw(raw: &str, old_id: &str, new_id: &str) -> String {
    if old_id.is_empty() {
        return raw.to_string();
    }
    let bytes = raw.as_bytes();
    let mut search = 0;
    while let Some(rel) = raw[search..].find(old_id) {
        let start = search + rel;
        let end = start + old_id.len();
        let before_ok = start == 0 || matches!(bytes[start - 1], b'#' | b':' | b'[');
        let after_ok = match raw[end..].chars().next() {
            None => true,
            Some(c) => c == '.' || c == ':' || c == ']' || c.is_whitespace(),
        };
        if before_ok && after_ok {
            return format!("{}{}{}", &raw[..start], new_id, &raw[end..]);
        }
        search = end;
    }
    raw.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::index_seam::get_index;
    use crate::core::resolved_index::ResolvedIndex;

    // --- rewrite_reference_raw (pure prefix replacement) --------------------

    #[test]
    fn rewrite_direct_anchor() {
        assert_eq!(
            rewrite_reference_raw("[[#team]]", "team", "squad"),
            "[[#squad]]"
        );
        assert_eq!(
            rewrite_reference_raw("[[team]]", "team", "squad"),
            "[[squad]]"
        );
    }

    #[test]
    fn rewrite_descendant_prefix_only() {
        assert_eq!(
            rewrite_reference_raw("[[#team.config]]", "team", "squad"),
            "[[#squad.config]]"
        );
        assert_eq!(
            rewrite_reference_raw("[[#team.members.alice]]", "team", "squad"),
            "[[#squad.members.alice]]"
        );
    }

    #[test]
    fn rewrite_preserves_kind_and_namespace() {
        assert_eq!(
            rewrite_reference_raw("[[team:Group]]", "team", "squad"),
            "[[squad:Group]]"
        );
        assert_eq!(
            rewrite_reference_raw("[[#ns:team]]", "team", "squad"),
            "[[#ns:squad]]"
        );
    }

    #[test]
    fn rewrite_does_not_over_replace_substring() {
        // `team` is a substring of `steam`/`teamwork` but not a bounded id token there.
        assert_eq!(
            rewrite_reference_raw("[[#team.steam]]", "team", "squad"),
            "[[#squad.steam]]"
        );
        assert_eq!(
            rewrite_reference_raw("[[#teamwork]]", "team", "squad"),
            "[[#teamwork]]"
        );
    }

    // --- end-to-end cascade over a real index -------------------------------

    fn index_for(content: &str) -> (tempfile::TempDir, ResolvedIndex) {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("readme.qmd.md"),
            "# Workspace [[ws: __Workspace]]\n",
        )
        .unwrap();
        std::fs::write(tmp.path().join("input.qmd.md"), content).unwrap();
        let index = get_index(tmp.path()).expect("index built");
        (tmp, index)
    }

    fn edit_pairs(result: &Value) -> Vec<(String, String, String)> {
        result["edits"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| {
                (
                    e["kind"].as_str().unwrap_or("").to_string(),
                    e["old_text"].as_str().unwrap_or("").to_string(),
                    e["new_text"].as_str().unwrap_or("").to_string(),
                )
            })
            .collect()
    }

    /// Renaming a parent must cascade to descendant references — the behavior the MCP suite
    /// previously did not cover at all. Fails on the pre-cascade implementation (which omitted
    /// the `[[#team.config]]` / `[[#team.members.alice]]` edits).
    #[test]
    fn rename_cascades_to_descendant_references() {
        let content = "# Team [[team]]\n\
                       \n\
                       - name: Engineering\n\
                       \n\
                       ## Config [[config]]\n\
                       \n\
                       - env: production\n\
                       \n\
                       ## Members [[members: [User]]]\n\
                       \n\
                       ### Alice [[alice]]\n\
                       \n\
                       - role: admin\n\
                       \n\
                       Ref to parent: [[#team]]\n\
                       \n\
                       Ref to child: [[#team.config]]\n\
                       \n\
                       Ref to deep: [[#team.members.alice]]\n";
        let (_tmp, index) = index_for(content);

        let result = rename_plan(&index, "team", "squad").expect("rename_plan ok");
        let edits = edit_pairs(&result);

        // Definition anchor of the parent.
        assert!(
            edits.iter().any(|(k, o, n)| k == "definition"
                && o.contains("[[team")
                && n.contains("[[squad")),
            "missing parent definition edit: {edits:?}"
        );
        // Direct reference.
        assert!(
            edits
                .iter()
                .any(|(_, o, n)| o == "[[#team]]" && n == "[[#squad]]"),
            "missing direct reference edit: {edits:?}"
        );
        // CASCADE: child reference — only the `team` prefix is rewritten.
        assert!(
            edits
                .iter()
                .any(|(_, o, n)| o == "[[#team.config]]" && n == "[[#squad.config]]"),
            "missing cascading child reference edit: {edits:?}"
        );
        // CASCADE: deep descendant reference.
        assert!(
            edits
                .iter()
                .any(|(_, o, n)| o == "[[#team.members.alice]]" && n == "[[#squad.members.alice]]"),
            "missing cascading deep reference edit: {edits:?}"
        );
        // No descendant *definition* edits (children are anchored by local id).
        assert!(
            !edits
                .iter()
                .any(|(k, o, _)| k == "definition" && (o.contains("config") || o.contains("alice"))),
            "unexpected descendant definition edit: {edits:?}"
        );
    }

    #[test]
    fn rename_unknown_id_is_not_found() {
        let (_tmp, index) = index_for("# Team [[team]]\n\n- name: x\n");
        let err = rename_plan(&index, "nonexistent", "squad").unwrap_err();
        assert_eq!(err["error"]["code"], "not-found");
    }
}
