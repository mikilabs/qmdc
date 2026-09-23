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

/// Normalise OS path separators to `/` for logical output (`__file`, `files`,
/// `uri`, `error.file`) so QMD's logical paths are portable and identical across
/// platforms and parsers (QMD-65). Replaces **only the platform separator**
/// (`std::path::MAIN_SEPARATOR`): on Windows `strip_prefix` yields `\`, converted
/// here to `/`; a genuine no-op on Unix (separator is already `/`).
///
/// Deliberately does NOT replace `\` unconditionally: on Unix a backslash is a
/// valid filename character (only `/` and NUL are forbidden), so `weird\name.md`
/// is a single component and must be preserved. Only for logical output — never
/// for a string handed back to the OS as a filesystem path.
pub(crate) fn path_to_slash(rel: &Path) -> String {
    sep_to_slash(&rel.to_string_lossy(), std::path::MAIN_SEPARATOR)
}

/// Testable core of [`path_to_slash`]: replace `sep` with `/`. Production always
/// passes `MAIN_SEPARATOR`; tests pass `'\\'` explicitly to exercise the Windows
/// branch on any host (a no-op-on-Unix helper is otherwise untestable on macOS).
fn sep_to_slash(s: &str, sep: char) -> String {
    s.replace(sep, "/")
}

/// Shared `__Workspace` marker check. Detects `[[id: __Workspace]]` in readme
/// content, allowing optional whitespace after the colon. Single source of truth
/// for workspace-root detection (avoids divergent inline regexes), consistent
/// with the resolver/indexer (which recognises a bare top-level marker and
/// otherwise falls back to a virtual workspace).
fn content_has_workspace_marker(content: &str) -> bool {
    use std::sync::OnceLock;
    static WORKSPACE_MARKER_RE: OnceLock<Regex> = OnceLock::new();
    let re =
        WORKSPACE_MARKER_RE.get_or_init(|| Regex::new(r"\[\[[^\]]+:\s*__Workspace\]\]").unwrap());
    re.is_match(content)
}

