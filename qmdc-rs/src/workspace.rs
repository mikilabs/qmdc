//! QMDC Workspace - Multi-file parsing with cross-file references.

use globset::{Glob, GlobSetBuilder};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

use crate::{parse, OutputFormat, ParseOptions};

/// (file, kind, namespace, id, line) — location tuple for indexed objects.
type ObjectLocation = (String, String, String, String, u32);

/// Shared `__Workspace` marker check. Detects `[[id: __Workspace]]` in readme
/// content, allowing optional whitespace after the colon. Single source of truth
/// for workspace-root detection (avoids divergent inline regexes).
fn content_has_workspace_marker(content: &str) -> bool {
    use std::sync::OnceLock;
    static WORKSPACE_MARKER_RE: OnceLock<Regex> = OnceLock::new();
    let re =
        WORKSPACE_MARKER_RE.get_or_init(|| Regex::new(r"\[\[[^\]]+:\s*__Workspace\]\]").unwrap());
    re.is_match(content)
}

/// Check if position is inside backticks (inline code).
/// Handles both single backticks (`) and double backticks (``).
fn is_inside_backticks(line: &str, pos: usize) -> bool {
    let bytes = line.as_bytes();
    let mut in_backtick = false;
    let mut i = 0;

    while i < bytes.len() && i < pos {
        if bytes[i] == b'`' {
            // Check for triple backticks (code fence) - treat entire line as code
            if i + 2 < bytes.len() && bytes[i + 1] == b'`' && bytes[i + 2] == b'`' {
                return true;
            }
            // Check for double backticks (``) - treat as single backtick pair
            if i + 1 < bytes.len() && bytes[i + 1] == b'`' {
                i += 1; // Skip second backtick
                in_backtick = !in_backtick;
            } else {
                in_backtick = !in_backtick;
            }
        }
        i += 1;
    }
    in_backtick
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceError {
    #[serde(rename = "type")]
    pub error_type: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub candidates: Option<Vec<String>>,
    pub severity: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceResult {
    pub root: String,
    pub workspace_id: Option<String>,
    pub files: Vec<String>,
    pub objects: Vec<Value>,
    pub errors: Vec<WorkspaceError>,
}

/// Find all nested workspace roots within a directory.
/// Returns paths to directories containing [[id:__Workspace]] in readme.qmd.md.
/// Respects .qmdcignore patterns.
pub fn find_nested_workspace_roots(root_path: &Path) -> Vec<PathBuf> {
    let ignore_set = load_qmdcignore(root_path);
    let mut roots = Vec::new();

    for entry in WalkDir::new(root_path).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();

        // Skip root directory
        if path == root_path {
            continue;
        }

        // Check .qmdcignore before processing
        if is_ignored(path, root_path, &ignore_set) {
            continue;
        }

        // Check if this is a readme.qmd.md
        if path
            .file_name()
            .map(|n| n == "readme.qmd.md")
            .unwrap_or(false)
        {
            // Skip root readme
            if path.parent() == Some(root_path) {
                continue;
            }

            if let Ok(content) = fs::read_to_string(path) {
                if content_has_workspace_marker(&content) {
                    if let Some(parent) = path.parent() {
                        roots.push(parent.to_path_buf());
                    }
                }
            }
        }
    }

    roots
}

/// Scan workspace directory for all *.qmd.md files.
/// Excludes files from nested workspaces.
/// Respects .qmdcignore patterns.
pub fn scan_workspace(root_path: &Path, exclude_nested: bool) -> Vec<String> {
    let ignore_set = load_qmdcignore(root_path);
    let nested_roots = if exclude_nested {
        find_nested_workspace_roots(root_path)
    } else {
        Vec::new()
    };

    let mut files = Vec::new();

    for entry in WalkDir::new(root_path).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();

        // Skip files in nested workspace directories
        if nested_roots.iter().any(|nr| path.starts_with(nr)) {
            continue;
        }

        // Check .qmdcignore before processing
        if is_ignored(path, root_path, &ignore_set) {
            continue;
        }

        if path.extension().map(|e| e == "md").unwrap_or(false)
            && path.to_string_lossy().contains(".qmd.")
        {
            if let Ok(rel_path) = path.strip_prefix(root_path) {
                files.push(rel_path.to_string_lossy().to_string());
            }
        }
    }

    // Sort: readme.qmd.md first in each directory
    files.sort_by(|a, b| {
        let a_parts: Vec<&str> = a.split('/').collect();
        let b_parts: Vec<&str> = b.split('/').collect();

        let a_dir = if a_parts.len() > 1 {
            a_parts[..a_parts.len() - 1].join("/")
        } else {
            String::new()
        };
        let b_dir = if b_parts.len() > 1 {
            b_parts[..b_parts.len() - 1].join("/")
        } else {
            String::new()
        };

        if a_dir != b_dir {
            return a_dir.cmp(&b_dir);
        }

        let a_file = a_parts.last().unwrap_or(&"");
        let b_file = b_parts.last().unwrap_or(&"");

        let a_priority = if *a_file == "readme.qmd.md" { 0 } else { 1 };
        let b_priority = if *b_file == "readme.qmd.md" { 0 } else { 1 };

        if a_priority != b_priority {
            return a_priority.cmp(&b_priority);
        }

        a_file.cmp(b_file)
    });

    files
}

/// Find __Workspace object in parsed objects.
fn find_workspace_object(objects: &[Value]) -> Option<&Value> {
    objects
        .iter()
        .find(|obj| obj.get("__kind").and_then(|v| v.as_str()) == Some("__Workspace"))
}

/// Find __Namespace object in parsed objects.
fn find_namespace_object(objects: &[Value]) -> Option<&Value> {
    objects
        .iter()
        .find(|obj| obj.get("__kind").and_then(|v| v.as_str()) == Some("__Namespace"))
}

