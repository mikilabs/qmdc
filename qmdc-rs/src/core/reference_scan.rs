//! Single reference-validation engine (QMD-68).
//!
//! This is THE one algorithm that produces broken_link / ambiguous_reference /
//! ambiguous_field_reference findings. It is the CLI validator's historical algorithm,
//! lifted verbatim into core so the CLI (`workspace.rs`), the LSP (`compute_diagnostics`),
//! and the MCP `validate` op all share exactly one implementation and can never drift.
//!
//! It operates purely on parsed objects (`&[Value]` carrying `__id`, `__kind`, `__file`,
//! `__line`, `__namespace`, `__references`). `index_objects` is the resolution universe;
//! `iter_objects` is the (possibly file-scoped) set whose references are checked. Optional
//! `file_lines` enables inline-code (backtick) suppression for references the parser did
//! not already strip (double-backtick spans); when absent, only the parser's single-
//! backtick stripping applies.

use std::collections::HashMap;

use serde_json::Value;

use crate::core::fields::QmdcObject;

/// The kind of a reference finding, mapped to both a legacy `type` string (CLI/workspace
/// JSON) and an LSP diagnostic code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefErrorKind {
    BrokenLink,
    Ambiguous,
    AmbiguousField,
}

impl RefErrorKind {
    /// Legacy `type` string used by the CLI `WorkspaceError` JSON and `_expected.json`.
    pub fn type_str(self) -> &'static str {
        match self {
            RefErrorKind::BrokenLink => "broken_link",
            RefErrorKind::Ambiguous => "ambiguous_reference",
            RefErrorKind::AmbiguousField => "ambiguous_field_reference",
        }
    }
    /// LSP diagnostic code.
    pub fn code(self) -> &'static str {
        match self {
            RefErrorKind::BrokenLink => "QMDC001",
            RefErrorKind::Ambiguous => "QMDC002",
            RefErrorKind::AmbiguousField => "QMDC009",
        }
    }
}

/// One reference finding, carrying everything every surface needs: the source object id,
/// the `[[target]]` reference string, position (line + UTF-16 columns), the message, and
/// the candidate list (for ambiguity diagnostics).
#[derive(Debug, Clone)]
pub struct RefFinding {
    pub file: String,
    pub line: u32,
    pub start_col: u32,
    pub end_col: u32,
    pub object: String,
    pub reference: String,
    pub ref_id: String,
    pub kind: RefErrorKind,
    pub message: String,
    pub candidates: Vec<String>,
}

/// Parse a reference target into `(namespace, kind, id)`.
///
/// Handles `#id`, `Kind:id`, `namespace:id`, `namespace:Kind:id`, and `file#id`. A bare
/// dotted id (`a.b`, no colon) is returned whole as the `id` (dots preserved) — this is
/// how hierarchical/full-path ids are looked up. The single source of truth for
/// reference-target parsing (previously duplicated in `workspace.rs`).
pub fn parse_reference_target(target: &str) -> (Option<String>, Option<String>, String) {
    let s = target.strip_prefix('#').unwrap_or(target);

    // Handle file#id (cross-file reference) — extract just the id part.
    let s = if let Some((_, id_part)) = s.split_once('#') {
        id_part
    } else {
        s
    };

    if let Some((first_part, rest)) = s.split_once(':') {
        if let Some((kind_part, id_part)) = rest.split_once(':') {
            // namespace:Kind:id
            return (
                Some(first_part.to_string()),
                Some(kind_part.to_string()),
                id_part.to_string(),
            );
        } else if first_part
            .chars()
            .next()
            .map(|c| c.is_uppercase())
            .unwrap_or(false)
        {
            // Uppercase first segment ⇒ Kind:id
            return (None, Some(first_part.to_string()), rest.to_string());
        } else {
            // namespace:id
            return (Some(first_part.to_string()), None, rest.to_string());
        }
    }

    (None, None, s.to_string())
}

