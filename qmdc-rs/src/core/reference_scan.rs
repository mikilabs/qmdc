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

/// Parse a reference target into `(workspace, namespace, id)`.
///
/// QMD-69: a reference target is a right-aligned suffix of the `__global_id` grammar
/// `workspace:namespace:id`, with an optional `.field` suffix carried inside `id`.
/// There is no `Kind` segment.
///
/// * `#id` — one segment: `(None, None, id)`. Resolves inside the referring object's
///   own workspace, own namespace first (see `reference_scan`).
/// * `#ns:id` — two segments: `(None, Some(ns), id)`.
/// * `#ws:ns:id` — three segments: `(Some(ws), Some(ns), id)`.
/// * `#ws::id` — three segments with an EMPTY middle: `(Some(ws), Some(""), id)`. The
///   empty namespace ELIDES rather than asserting a root namespace, so the target may
///   live in any namespace of `ws`; more than one candidate is an ambiguity.
///
/// `file#id` keeps only the id part. A bare dotted id (`a.b`, no colon) is returned
/// whole as the `id` (dots preserved) — that is how hierarchical/full-path ids and
/// `.field` paths are looked up. The single source of truth for reference-target parsing.
pub fn parse_reference_target(target: &str) -> (Option<String>, Option<String>, String) {
    let s = target.strip_prefix('#').unwrap_or(target);

    // Handle file#id (cross-file reference) — extract just the id part.
    let s = if let Some((_, id_part)) = s.split_once('#') {
        id_part
    } else {
        s
    };

    if let Some((first_part, rest)) = s.split_once(':') {
        if let Some((second_part, id_part)) = rest.split_once(':') {
            // workspace:namespace:id — `second_part` may be empty (elided namespace).
            return (
                Some(first_part.to_string()),
                Some(second_part.to_string()),
                id_part.to_string(),
            );
        }
        // namespace:id
        return (None, Some(first_part.to_string()), rest.to_string());
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

// (file, kind, namespace, workspace, line)
type IdLoc = (String, String, String, String, u32);
// (file, kind, namespace, workspace, id, line)
type LocalLoc = (String, String, String, String, String, u32);

/// Whether a candidate in `cand_workspace` satisfies a reference's WORKSPACE qualifier
/// (QMD-69).
///
/// * `Some(ws)` binds the match to that workspace.
/// * `None` keeps it inside `obj_workspace`, because a cross-workspace reference must name
///   its workspace.
///
/// An empty `obj_workspace` (single-workspace parse, or the LSP's freshly-parsed open
/// document) imposes no constraint, and so does an empty `cand_workspace` — that is a
/// workspace ROOT object, reachable only when the id being looked up is the workspace's own
/// name, which the caller checks.
fn workspace_matches(
    ref_workspace: Option<&str>,
    cand_workspace: &str,
    obj_workspace: &str,
) -> bool {
    match ref_workspace {
        Some(rws) => cand_workspace == rws,
        None => {
            obj_workspace.is_empty() || cand_workspace.is_empty() || cand_workspace == obj_workspace
        }
    }
}

/// Whether a candidate at `(cand_workspace, cand_namespace)` satisfies a reference's
/// qualifiers (QMD-69).
///
/// Used by every path that resolves a reference by its `__id`: the main candidate filter, the
/// field-reference escape and the `ambiguous_field_reference` check. A path that decides on
/// its own is exactly how the surfaces drifted apart in the first place.
///
/// `ref_namespace` is `Some(ns)` for an assertion, `Some("")` for the ELIDED form (`ws::id`,
/// no namespace constraint), and `None` when the reference named none — which imposes no
/// constraint here either, the own-namespace-first preference being the caller's rule.
///
/// The `__local_id` fallback deliberately does NOT use this: there an unqualified reference
/// is scoped to the referring object's own namespace exactly, which is a different rule and
/// is spelled out at that call site.
fn qualifiers_match(
    ref_workspace: Option<&str>,
    ref_namespace: Option<&str>,
    cand_workspace: &str,
    cand_namespace: &str,
    obj_workspace: &str,
) -> bool {
    if !workspace_matches(ref_workspace, cand_workspace, obj_workspace) {
        return false;
    }
    match ref_namespace {
        Some("") => true,
        Some(rns) => cand_namespace == rns,
        None => true,
    }
}

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
            obj.workspace().to_string(),
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
            obj.workspace().to_string(),
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
    // QMD-69: a MULTIMAP, not `id -> first object`. The field-reference check has to pick a
    // candidate that satisfies the reference's qualifiers, and in a composed container the
    // same id legitimately exists in several workspaces — keeping only the first silently
    // inspected the wrong object.
    let mut obj_by_id: HashMap<&str, Vec<&Value>> = HashMap::new();
    for obj in index_objects {
        let id = obj.id();
        if !id.is_empty() {
            obj_by_id.entry(id).or_default().push(obj);
        }
    }

    let mut findings = Vec::new();

    for obj in iter_objects {
        let obj_id = obj.id();
        let file_path = obj.file().to_string();
        let obj_ns = obj.namespace_id();
        // The referring object's own workspace. Empty in a single-workspace parse and in
        // the LSP's freshly-parsed open document, where it correctly imposes no filter.
        let obj_workspace = obj.workspace().to_string();
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

            let (ref_workspace, ref_namespace, ref_id) = parse_reference_target(target);

            // QMD-69: one predicate for every resolution path — see `qualifiers_match`.
            let matching: Vec<&IdLoc> = objects_by_id
                .get(&ref_id)
                .map(|cands| {
                    cands
                        .iter()
                        .filter(|(_, _, ns, ws, _)| {
                            qualifiers_match(
                                ref_workspace.as_deref(),
                                ref_namespace.as_deref(),
                                ws,
                                ns,
                                &obj_workspace,
                            )
                        })
                        .collect()
                })
                .unwrap_or_default();

            let prefer_own_namespace = ref_namespace.is_none();
            let resolved: Vec<&IdLoc> = if prefer_own_namespace {
                if let Some(ons) = &obj_namespace {
                    let same: Vec<&IdLoc> = matching
                        .iter()
                        .filter(|(_, _, ns, _, _)| ns == ons)
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
                //
                // QMD-69: this path must honour the reference's WORKSPACE qualifier exactly
                // as the main filter does. It used to ignore the workspace entirely, so
                // `[[#no_such_ws:ns:leaf]]` was accepted.
                //
                // Its NAMESPACE rule is deliberately different from `qualifiers_match` and is
                // kept as it always was: an unqualified reference resolves by `__local_id`
                // only inside the referring object's OWN namespace (the root namespace when
                // it has none). That is what makes a bare `[[#config]]` at the workspace root
                // NOT reach `gateway.config` in the `services` namespace — pinned by
                // `local-id-cross-namespace-no-fallback` and by the `errors` fixture.
                let mut local_resolved = false;
                if let Some(cands) = by_local_id.get(&ref_id) {
                    let own_ns = obj_namespace.as_deref().unwrap_or("");
                    let filtered: Vec<&LocalLoc> = cands
                        .iter()
                        .filter(|(_, _, ns, ws, _, _)| {
                            if !workspace_matches(ref_workspace.as_deref(), ws, &obj_workspace) {
                                return false;
                            }
                            match ref_namespace.as_deref() {
                                // `ws::id` — namespace elided: any namespace.
                                Some("") => true,
                                // `ns:id` / `ws:ns:id` — asserted namespace.
                                Some(rns) => ns == rns,
                                // Unqualified — own namespace exactly.
                                None => ns == own_ns,
                            }
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
                                .map(|(_, k, ns, _ws, id, _)| {
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
                //
                // QMD-69: the prefix object must itself satisfy the reference's qualifiers.
                // This escape used to accept any object with a matching id and field, so a
                // reference naming a workspace that does not exist — or one that exists but
                // does not hold the object — was silently treated as a field reference and
                // never reported.
                let mut is_field_ref = false;
                if let Some(dot) = ref_id.rfind('.') {
                    let prefix = &ref_id[..dot];
                    let field = &ref_id[dot + 1..];
                    if let Some(cands) = obj_by_id.get(prefix) {
                        is_field_ref = cands.iter().any(|cand| {
                            qualifiers_match(
                                ref_workspace.as_deref(),
                                ref_namespace.as_deref(),
                                cand.workspace(),
                                cand.namespace_id(),
                                &obj_workspace,
                            ) && cand.get(field).is_some()
                                && !field.starts_with("__")
                        });
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
                        .filter(|(_, _, ns, _, _, _)| match &obj_namespace {
                            Some(ons) => ns != ons,
                            None => !ns.is_empty(),
                        })
                        .collect();
                    if let Some((_, _, ns, _ws, id, _)) = other.first() {
                        hint = format!(". Did you mean [[#{}:{}]]?", ns, id);
                    }
                }
                if hint.is_empty() {
                    if let Some(cands) = objects_by_id.get(&ref_id) {
                        let other: Vec<&IdLoc> = cands
                            .iter()
                            .filter(|(_, _, ns, _, _)| match &obj_namespace {
                                Some(ons) => ns != ons,
                                None => !ns.is_empty(),
                            })
                            .collect();
                        if let Some((_, _, ns, _, _)) = other.first() {
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
                //
                // QMD-69: same qualifier rule as the field-ref escape above — the prefix
                // object considered here must be one the reference could actually name, or
                // QMDC009 would be raised about an object in a workspace the reference never
                // mentioned.
                if let Some(dot) = ref_id.rfind('.') {
                    let prefix = &ref_id[..dot];
                    let field = &ref_id[dot + 1..];
                    {
                        if let Some(cand) = obj_by_id.get(prefix).and_then(|cands| {
                            cands.iter().find(|cand| {
                                qualifiers_match(
                                    ref_workspace.as_deref(),
                                    ref_namespace.as_deref(),
                                    cand.workspace(),
                                    cand.namespace_id(),
                                    &obj_workspace,
                                )
                            })
                        }) {
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
                    resolved.iter().map(|(_, k, _, _, _)| k).collect();
                let namespaces: std::collections::HashSet<&String> =
                    resolved.iter().map(|(_, _, ns, _, _)| ns).collect();
                // QMD-69: there is no Kind segment to suppress ambiguity with, and an
                // elided namespace (`ws::id`) explicitly MAY match several namespaces —
                // which is an ambiguity, not a silent pick.
                let is_ambiguous = { kinds.len() > 1 || namespaces.len() > 1 };
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
                            .map(|(f, k, ns, _ws, l)| {
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