/// Get line number where object is defined.
fn get_line_number(content: &str, obj: &Value) -> u32 {
    let obj_id = obj.get("__id").and_then(|v| v.as_str()).unwrap_or("");
    let obj_kind = obj.get("__kind").and_then(|v| v.as_str()).unwrap_or("");

    let pattern1 = format!(
        r"^\s*#+\s+.*\[\[{}:{}\]\]",
        regex::escape(obj_id),
        regex::escape(obj_kind)
    );
    let pattern2 = format!(r"^\s*#+\s+.*\[\[{}\]\]", regex::escape(obj_id));

    let re1 = Regex::new(&pattern1).ok();
    let re2 = Regex::new(&pattern2).ok();

    for (i, line) in content.lines().enumerate() {
        if let Some(ref re) = re1 {
            if re.is_match(line) {
                return (i + 1) as u32;
            }
        }
        if let Some(ref re) = re2 {
            if re.is_match(line) {
                return (i + 1) as u32;
            }
        }
    }

    1 // Default to line 1
}

/// Parse entire workspace.
pub fn parse_workspace(root_path: &Path, format: OutputFormat) -> WorkspaceResult {
    let root = root_path.to_path_buf();
    let ignore_set = load_qmdcignore(&root);
    let files = scan_workspace(&root, true);

    // Check for nested workspaces (this is an error)
    let nested_workspace_roots = find_nested_workspace_roots(&root);
    let mut errors: Vec<WorkspaceError> = Vec::new();

    for nested_root in &nested_workspace_roots {
        let nested_readme = nested_root.join("readme.qmd.md");
        if let Ok(content) = fs::read_to_string(&nested_readme) {
            let options = ParseOptions {
                random_seed: Some(666),
                format,
            };
            let objects = parse(&content, options);

            if let Some(ws_obj) = find_workspace_object(&objects) {
                let ws_id = ws_obj.get("__id").and_then(|v| v.as_str()).unwrap_or("");
                let rel_path = nested_readme
                    .strip_prefix(&root)
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_default();

                errors.push(WorkspaceError {
                    error_type: "nested_workspace".to_string(),
                    message: format!("Nested workspace '{}' found inside workspace. Workspaces cannot be nested.", ws_id),
                    file: Some(rel_path),
                    line: Some(get_line_number(&content, ws_obj)),
                    object: Some(ws_id.to_string()),
                    field_name: None,
                    reference: None,
                    candidates: None,
                    severity: "error".to_string(),
                });
            }
        }
    }

    struct ParsedFile {
        file_path: String,  // relative to workspace root
        full_path: PathBuf, // absolute within workspace
        file_dir: String,   // parent dir of file_path (relative), "" for root
        is_readme: bool,
        content: String,
        objects: Vec<Value>, // parsed objects (Full)
    }

    // Single pass: read + parse each file once (Full), keep content for line fallback
    let mut parsed_files: Vec<ParsedFile> = Vec::new();
    for file_path in &files {
        let full_path = root.join(file_path);
        if let Ok(content) = fs::read_to_string(&full_path) {
            let options = ParseOptions {
                random_seed: Some(666),
                format: OutputFormat::Full, // always Full: validations rely on __references
            };
            let objects = parse(&content, options);

            let file_dir = Path::new(file_path)
                .parent()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default();

            let is_readme = Path::new(file_path)
                .file_name()
                .and_then(|n| n.to_str())
                .map(|n| n == "readme.qmd.md")
                .unwrap_or(false);

            parsed_files.push(ParsedFile {
                file_path: file_path.clone(),
                full_path,
                file_dir,
                is_readme,
                content,
                objects,
            });
        }
    }

    // Discover workspace + namespaces from already-parsed files
    let mut workspace_id: Option<String> = None;
    let mut workspace_ref: Option<String> = None;
    let mut namespace_map: HashMap<String, String> = HashMap::new(); // dir -> namespace id

    for pf in &parsed_files {
        if pf.is_readme {
            if let Some(ws_obj) = find_workspace_object(&pf.objects) {
                workspace_id = ws_obj
                    .get("__id")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                if let Some(ref id) = workspace_id {
                    workspace_ref = Some(id.clone()); // plain ID
                }
            }
            if let Some(ns_obj) = find_namespace_object(&pf.objects) {
                if let Some(ns_id) = ns_obj.get("__id").and_then(|v| v.as_str()) {
                    namespace_map.insert(pf.file_dir.clone(), ns_id.to_string());
                }
            }
        } else {
            // __Workspace in non-readme is an error (unless ignored)
            if !is_ignored(&pf.full_path, &root, &ignore_set) {
                if let Some(ws_obj) = find_workspace_object(&pf.objects) {
                    let ws_id = ws_obj.get("__id").and_then(|v| v.as_str()).unwrap_or("");
                    errors.push(WorkspaceError {
                        error_type: "workspace_in_wrong_file".to_string(),
                        message: format!(
                            "Workspace '{}' must be defined in readme.qmd.md, not in '{}'.",
                            ws_id, pf.file_path
                        ),
                        file: Some(pf.file_path.clone()),
                        line: Some(get_line_number(&pf.content, ws_obj)),
                        object: Some(ws_id.to_string()),
                        field_name: None,
                        reference: None,
                        candidates: None,
                        severity: "error".to_string(),
                    });
                }
            }
        }
    }

    // Resolve namespace for any directory, with memoization
    let mut ns_cache: HashMap<String, Option<String>> = HashMap::new();
    let mut resolve_namespace_for_dir = |dir: &str| -> Option<String> {
        if let Some(v) = ns_cache.get(dir) {
            return v.clone();
        }
        let mut check_dir = dir.to_string();
        loop {
            if let Some(ns) = namespace_map.get(&check_dir) {
                let v = Some(ns.clone());
                ns_cache.insert(dir.to_string(), v.clone());
                return v;
            }
            if check_dir.is_empty() {
                ns_cache.insert(dir.to_string(), None);
                return None;
            }
            check_dir = Path::new(&check_dir)
                .parent()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default();
        }
    };

    // Build final object list with metadata, without re-parsing files
    let mut all_objects: Vec<Value> = Vec::new();
    for pf in &parsed_files {
        let namespace_id = resolve_namespace_for_dir(&pf.file_dir);
        for mut obj in pf.objects.clone() {
            let kind = obj
                .get("__kind")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            // Skip __ParsingError objects - they are handled separately and shouldn't get __file/__line
            if kind == "__ParsingError" {
                continue;
            }

            // Skip __Workspace objects from non-readme files
            if !pf.is_readme && kind == "__Workspace" {
                continue;
            }

            let line_num = obj
                .get("__line")
                .and_then(|v| v.as_u64())
                .map(|l| l as u32)
                .unwrap_or_else(|| get_line_number(&pf.content, &obj));

            if let Value::Object(map) = &mut obj {
                map.insert("__file".to_string(), json!(pf.file_path.clone()));
                map.insert("__line".to_string(), json!(line_num));

                // Add workspace reference (except for __Workspace itself)
                if kind != "__Workspace" {
                    if let Some(ref ws_ref) = workspace_ref {
                        map.insert("__workspace".to_string(), json!(ws_ref));
                    }
                }

                // Add namespace reference
                if kind != "__Workspace" && kind != "__Namespace" {
                    if let Some(ref ns_id) = namespace_id {
                        map.insert("__namespace".to_string(), json!(ns_id));
                    }
                } else if kind == "__Namespace" {
                    if let Some(ref ws_ref) = workspace_ref {
                        map.insert("__workspace".to_string(), json!(ws_ref));
                    }
                }
            }

            all_objects.push(obj);
        }
    }

    // Phase 3: Resolve dot-ID parents
    // Objects with __local_id == __id and "." in __id are dot-ID declarations
    // that need parent resolution from the global object graph
    {
        let all_ids: std::collections::HashSet<String> = all_objects
            .iter()
            .filter_map(|obj| obj.get("__id").and_then(|v| v.as_str()).map(String::from))
            .collect();

        for obj in &mut all_objects {
            let obj_id = obj
                .get("__id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let local_id = obj
                .get("__local_id")
                .and_then(|v| v.as_str())
                .map(String::from);

            // Dot-ID detection: __local_id equals __id AND contains a dot
            // (same-file children have __local_id != __id)
            if let Some(ref lid) = local_id {
                if lid != &obj_id || !obj_id.contains('.') {
                    continue;
                }
            } else {
                continue;
            }

            // Already has a parent (shouldn't happen, but guard)
            if obj.get("__parent").and_then(|v| v.as_str()).is_some() {
                continue;
            }

            // Split on last dot to get parent path
            let last_dot = obj_id.rfind('.').unwrap();
            let parent_path = &obj_id[..last_dot];

            if all_ids.contains(parent_path) {
                if let Value::Object(map) = obj {
                    map.insert(
                        "__parent".to_string(),
                        json!(format!("[[#{}]]", parent_path)),
                    );
                }
            } else {
                errors.push(WorkspaceError {
                    error_type: "broken_parent".to_string(),
                    message: format!("Parent object '{}' not found in workspace", parent_path),
                    file: obj.get("__file").and_then(|v| v.as_str()).map(String::from),
                    line: obj.get("__line").and_then(|v| v.as_u64()).map(|l| l as u32),
                    object: Some(obj_id.clone()),
                    field_name: None,
                    reference: None,
                    candidates: None,
                    severity: "error".to_string(),
                });
            }
        }
    }

    // Build index of all objects by id, kind, and namespace for validation
    let mut objects_by_id: HashMap<String, Vec<(String, String, String, u32)>> = HashMap::new(); // id -> [(file, kind, namespace, line), ...]

    for obj in &all_objects {
        // Skip __ParsingError objects - they are handled separately and shouldn't participate in duplicate_id validation
        if let Some(kind) = obj.get("__kind").and_then(|v| v.as_str()) {
            if kind == "__ParsingError" {
                continue;
            }
        }

        if let (Some(id), Some(file), Some(line)) = (
            obj.get("__id").and_then(|v| v.as_str()),
            obj.get("__file").and_then(|v| v.as_str()),
            obj.get("__line").and_then(|v| v.as_u64()),
        ) {
            let kind = obj
                .get("__kind")
                .and_then(|v| v.as_str())
                .unwrap_or("__Object");
            let namespace = obj
                .get("__namespace")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()) // Already plain ID
                .unwrap_or_default();
            let line = line as u32;

            objects_by_id.entry(id.to_string()).or_default().push((
                file.to_string(),
                kind.to_string(),
                namespace,
                line,
            ));
        }
    }

    // Build index of objects by __local_id for fallback resolution
    // local_id -> [(file, kind, namespace, id, line), ...]
    let mut by_local_id: HashMap<String, Vec<ObjectLocation>> = HashMap::new();

    for obj in &all_objects {
        if let Some(kind) = obj.get("__kind").and_then(|v| v.as_str()) {
            // Skip non-user-facing system kinds (match Python/TypeScript filtering)
            let user_facing = ["__Workspace", "__Namespace", "__Document", "__Object"];
            if kind.starts_with("__") && !user_facing.contains(&kind) {
                continue;
            }
        }

        if let (Some(local_id), Some(file), Some(line)) = (
            obj.get("__local_id").and_then(|v| v.as_str()),
            obj.get("__file").and_then(|v| v.as_str()),
            obj.get("__line").and_then(|v| v.as_u64()),
        ) {
            if !local_id.is_empty() {
                let kind = obj
                    .get("__kind")
                    .and_then(|v| v.as_str())
                    .unwrap_or("__Object");
                let namespace = obj
                    .get("__namespace")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .unwrap_or_default();
                let id = obj
                    .get("__id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let line = line as u32;

                by_local_id.entry(local_id.to_string()).or_default().push((
                    file.to_string(),
                    kind.to_string(),
                    namespace,
                    id,
                    line,
                ));
            }
        }
    }

    // Check for duplicate IDs (same id, different files or same file)
    // Skip system objects (__Document, __TextBlock) as they are auto-generated per file
    for (id, locations) in &objects_by_id {
        // Skip system objects with auto-generated IDs
        let is_system_object = locations.iter().any(|(_, kind, _, _)| {
            kind == "__Document" || kind == "__TextBlock" || kind == "__ParsingError"
        });
        if is_system_object {
            continue;
        }

        if locations.len() > 1 {
            // Check if duplicates are in different files
            let files: std::collections::HashSet<&String> =
                locations.iter().map(|(f, _, _, _)| f).collect();
            if files.len() > 1 {
                // Duplicate ID across files
                for location in locations.iter().skip(1) {
                    errors.push(WorkspaceError {
                        error_type: "duplicate_id".to_string(),
                        message: format!("Duplicate ID '{}' found in multiple files", id),
                        file: Some(location.0.clone()),
                        line: Some(location.3),
                        object: Some(id.clone()),
                        field_name: None,
                        reference: None,
                        candidates: Some(
                            locations
                                .iter()
                                .map(|(f, _, _, l)| format!("{}:{}", f, l))
                                .collect(),
                        ),
                        severity: "error".to_string(),
                    });
                }
            } else {
                // Same file - check if different kinds
                let kinds: std::collections::HashSet<&String> =
                    locations.iter().map(|(_, k, _, _)| k).collect();
                if kinds.len() > 1 {
                    // Same ID, different kinds - ambiguous
                    let first_kind = &locations[0].1;
                    for location in locations.iter().skip(1) {
                        errors.push(WorkspaceError {
                            error_type: "duplicate_id".to_string(),
                            message: format!(
                                "Duplicate ID '{}' with different kinds: {} and {}",
                                id, first_kind, location.1
                            ),
                            file: Some(location.0.clone()),
                            line: Some(location.3),
                            object: Some(id.clone()),
                            field_name: None,
                            reference: None,
                            candidates: Some(
                                locations
                                    .iter()
                                    .map(|(f, k, _, l)| format!("{}:{}:{}", f, k, l))
                                    .collect(),
                            ),
                            severity: "error".to_string(),
                        });
                    }
                } else {
                    // Same file, same kind — parser already detects these
                    // (emits __ParsingError with type=duplicate_id).
                    // No need to re-detect here; doing so would also
                    // false-positive on skeleton objects the parser emits
                    // for object-array children.
                }
            }
        }
    }

    // Build file content cache from already-parsed files
    let mut file_content_cache: HashMap<String, Vec<String>> = HashMap::new();
    for pf in &parsed_files {
        file_content_cache.insert(
            pf.file_path.clone(),
            pf.content.lines().map(|s| s.to_string()).collect(),
        );
    }

    // Create regex once for double backticks check
    let double_backtick_re = Regex::new(r"``").unwrap();

    // Check for broken links and ambiguous references using already-parsed __references
    for obj in &all_objects {
        let obj_id = obj.get("__id").and_then(|v| v.as_str()).unwrap_or("");
        let file_path = obj
            .get("__file")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        // Determine current namespace for the file (used for objects without explicit __namespace)
        let file_dir = Path::new(&file_path)
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();
        let current_namespace = resolve_namespace_for_dir(&file_dir);

        let obj_namespace = obj
            .get("__namespace")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .or_else(|| current_namespace.clone());

        if let Some(refs) = obj.get("__references").and_then(|v| v.as_array()) {
            for r in refs {
                if let (Some(target), Some(line)) = (
                    r.get("target").and_then(|v| v.as_str()),
                    r.get("line").and_then(|v| v.as_u64()),
                ) {
                    let line = line as u32;

                    let (ref_namespace, ref_kind, ref_id) = parse_reference_target(target);

                    let matching_objects: Vec<_> = objects_by_id
                        .get(&ref_id)
                        .map(|candidates| {
                            candidates
                                .iter()
                                .filter(|(_, kind, ns, _)| {
                                    if let Some(ref_ns) = &ref_namespace {
                                        return ns == ref_ns;
                                    }
                                    if let Some(ref_k) = &ref_kind {
                                        if kind != ref_k {
                                            return false;
                                        }
                                    }
                                    true
                                })
                                .collect()
                        })
                        .unwrap_or_default();

                    let resolved_objects: Vec<_> = if ref_namespace.is_none() {
                        if let Some(obj_ns) = &obj_namespace {
                            let same_ns: Vec<_> = matching_objects
                                .iter()
                                .filter(|(_, _, ns, _)| ns == obj_ns)
                                .copied()
                                .collect();
                            if !same_ns.is_empty() {
                                same_ns
                            } else {
                                matching_objects.to_vec()
                            }
                        } else {
                            matching_objects.to_vec()
                        }
                    } else {
                        matching_objects.to_vec()
                    };

                    if resolved_objects.is_empty() {
                        // Check if reference is inside backticks (inline code) - skip validation
                        let mut skip_validation = false;

                        // Get file content from cache (already parsed)
                        if let Some(file_lines) = file_content_cache.get(&file_path) {
                            if line > 0 && line <= file_lines.len() as u32 {
                                let orig_line = &file_lines[(line - 1) as usize];
                                // Find position of reference in line
                                let raw_ref =
                                    r.get("raw").and_then(|v| v.as_str()).unwrap_or(target);
                                if let Some(ref_pos) = orig_line.find(raw_ref) {
                                    // Check if reference is inside backticks
                                    if is_inside_backticks(orig_line, ref_pos) {
                                        skip_validation = true;
                                    }
                                    // Also check if reference is between double backticks (``...``)
                                    if !skip_validation {
                                        let matches: Vec<_> = double_backtick_re
                                            .find_iter(orig_line)
                                            .map(|m| m.start())
                                            .collect();
                                        for i in (0..matches.len()).step_by(2) {
                                            if i + 1 < matches.len() {
                                                let start_pos = matches[i];
                                                let end_pos = matches[i + 1];
                                                if start_pos < ref_pos && ref_pos < end_pos {
                                                    skip_validation = true;
                                                    break;
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        if skip_validation {
                            continue;
                        }

                        // __local_id fallback resolution
                        let mut local_resolved = false;
                        if let Some(local_candidates) = by_local_id.get(&ref_id) {
                            // Filter by target namespace (ref_ns if explicit, else source obj namespace)
                            let target_ns: Option<&str> = if ref_namespace.is_some() {
                                ref_namespace.as_deref()
                            } else {
                                obj_namespace.as_deref()
                            };
                            let filtered: Vec<_> = local_candidates
                                .iter()
                                .filter(|(_, _, ns, _, _)| {
                                    if let Some(tns) = target_ns {
                                        ns == tns
                                    } else {
                                        ns.is_empty()
                                    }
                                })
                                .collect();

                            if filtered.len() == 1 {
                                // Resolved via __local_id — no error
                                local_resolved = true;
                            } else if filtered.len() > 1 {
                                // Ambiguous by __local_id
                                errors.push(WorkspaceError {
                                    error_type: "ambiguous_reference".to_string(),
                                    message: format!(
                                        "Ambiguous reference '{}' - multiple objects match by __local_id",
                                        target
                                    ),
                                    file: Some(file_path.clone()),
                                    line: Some(line),
                                    object: Some(obj_id.to_string()),
                                    field_name: None,
                                    reference: Some(format!("[[{}]]", target)),
                                    candidates: Some(
                                        filtered
                                            .iter()
                                            .map(|(_, k, ns, id, _)| {
                                                if ns.is_empty() {
                                                    format!("{}:{}", k, id)
                                                } else {
                                                    format!("{}:{}:{}", ns, k, id)
                                                }
                                            })
                                            .collect(),
                                    ),
                                    severity: "error".to_string(),
                                });
                                local_resolved = true; // Skip further processing
                            }
                            // else: filtered is empty, fall through to existing logic
                        }

                        if !local_resolved {
                            // Field-level reference resolution: if ref_id contains a dot,
                            // check if prefix.field resolves to a field on an object
                            let mut is_field_ref = false;
                            if ref_id.contains('.') {
                                let last_dot = ref_id.rfind('.').unwrap();
                                let obj_prefix = &ref_id[..last_dot];
                                let field_part = &ref_id[last_dot + 1..];
                                if objects_by_id.contains_key(obj_prefix) {
                                    // Check if the object actually has this field
                                    let candidate_obj = all_objects.iter().find(|o| {
                                        o.get("__id").and_then(|v| v.as_str()) == Some(obj_prefix)
                                    });
                                    if let Some(cand) = candidate_obj {
                                        if cand.get(field_part).is_some()
                                            && !field_part.starts_with("__")
                                        {
                                            is_field_ref = true;
                                        }
                                    }
                                }
                            }
                            if !is_field_ref {
                                // Check if the object exists in a different namespace
                                // (cross-namespace hint for better error messages)
                                let mut hint = String::new();
                                if let Some(other_ns_candidates) = by_local_id.get(&ref_id) {
                                    // Found by __local_id in another namespace
                                    let other_ns: Vec<_> = other_ns_candidates
                                        .iter()
                                        .filter(|(_, _, ns, _, _)| {
                                            if let Some(obj_ns) = &obj_namespace {
                                                ns != obj_ns
                                            } else {
                                                !ns.is_empty()
                                            }
                                        })
                                        .collect();
                                    if !other_ns.is_empty() {
                                        let (_, _, ns, id, _) = other_ns[0];
                                        hint = format!(". Did you mean [[#{}:{}]]?", ns, id);
                                    }
                                }
                                if hint.is_empty() {
                                    // Check by __id in other namespaces
                                    if let Some(id_candidates) = objects_by_id.get(&ref_id) {
                                        let other_ns: Vec<_> = id_candidates
                                            .iter()
                                            .filter(|(_, _, ns, _)| {
                                                if let Some(obj_ns) = &obj_namespace {
                                                    ns != obj_ns
                                                } else {
                                                    !ns.is_empty()
                                                }
                                            })
                                            .collect();
                                        if !other_ns.is_empty() {
                                            let (_, _, ns, _) = other_ns[0];
                                            hint =
                                                format!(". Did you mean [[#{}:{}]]?", ns, ref_id);
                                        }
                                    }
                                }

                                errors.push(WorkspaceError {
                                    error_type: "broken_link".to_string(),
                                    message: format!("Object '{}' not found{}", ref_id, hint),
                                    file: Some(file_path.clone()),
                                    line: Some(line),
                                    object: Some(obj_id.to_string()),
                                    field_name: None,
                                    reference: Some(format!("[[{}]]", target)),
                                    candidates: None,
                                    severity: "error".to_string(),
                                });
                            }
                        }
                    } else if resolved_objects.len() == 1 {
                        // Object found — check for ambiguous_field_reference
                        // If ref_id contains a dot, check if the field-path interpretation
                        // also resolves to a scalar field (not a reference to this object)
                        if ref_id.contains('.') {
                            let last_dot = ref_id.rfind('.').unwrap();
                            let obj_prefix = &ref_id[..last_dot];
                            let field_part = &ref_id[last_dot + 1..];
                            if objects_by_id.contains_key(obj_prefix) {
                                // Find the object with obj_prefix to check its fields
                                let candidate_obj = all_objects.iter().find(|o| {
                                    o.get("__id").and_then(|v| v.as_str()) == Some(obj_prefix)
                                });
                                if let Some(cand) = candidate_obj {
                                    let has_field = cand.get(field_part).is_some()
                                        && !field_part.starts_with("__");
                                    if has_field {
                                        let field_val = cand.get(field_part).unwrap();
                                        // Ambiguous if field value is NOT a reference to the resolved object
                                        let expected_ref =
                                            Value::String(format!("[[#{}]]", ref_id));
                                        if *field_val != expected_ref {
                                            let field_val_repr = {
                                                let s = field_val.to_string();
                                                if s.len() < 40 {
                                                    s
                                                } else {
                                                    format!("{}...", &s[..37])
                                                }
                                            };
                                            errors.push(WorkspaceError {
                                                error_type: "ambiguous_field_reference"
                                                    .to_string(),
                                                message: format!(
                                                    "Reference '{}' cannot be unequivocally resolved to an object or a field",
                                                    target
                                                ),
                                                file: Some(file_path.clone()),
                                                line: Some(line),
                                                object: Some(obj_id.to_string()),
                                                field_name: None,
                                                reference: Some(format!("[[{}]]", target)),
                                                candidates: Some(vec![
                                                    format!("object with __id '{}'", ref_id),
                                                    format!(
                                                        "field '{}' on object '{}' (value: {})",
                                                        field_part, obj_prefix, field_val_repr
                                                    ),
                                                ]),
                                                severity: "error".to_string(),
                                            });
                                        }
                                    }
                                }
                            }
                        }
                    } else if resolved_objects.len() > 1 {
                        let kinds: std::collections::HashSet<&String> =
                            resolved_objects.iter().map(|(_, k, _, _)| k).collect();
                        let namespaces: std::collections::HashSet<&String> =
                            resolved_objects.iter().map(|(_, _, ns, _)| ns).collect();

                        let is_ambiguous = if ref_kind.is_some() && ref_namespace.is_some() {
                            false
                        } else {
                            kinds.len() > 1 || namespaces.len() > 1
                        };

                        if is_ambiguous {
                            errors.push(WorkspaceError {
                                error_type: "ambiguous_reference".to_string(),
                                message: format!(
                                    "Ambiguous reference '{}' - multiple objects match",
                                    target
                                ),
                                file: Some(file_path.clone()),
                                line: Some(line),
                                object: Some(obj_id.to_string()),
                                field_name: None,
                                reference: Some(format!("[[{}]]", target)),
                                candidates: Some(
                                    resolved_objects
                                        .iter()
                                        .map(|(f, k, ns, l)| {
                                            if ns.is_empty() {
                                                format!("{}:{}:{}", f, k, l)
                                            } else {
                                                format!("{}:{}:{}:{}", ns, f, k, l)
                                            }
                                        })
                                        .collect(),
                                ),
                                severity: "error".to_string(),
                            });
                        }
                    }
                }
            }
        }
    }

    // If no explicit workspace found but we have QMD.md files, create virtual workspace
    // BUT: Don't create virtual workspace if there's a workspace_in_wrong_file error
    let has_wrong_file_error = errors
        .iter()
        .any(|e| e.error_type == "workspace_in_wrong_file");

    if workspace_id.is_none() && !files.is_empty() && !has_wrong_file_error {
        // Use folder name as workspace ID
        let virtual_ws_id = root
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "workspace".to_string());

        workspace_id = Some(virtual_ws_id.clone());

        // Create __Workspace object for virtual workspace
        let mut ws_obj = serde_json::Map::new();
        ws_obj.insert("__id".to_string(), json!(virtual_ws_id.clone()));
        ws_obj.insert("__kind".to_string(), json!("__Workspace"));
        ws_obj.insert("__file".to_string(), json!(""));
        ws_obj.insert("__line".to_string(), json!(1));
        ws_obj.insert("name".to_string(), json!(virtual_ws_id.clone()));

        // Add __Workspace object to all_objects (at the beginning)
        all_objects.insert(0, serde_json::Value::Object(ws_obj));

        // Update all existing objects to have __workspace field
        for obj in &mut all_objects {
            if let Value::Object(map) = obj {
                let kind = map.get("__kind").and_then(|v| v.as_str()).unwrap_or("");

                // Add __workspace to all objects except __Workspace itself
                if kind != "__Workspace" {
                    map.insert("__workspace".to_string(), json!(virtual_ws_id.clone()));
                }
            }
        }
    }

    // Extract __ParsingError objects and convert to WorkspaceError
    // Process them directly from parsed_files, not from all_objects (they were excluded from all_objects)
    for pf in &parsed_files {
        for obj in &pf.objects {
            if let Some(kind) = obj.get("__kind").and_then(|k| k.as_str()) {
                if kind == "__ParsingError" {
                    let error_type = obj
                        .get("type")
                        .and_then(|t| t.as_str())
                        .unwrap_or("parsing_error");
                    let reference = obj
                        .get("reference")
                        .and_then(|r| r.as_str())
                        .map(String::from);
                    let line = obj.get("line").and_then(|l| l.as_u64()).unwrap_or(0) as u32;
                    let object_id = obj.get("object").and_then(|o| o.as_str()).map(String::from);
                    let field_name = obj.get("field").and_then(|f| f.as_str()).map(String::from);

                    // Build message from all non-system fields
                    let mut detail_parts: Vec<String> = Vec::new();
                    if let Some(obj_map) = obj.as_object() {
                        for (k, v) in obj_map.iter() {
                            if k.starts_with("__") || k == "type" || k == "line" {
                                continue;
                            }
                            let v_str = match v {
                                serde_json::Value::String(s) => s.clone(),
                                _ => v.to_string(),
                            };
                            detail_parts.push(format!("{}: {}", k, v_str));
                        }
                    }
                    let message = if detail_parts.is_empty() {
                        error_type.to_string()
                    } else {
                        format!("{}: {}", error_type, detail_parts.join(", "))
                    };

                    errors.push(WorkspaceError {
                        error_type: error_type.to_string(),
                        message,
                        file: Some(pf.file_path.clone()),
                        line: Some(line),
                        object: object_id,
                        field_name,
                        reference,
                        candidates: None,
                        severity: "error".to_string(),
                    });
                }
            }
        }
    }

    WorkspaceResult {
        root: root.to_string_lossy().to_string(),
        workspace_id,
        files,
        objects: all_objects,
        errors,
    }
}

/// Parse reference target into (namespace, kind, id)
/// Handles: #id, Kind:id, namespace:id, namespace:Kind:id
fn parse_reference_target(target: &str) -> (Option<String>, Option<String>, String) {
    let s = target.strip_prefix('#').unwrap_or(target);

    // Handle file#id (cross-file reference) - extract just the id part
    let s = if let Some((_, id_part)) = s.split_once('#') {
        id_part
    } else {
        s
    };

    // Handle namespace:Kind:id or namespace:id
    if let Some((first_part, rest)) = s.split_once(':') {
        if let Some((kind_part, id_part)) = rest.split_once(':') {
            // namespace:Kind:id
            return (
                Some(first_part.to_string()),
                Some(kind_part.to_string()),
                id_part.to_string(),
            );
        } else {
            // Could be namespace:id or Kind:id - check if first_part looks like a namespace
            // For now, assume it's Kind:id if it's capitalized, namespace:id otherwise
            // This is a heuristic - in practice, we'd need to check against actual namespace list
            if first_part
                .chars()
                .next()
                .map(|c| c.is_uppercase())
                .unwrap_or(false)
            {
                // Likely Kind:id
                return (None, Some(first_part.to_string()), rest.to_string());
            } else {
                // Likely namespace:id
                return (Some(first_part.to_string()), None, rest.to_string());
            }
        }
    }

    // Just #id - no namespace or kind specified
    (None, None, s.to_string())
}

/// Find all workspace directories (directories containing readme.qmd.md with __Workspace).
/// Respects .qmdcignore patterns.
pub fn find_all_workspace_dirs(root_path: &Path) -> Vec<PathBuf> {
    let ignore_set = load_qmdcignore(root_path);
    let mut workspace_dirs = Vec::new();

    for entry in WalkDir::new(root_path).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();

        // Check .qmdcignore before processing
        if is_ignored(path, root_path, &ignore_set) {
            continue;
        }

        // Check if this is a readme.qmd.md
        if path
            .file_name()
            .map(|n| n == "readme.qmd.md")
            .unwrap_or(false)
        {
            if let Ok(content) = fs::read_to_string(path) {
                if content_has_workspace_marker(&content) {
                    if let Some(parent) = path.parent() {
                        workspace_dirs.push(parent.to_path_buf());
                    }
                }
            }
        }
    }

    workspace_dirs
}

/// Load .qmdcignore patterns from root directory and build a GlobSet
pub fn load_qmdcignore(root_path: &Path) -> Option<globset::GlobSet> {
    let qmdcignore_path = root_path.join(".qmdcignore");

    if !qmdcignore_path.exists() {
        return None;
    }

    let content = match fs::read_to_string(&qmdcignore_path) {
        Ok(c) => c,
        Err(_) => return None,
    };

    let mut builder = GlobSetBuilder::new();

    for line in content.lines() {
        let line = line.trim();

        // Skip empty lines and comments
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        // If pattern ends with /, replace with /** to match all files within
        let pattern = if line.ends_with('/') {
            format!("{}**", line)
        } else {
            line.to_string()
        };

        if let Ok(glob) = Glob::new(&pattern) {
            builder.add(glob);
        }
    }

    builder.build().ok()
}

/// Check if a path should be ignored based on GlobSet
pub fn is_ignored(path: &Path, root_path: &Path, ignore_set: &Option<globset::GlobSet>) -> bool {
    if let Some(ref set) = ignore_set {
        if let Ok(rel_path) = path.strip_prefix(root_path) {
            return set.is_match(rel_path);
        }
    }
    false
}

/// Parse all workspaces found in a directory tree (non-nested).
/// If root_path itself is a workspace, parse only that one.
/// If root_path contains multiple workspace directories, parse all of them.
/// Respects .qmdcignore patterns at the root level.
pub fn parse_all_workspaces(root_path: &Path, format: OutputFormat) -> WorkspaceResult {
    // Load .qmdcignore patterns
    let ignore_set = load_qmdcignore(root_path);

    // Check if root_path itself is a workspace
    let root_readme = root_path.join("readme.qmd.md");
    if root_readme.exists() && !is_ignored(&root_readme, root_path, &ignore_set) {
        if let Ok(content) = fs::read_to_string(&root_readme) {
            if content_has_workspace_marker(&content) {
                // Root is a workspace - use single workspace parsing
                return parse_workspace(root_path, format);
            }
        }
    }

    // Root is not a workspace - find all workspaces in subdirectories
    let all_workspace_dirs = find_all_workspace_dirs(root_path);

    // Filter out ignored workspaces
    let workspace_dirs: Vec<PathBuf> = all_workspace_dirs
        .into_iter()
        .filter(|ws_dir| {
            let readme = ws_dir.join("readme.qmd.md");
            !is_ignored(&readme, root_path, &ignore_set)
        })
        .collect();

    if workspace_dirs.is_empty() {
        // No explicit workspaces found - check if root has .qmd.md files
        // If yes, treat root as a virtual workspace
        // IMPORTANT: Must respect .qmdcignore when checking for files
        let has_qmdc_files = WalkDir::new(root_path)
            .max_depth(5)
            .into_iter()
            .filter_map(|e| e.ok())
            .any(|e| {
                let path = e.path();
                // Check .qmdcignore before considering file
                if is_ignored(path, root_path, &ignore_set) {
                    return false;
                }
                path.extension().map(|ext| ext == "md").unwrap_or(false)
                    && path.to_string_lossy().contains(".qmd.")
            });

        if has_qmdc_files {
            // Treat root as a virtual workspace
            return parse_workspace(root_path, format);
        }

        // No workspaces and no QMD.md files - return empty result
        return WorkspaceResult {
            root: root_path.to_string_lossy().to_string(),
            workspace_id: None,
            files: vec![],
            objects: vec![],
            errors: vec![],
        };
    }

    // Parse each workspace and combine results
    let mut all_objects: Vec<Value> = Vec::new();
    let mut all_files: Vec<String> = Vec::new();
    let mut all_errors: Vec<WorkspaceError> = Vec::new();

    for ws_dir in &workspace_dirs {
        let ws_result = parse_workspace(ws_dir, format);

        // Adjust __file paths in objects to be relative to root_path
        for mut obj in ws_result.objects {
            // Skip __ParsingError objects - they are handled separately
            let kind = obj.get("__kind").and_then(|v| v.as_str()).unwrap_or("");
            if kind == "__ParsingError" {
                continue;
            }

            if let Some(obj_map) = obj.as_object_mut() {
                if let Some(file) = obj_map.get("__file").and_then(|v| v.as_str()) {
                    if let Ok(rel_path) = ws_dir.join(file).strip_prefix(root_path) {
                        obj_map.insert(
                            "__file".to_string(),
                            json!(rel_path.to_string_lossy().to_string()),
                        );
                    }
                }
            }
            all_objects.push(obj);
        }

        // Make file paths relative to root_path
        for file in ws_result.files {
            if let Ok(rel_path) = ws_dir.join(&file).strip_prefix(root_path) {
                all_files.push(rel_path.to_string_lossy().to_string());
            }
        }

        // Adjust error file paths to be relative to root_path
        for mut error in ws_result.errors {
            if let Some(ref file) = error.file {
                if let Ok(rel_path) = ws_dir.join(file).strip_prefix(root_path) {
                    error.file = Some(rel_path.to_string_lossy().to_string());
                }
            }
            all_errors.push(error);
        }
    }

    // After parsing explicit workspaces, check for orphan .qmd.md files
    // (files outside any workspace directory that should be loaded too)
    let mut orphan_files = Vec::new();
    for entry in WalkDir::new(root_path)
        .max_depth(5)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if path.extension().map(|e| e == "md").unwrap_or(false)
            && path.to_string_lossy().contains(".qmd.")
            && !is_ignored(path, root_path, &ignore_set)
        // Apply .qmdcignore filtering
        {
            // Exclude files inside explicit workspace directories
            if !workspace_dirs.iter().any(|ws_dir| path.starts_with(ws_dir)) {
                orphan_files.push(path.to_path_buf());
            }
        }
    }

    if !orphan_files.is_empty() {
        // First pass: check for workspace_in_wrong_file errors in orphan files
        let mut has_wrong_file_error = false;
        for file_path in &orphan_files {
            if let Ok(content) = fs::read_to_string(file_path) {
                let options = ParseOptions {
                    random_seed: Some(666),
                    format,
                };
                let objects = parse(&content, options);

                let is_readme = file_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| n == "readme.qmd.md")
                    .unwrap_or(false);

                for obj in objects {
                    if !is_readme {
                        if let Some(kind) = obj.get("__kind").and_then(|v| v.as_str()) {
                            if kind == "__Workspace" {
                                has_wrong_file_error = true;
                                break;
                            }
                        }
                    }
                }
            }
        }

        // Only create virtual workspace if:
        // 1. There are no explicit workspaces (workspace_dirs.is_empty())
        // 2. There's no workspace_in_wrong_file error
        let should_create_virtual_workspace = workspace_dirs.is_empty() && !has_wrong_file_error;

        // Parse orphan files as if they belong to a virtual workspace
        // at root_path with ID from folder name
        let virtual_ws_id = root_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "workspace".to_string());

        if should_create_virtual_workspace {
            // Create __Workspace object for virtual workspace
            let mut ws_obj = serde_json::Map::new();
            ws_obj.insert("__id".to_string(), json!(virtual_ws_id.clone()));
            ws_obj.insert("__kind".to_string(), json!("__Workspace"));
            ws_obj.insert("__file".to_string(), json!(""));
            ws_obj.insert("__line".to_string(), json!(1));
            ws_obj.insert("name".to_string(), json!(virtual_ws_id.clone()));
            all_objects.insert(0, serde_json::Value::Object(ws_obj));
        }

        for file_path in orphan_files {
            if let Ok(content) = fs::read_to_string(&file_path) {
                let options = ParseOptions {
                    random_seed: Some(666),
                    format,
                };
                let objects = parse(&content, options);

                let rel_file = file_path
                    .strip_prefix(root_path)
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_else(|_| file_path.to_string_lossy().to_string());

                // Check if this is a readme file
                let is_readme = file_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| n == "readme.qmd.md")
                    .unwrap_or(false);

                // Add __file metadata to each object
                for mut obj in objects {
                    // Skip __Workspace objects from non-readme files
                    if !is_readme {
                        if let Some(kind) = obj.get("__kind").and_then(|v| v.as_str()) {
                            if kind == "__Workspace" {
                                // Add error for this invalid workspace (but skip if file is ignored)
                                if !is_ignored(&file_path, root_path, &ignore_set) {
                                    if let Some(ws_id) = obj.get("__id").and_then(|v| v.as_str()) {
                                        all_errors.push(WorkspaceError {
                                            error_type: "workspace_in_wrong_file".to_string(),
                                            message: format!("Workspace '{}' must be defined in readme.qmd.md, not in '{}'.", ws_id, rel_file),
                                            file: Some(rel_file.clone()),
                                            line: Some(get_line_number(&content, &obj)),
                                            object: Some(ws_id.to_string()),
                                            field_name: None,
                                            reference: None,
                                            candidates: None,
                                            severity: "error".to_string(),
                                        });
                                    }
                                }
                                continue; // Skip this object
                            }
                        }
                    }

                    // Skip __ParsingError objects - they are handled separately
                    let kind = obj.get("__kind").and_then(|v| v.as_str()).unwrap_or("");
                    if kind == "__ParsingError" {
                        continue;
                    }

                    if let Some(obj_map) = obj.as_object_mut() {
                        obj_map.insert("__file".to_string(), json!(rel_file.clone()));
                        // Only add __workspace if we created virtual workspace
                        if should_create_virtual_workspace {
                            obj_map.insert("__workspace".to_string(), json!(virtual_ws_id.clone()));
                            // Store plain ID
                        }
                    }
                    all_objects.push(obj);
                }

                all_files.push(rel_file);
            }
        }
    }

    WorkspaceResult {
        root: root_path.to_string_lossy().to_string(),
        workspace_id: None, // Multiple workspaces, no single ID
        files: all_files,
        objects: all_objects,
        errors: all_errors,
    }
}

/// Walk UP from `start_path` looking for the nearest workspace root.
///
/// A directory is a workspace root if it contains `readme.qmd.md` with a
/// `[[id: __Workspace]]` marker. Checks `start_path` itself first, then each
/// ancestor. Returns the first match, or `None` if no ancestor is a workspace.
///
/// The path is canonicalized to an absolute path first so that relative inputs
/// like `.` correctly walk up the filesystem tree (parity with Python's
/// `Path(start).resolve()` and TS `resolve(start)`).
pub fn find_workspace_root(start_path: &Path) -> Option<PathBuf> {
    // Canonicalize to an absolute path so ancestor traversal works for relative
    // inputs (e.g. `.`). Fall back to cwd-joining when the path can't be
    // canonicalized (e.g. it doesn't exist).
    let abs = fs::canonicalize(start_path).unwrap_or_else(|_| {
        if start_path.is_absolute() {
            start_path.to_path_buf()
        } else {
            std::env::current_dir()
                .map(|cwd| cwd.join(start_path))
                .unwrap_or_else(|_| start_path.to_path_buf())
        }
    });

    // If the path is a file, begin from its parent directory.
    let mut dir = if abs.is_file() {
        abs.parent().map(|p| p.to_path_buf())
    } else {
        Some(abs)
    };

    while let Some(current) = dir {
        let readme = current.join("readme.qmd.md");
        if readme.exists() {
            if let Ok(content) = fs::read_to_string(&readme) {
                if content_has_workspace_marker(&content) {
                    return Some(current);
                }
            }
        }
        dir = current.parent().map(|p| p.to_path_buf());
    }

    None
}

/// Unified workspace resolver (QMD-59).
///
/// Lets `workspace parse`/`validate`/`query` work from ANY directory:
///
/// 1. Walk-UP: if `path` itself or any ancestor is a workspace, parse that
///    workspace via `parse_workspace` (preserves nested-workspace detection).
/// 2. Walk-DOWN: otherwise `path` is a non-workspace container; `parse_all_workspaces`
///    resolves each contained sub-workspace independently (union of errors),
///    or falls back to a virtual workspace for orphan files.
pub fn resolve_workspace(path: &Path, format: OutputFormat) -> WorkspaceResult {
    if let Some(root) = find_workspace_root(path) {
        return parse_workspace(&root, format);
    }
    parse_all_workspaces(path, format)
}