/// Whether `pos` (a byte offset) sits inside an inline-code span on `line`.
/// Single/double/triple backtick aware (a code fence line is treated wholly as code).
pub fn is_inside_backticks(line: &str, pos: usize) -> bool {
    let bytes = line.as_bytes();
    let mut in_backtick = false;
    let mut i = 0;
    while i < bytes.len() && i < pos {
        if bytes[i] == b'`' {
            if i + 2 < bytes.len() && bytes[i + 1] == b'`' && bytes[i + 2] == b'`' {
                return true;
            }
            if i + 1 < bytes.len() && bytes[i + 1] == b'`' {
                i += 1; // consume the second backtick of a `` pair
                in_backtick = !in_backtick;
            } else {
                in_backtick = !in_backtick;
            }
        }
        i += 1;
    }
    in_backtick
}

/// Is the reference at `ref_pos` between a pair of double backticks (``…``) on `line`?
fn between_double_backticks(line: &str, ref_pos: usize) -> bool {
    let matches: Vec<usize> = line.match_indices("``").map(|(i, _)| i).collect();
    let mut i = 0;
    while i + 1 < matches.len() {
        if matches[i] < ref_pos && ref_pos < matches[i + 1] {
            return true;
        }
        i += 2;
    }
    false
}

// (file, kind, namespace, line)
type IdLoc = (String, String, String, u32);
// (file, kind, namespace, id, line)
type LocalLoc = (String, String, String, String, u32);

fn build_objects_by_id(objects: &[Value]) -> HashMap<String, Vec<IdLoc>> {
    let mut map: HashMap<String, Vec<IdLoc>> = HashMap::new();
    for obj in objects {
        if obj.kind() == "__ParsingError" {
            continue;
        }
        // Note: broader system-kind exclusion is applied by `is_system_kind` in the
        // duplicate detector; reference indexing only skips parser-error stubs so that
        // real objects (including __Document/__TextBlock ids, if referenced) remain
        // resolvable exactly as the historical CLI did.
        let id = obj.id();
        if id.is_empty() {
            continue;
        }
        // `__file`/`__line` are only for reporting; the LSP's freshly-parsed open-doc
        // objects carry no `__file`, so we must NOT require it for indexing.
        let kind = if obj.kind().is_empty() {
            "__Object"
        } else {
            obj.kind()
        };
        map.entry(id.to_string()).or_default().push((
            obj.file().to_string(),
            kind.to_string(),
            obj.namespace_id().to_string(),
            obj.line() as u32,
        ));
    }
    map
}

fn build_by_local_id(objects: &[Value]) -> HashMap<String, Vec<LocalLoc>> {
    let user_facing = ["__Workspace", "__Namespace", "__Document", "__Object"];
    let mut map: HashMap<String, Vec<LocalLoc>> = HashMap::new();
    for obj in objects {
        let kind = obj.kind();
        if kind.starts_with("__") && !user_facing.contains(&kind) {
            continue;
        }
        let local_id = obj.str_field("__local_id");
        if local_id.is_empty() {
            continue;
        }
        let kind = if kind.is_empty() { "__Object" } else { kind };
        map.entry(local_id.to_string()).or_default().push((
            obj.file().to_string(),
            kind.to_string(),
            obj.namespace_id().to_string(),
            obj.id().to_string(),
            obj.line() as u32,
        ));
    }
    map
}