/// Does the directory's `readme.qmd.md` declare a `__Workspace`?
/// Used by the core/MCP resolver (`resolve_root_bidirectional`) for its self-check.
pub fn dir_is_workspace_root(dir: &Path) -> bool {
    let readme = dir.join("readme.qmd.md");
    if !readme.is_file() {
        return false;
    }
    match fs::read_to_string(&readme) {
        Ok(content) => content_has_workspace_marker(&content),
        Err(_) => false,
    }
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

/// Maximum directory depth for downward workspace discovery.
///
/// A fixed, non-configurable default (QMD-63): the MCP resolver's downward scan
/// (`find_nested_workspace_roots_bounded`) is capped at this depth so pointing a tool
/// at a large checkout (build artifacts, vendored deps) can't turn a single call into
/// a full-tree crawl. Workspaces normally live near the top; a marker deeper than this
/// is not discovered by the MCP downward scan. Documented in `docs/mcp/readme.qmd.md`.
pub const WORKSPACE_SCAN_MAX_DEPTH: usize = 5;

/// Find all nested workspace roots within a directory (unbounded depth).
/// Returns paths to directories containing [[id:__Workspace]] in readme.qmd.md.
///
/// Used by `parse_workspace`/`scan_workspace` for nested-workspace detection and
/// exclusion, which must see nesting at *any* depth — so this variant is not
/// depth-capped. It is still `.qmdcignore`-pruned via `filter_entry` (descent into
/// ignored directories is skipped, not merely filtered from results).
pub fn find_nested_workspace_roots(root_path: &Path) -> Vec<PathBuf> {
    find_nested_workspace_roots_bounded(root_path, usize::MAX)
}

/// Depth-bounded variant of [`find_nested_workspace_roots`]. The crawl is capped at
/// `max_depth` directory levels. Used by the MCP resolver (with
/// [`WORKSPACE_SCAN_MAX_DEPTH`]) so pointing a tool at a large checkout can't turn a
/// single call into a full-tree crawl; the unbounded wrapper is used where complete
/// nested-workspace detection is required.
pub fn find_nested_workspace_roots_bounded(root_path: &Path, max_depth: usize) -> Vec<PathBuf> {
    let ignore_set = load_qmdcignore(root_path);
    let mut roots = Vec::new();

    // `filter_entry` prunes descent into ignored dirs; `max_depth` caps the crawl.
    let walker = WalkDir::new(root_path)
        .max_depth(max_depth)
        .into_iter()
        .filter_entry(|e| !is_ignored(e.path(), root_path, &ignore_set));

    for entry in walker.filter_map(|e| e.ok()) {
        let path = entry.path();

        // Skip root directory
        if path == root_path {
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

    // Deterministic: shortest first (an ancestor sorts before its descendants),
    // with a lexicographic tie-break so equal-length siblings have a stable order
    // across filesystems/OSes (keeps the MCP `ambiguous` candidate list stable).
    roots.sort_by(|a, b| {
        a.as_os_str()
            .len()
            .cmp(&b.as_os_str().len())
            .then_with(|| a.cmp(b))
    });
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
                files.push(path_to_slash(rel_path));
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
                    .map(path_to_slash)
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

    // Duplicate IDs (QMDC003) — namespace-scoped per QMD-67. Produced by the SINGLE
    // shared detector (`core::ops::validate::collect_duplicate_issues`), the exact same
    // one the MCP `validate` op and the LSP consume, so the three surfaces cannot drift
    // (QMD-68). Cross-file duplicates and same-file/different-kind duplicates are emitted
    // here; same-file/same-kind is intentionally left to the parser (`__ParsingError`).
    for dup in crate::core::ops::validate::collect_duplicate_issues(&all_objects) {
        errors.push(WorkspaceError {
            error_type: "duplicate_id".to_string(),
            message: dup.message,
            file: Some(dup.file),
            line: Some(dup.line as u32),
            object: Some(dup.id),
            field_name: None,
            reference: None,
            candidates: if dup.candidates.is_empty() {
                None
            } else {
                Some(dup.candidates)
            },
            severity: "error".to_string(),
        });
    }

    // Build file content cache from already-parsed files
    let mut file_content_cache: HashMap<String, Vec<String>> = HashMap::new();
    for pf in &parsed_files {
        file_content_cache.insert(
            pf.file_path.clone(),
            pf.content.lines().map(|s| s.to_string()).collect(),
        );
    }

    // QMD-68: broken_link / ambiguous_reference / ambiguous_field_reference are produced
    // by the SINGLE shared engine (`core::reference_scan`) — the same algorithm the LSP and
    // MCP use, so the three surfaces can never drift. The CLI passes file content so the
    // historical double-backtick inline-code suppression is preserved.
    for f in crate::core::reference_scan::reference_scan(
        &all_objects,
        &all_objects,
        Some(&file_content_cache),
    ) {
        errors.push(WorkspaceError {
            error_type: f.kind.type_str().to_string(),
            message: f.message,
            file: Some(f.file),
            line: Some(f.line),
            object: Some(f.object),
            field_name: None,
            reference: Some(f.reference),
            candidates: if f.candidates.is_empty() {
                None
            } else {
                Some(f.candidates)
            },
            severity: "error".to_string(),
        });
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
                        obj_map.insert("__file".to_string(), json!(path_to_slash(rel_path)));
                    }
                }
            }
            all_objects.push(obj);
        }

        // Make file paths relative to root_path
        for file in ws_result.files {
            if let Ok(rel_path) = ws_dir.join(&file).strip_prefix(root_path) {
                all_files.push(path_to_slash(rel_path));
            }
        }

        // Adjust error file paths to be relative to root_path.
        // QMD-69: reference findings are dropped here and recomputed once over the
        // composed object set below — in isolation this workspace could not see its
        // siblings' objects, so any cross-workspace reference looked broken.
        for mut error in ws_result.errors {
            if is_reference_finding(&error.error_type) {
                continue;
            }
            if let Some(ref file) = error.file {
                if let Ok(rel_path) = ws_dir.join(file).strip_prefix(root_path) {
                    error.file = Some(path_to_slash(rel_path));
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
                    .map(path_to_slash)
                    .unwrap_or_else(|_| path_to_slash(&file_path));

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

    // QMD-69: re-run reference validation over the COMPOSED object set.
    //
    // Each sibling workspace above was parsed and validated in isolation by
    // `parse_workspace`, so its reference findings were computed against an index that
    // could not see the other workspaces' objects. A workspace-qualified cross-workspace
    // reference was therefore always reported as a broken link. Those stale findings are
    // dropped as the per-workspace errors are collected (see `is_reference_finding`), and
    // the single shared engine runs once here over every object in the container.
    //
    // Structural findings (duplicate_id, workspace_in_wrong_file, parsing errors) stay
    // per-workspace: they are scoped to one workspace by definition, and identity is
    // workspace-scoped (QMD-67), so a container must not report a duplicate across
    // siblings.
    {
        let mut file_content_cache: HashMap<String, Vec<String>> = HashMap::new();
        for rel_file in &all_files {
            if let Ok(content) = fs::read_to_string(root_path.join(rel_file)) {
                file_content_cache.insert(
                    rel_file.clone(),
                    content.lines().map(String::from).collect(),
                );
            }
        }
        for f in crate::core::reference_scan::reference_scan(
            &all_objects,
            &all_objects,
            Some(&file_content_cache),
        ) {
            all_errors.push(WorkspaceError {
                error_type: f.kind.type_str().to_string(),
                message: f.message,
                file: Some(f.file),
                line: Some(f.line),
                object: Some(f.object),
                field_name: None,
                reference: Some(f.reference),
                candidates: if f.candidates.is_empty() {
                    None
                } else {
                    Some(f.candidates)
                },
                severity: "error".to_string(),
            });
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

/// Whether a `WorkspaceError` was produced by the reference resolver.
///
/// QMD-69: these are recomputed over the composed object set when a container holds
/// several sibling workspaces, so the per-workspace copies must be discarded first.
fn is_reference_finding(error_type: &str) -> bool {
    matches!(
        error_type,
        "broken_link" | "ambiguous_reference" | "ambiguous_field_reference"
    )
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

#[cfg(test)]
mod qmd65_tests {
    use super::{path_to_slash, sep_to_slash};
    use std::path::Path;

    /// The Windows-separator branch renders logical paths with forward slashes
    /// (QMD-65). Driven on any host by passing `'\\'` explicitly, since the
    /// production wrapper is a no-op on macOS (`MAIN_SEPARATOR == '/'`).
    #[test]
    fn sep_to_slash_converts_windows_separator() {
        assert_eq!(sep_to_slash("db\\models.qmd.md", '\\'), "db/models.qmd.md");
        assert_eq!(
            sep_to_slash("architecture\\domain\\readme.qmd.md", '\\'),
            "architecture/domain/readme.qmd.md"
        );
    }

    /// On Unix a backslash is a valid filename character (only `/` and NUL are
    /// forbidden), so it must be preserved — the platform-separator replace never
    /// touches it. Guards against a naive unconditional `\` -> `/` replace.
    #[test]
    fn sep_to_slash_preserves_literal_backslash_on_unix() {
        assert_eq!(
            sep_to_slash("weird\\name.qmd.md", '/'),
            "weird\\name.qmd.md"
        );
        assert_eq!(sep_to_slash("a/b/c.qmd.md", '/'), "a/b/c.qmd.md");
    }

    /// The production wrapper is a no-op for already-`/` paths on every host.
    #[test]
    fn path_to_slash_passthrough_forward_slashes() {
        assert_eq!(path_to_slash(Path::new("a/b/c.qmd.md")), "a/b/c.qmd.md");
    }
}