/// Run the single reference-validation algorithm. See module docs.
pub fn reference_scan(
    index_objects: &[Value],
    iter_objects: &[Value],
    file_lines: Option<&HashMap<String, Vec<String>>>,
) -> Vec<RefFinding> {
    let objects_by_id = build_objects_by_id(index_objects);
    let by_local_id = build_by_local_id(index_objects);
    // Quick lookup for field checks: id -> object.
    let mut obj_by_id: HashMap<&str, &Value> = HashMap::new();
    for obj in index_objects {
        let id = obj.id();
        if !id.is_empty() {
            obj_by_id.entry(id).or_insert(obj);
        }
    }

    let mut findings = Vec::new();

    for obj in iter_objects {
        let obj_id = obj.id();
        let file_path = obj.file().to_string();
        let obj_ns = obj.namespace_id();
        // A __Namespace object carries no own `__namespace`, but it DEFINES a namespace
        // and resolves its references within it (its own `__id`). Mirrors the Python/TS
        // validators and the CLI's directory-namespace fallback.
        let obj_namespace: Option<String> = if !obj_ns.is_empty() {
            Some(obj_ns.to_string())
        } else if obj.kind() == "__Namespace" && !obj_id.is_empty() {
            Some(obj_id.to_string())
        } else {
            None
        };

        for r in obj.references() {
            let target = match r.get("target").and_then(|v| v.as_str()) {
                Some(t) => t,
                None => continue,
            };
            let line = match r.get("line").and_then(|v| v.as_u64()) {
                Some(l) => l as u32,
                None => continue,
            };
            let start_col = r.get("start_col").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
            let end_col = r.get("end_col").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
            let raw = r.get("raw").and_then(|v| v.as_str()).unwrap_or(target);

            let (ref_namespace, ref_kind, ref_id) = parse_reference_target(target);

            let matching: Vec<&IdLoc> = objects_by_id
                .get(&ref_id)
                .map(|cands| {
                    cands
                        .iter()
                        .filter(|(_, kind, ns, _)| {
                            if let Some(rns) = &ref_namespace {
                                return ns == rns;
                            }
                            if let Some(rk) = &ref_kind {
                                if kind != rk {
                                    return false;
                                }
                            }
                            true
                        })
                        .collect()
                })
                .unwrap_or_default();

            let resolved: Vec<&IdLoc> = if ref_namespace.is_none() {
                if let Some(ons) = &obj_namespace {
                    let same: Vec<&IdLoc> = matching
                        .iter()
                        .filter(|(_, _, ns, _)| ns == ons)
                        .copied()
                        .collect();
                    if same.is_empty() {
                        matching.clone()
                    } else {
                        same
                    }
                } else {
                    matching.clone()
                }
            } else {
                matching.clone()
            };

            if resolved.is_empty() {
                // Inline-code suppression (only when file content is available).
                if let Some(fl) = file_lines {
                    if let Some(lines) = fl.get(&file_path) {
                        if line > 0 && (line as usize) <= lines.len() {
                            let orig = &lines[(line - 1) as usize];
                            if let Some(pos) = orig.find(raw) {
                                if is_inside_backticks(orig, pos)
                                    || between_double_backticks(orig, pos)
                                {
                                    continue;
                                }
                            }
                        }
                    }
                }

                // __local_id fallback.
                let mut local_resolved = false;
                if let Some(cands) = by_local_id.get(&ref_id) {
                    let target_ns: Option<&str> = if ref_namespace.is_some() {
                        ref_namespace.as_deref()
                    } else {
                        obj_namespace.as_deref()
                    };
                    let filtered: Vec<&LocalLoc> = cands
                        .iter()
                        .filter(|(_, _, ns, _, _)| match target_ns {
                            Some(tns) => ns == tns,
                            None => ns.is_empty(),
                        })
                        .collect();
                    if filtered.len() == 1 {
                        local_resolved = true;
                    } else if filtered.len() > 1 {
                        findings.push(RefFinding {
                            file: file_path.clone(),
                            line,
                            start_col,
                            end_col,
                            object: obj_id.to_string(),
                            reference: format!("[[{}]]", target),
                            ref_id: ref_id.clone(),
                            kind: RefErrorKind::Ambiguous,
                            message: format!(
                                "Ambiguous reference '{}' - multiple objects match by __local_id",
                                target
                            ),
                            candidates: filtered
                                .iter()
                                .map(|(_, k, ns, id, _)| {
                                    if ns.is_empty() {
                                        format!("{}:{}", k, id)
                                    } else {
                                        format!("{}:{}:{}", ns, k, id)
                                    }
                                })
                                .collect(),
                        });
                        local_resolved = true;
                    }
                }
                if local_resolved {
                    continue;
                }

                // Field-ref: prefix.field where prefix is an object with that field.
                let mut is_field_ref = false;
                if let Some(dot) = ref_id.rfind('.') {
                    let prefix = &ref_id[..dot];
                    let field = &ref_id[dot + 1..];
                    if objects_by_id.contains_key(prefix) {
                        if let Some(cand) = obj_by_id.get(prefix) {
                            if cand.get(field).is_some() && !field.starts_with("__") {
                                is_field_ref = true;
                            }
                        }
                    }
                }
                if is_field_ref {
                    continue;
                }

                // Broken link with cross-namespace "did you mean" hint.
                let mut hint = String::new();
                if let Some(cands) = by_local_id.get(&ref_id) {
                    let other: Vec<&LocalLoc> = cands
                        .iter()
                        .filter(|(_, _, ns, _, _)| match &obj_namespace {
                            Some(ons) => ns != ons,
                            None => !ns.is_empty(),
                        })
                        .collect();
                    if let Some((_, _, ns, id, _)) = other.first() {
                        hint = format!(". Did you mean [[#{}:{}]]?", ns, id);
                    }
                }
                if hint.is_empty() {
                    if let Some(cands) = objects_by_id.get(&ref_id) {
                        let other: Vec<&IdLoc> = cands
                            .iter()
                            .filter(|(_, _, ns, _)| match &obj_namespace {
                                Some(ons) => ns != ons,
                                None => !ns.is_empty(),
                            })
                            .collect();
                        if let Some((_, _, ns, _)) = other.first() {
                            hint = format!(". Did you mean [[#{}:{}]]?", ns, ref_id);
                        }
                    }
                }

                findings.push(RefFinding {
                    file: file_path.clone(),
                    line,
                    start_col,
                    end_col,
                    object: obj_id.to_string(),
                    reference: format!("[[{}]]", target),
                    ref_id: ref_id.clone(),
                    kind: RefErrorKind::BrokenLink,
                    message: format!("Object '{}' not found{}", ref_id, hint),
                    candidates: Vec::new(),
                });
            } else if resolved.len() == 1 {
                // Possible ambiguous_field_reference for dotted refs.
                if let Some(dot) = ref_id.rfind('.') {
                    let prefix = &ref_id[..dot];
                    let field = &ref_id[dot + 1..];
                    if objects_by_id.contains_key(prefix) {
                        if let Some(cand) = obj_by_id.get(prefix) {
                            if let Some(field_val) = cand.get(field) {
                                if !field.starts_with("__") {
                                    let expected = Value::String(format!("[[#{}]]", ref_id));
                                    if *field_val != expected {
                                        let repr = {
                                            let s = field_val.to_string();
                                            if s.chars().count() < 40 {
                                                s
                                            } else {
                                                let t: String = s.chars().take(37).collect();
                                                format!("{}...", t)
                                            }
                                        };
                                        // Candidate detail is carried in both the
                                        // structured `candidates` list AND appended to the
                                        // human message (QMD-68 review Q2) so every surface
                                        // — CLI, LSP, MCP — shows the two conflicting
                                        // interpretations, not just the bare sentence.
                                        let cand_object = format!("object with __id '{}'", ref_id);
                                        let cand_field = format!(
                                            "field '{}' on object '{}' (value: {})",
                                            field, prefix, repr
                                        );
                                        findings.push(RefFinding {
                                            file: file_path.clone(),
                                            line,
                                            start_col,
                                            end_col,
                                            object: obj_id.to_string(),
                                            reference: format!("[[{}]]", target),
                                            ref_id: ref_id.clone(),
                                            kind: RefErrorKind::AmbiguousField,
                                            message: format!(
                                                "Reference '{}' cannot be unequivocally resolved to an object or a field: {}; {}",
                                                target, cand_object, cand_field
                                            ),
                                            candidates: vec![cand_object, cand_field],
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
            } else {
                // resolved.len() > 1 — kind/namespace multiplicity ambiguity.
                let kinds: std::collections::HashSet<&String> =
                    resolved.iter().map(|(_, k, _, _)| k).collect();
                let namespaces: std::collections::HashSet<&String> =
                    resolved.iter().map(|(_, _, ns, _)| ns).collect();
                let is_ambiguous = if ref_kind.is_some() && ref_namespace.is_some() {
                    false
                } else {
                    kinds.len() > 1 || namespaces.len() > 1
                };
                if is_ambiguous {
                    findings.push(RefFinding {
                        file: file_path.clone(),
                        line,
                        start_col,
                        end_col,
                        object: obj_id.to_string(),
                        reference: format!("[[{}]]", target),
                        ref_id: ref_id.clone(),
                        kind: RefErrorKind::Ambiguous,
                        message: format!(
                            "Ambiguous reference '{}' - multiple objects match",
                            target
                        ),
                        candidates: resolved
                            .iter()
                            .map(|(f, k, ns, l)| {
                                if ns.is_empty() {
                                    format!("{}:{}:{}", f, k, l)
                                } else {
                                    format!("{}:{}:{}:{}", ns, f, k, l)
                                }
                            })
                            .collect(),
                    });
                }
            }
        }
    }

    findings
}
