//! QMDC Workspace - Multi-file parsing with cross-file references.

use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

use crate::ignore::{is_ignored, load_qmdcignore};
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

/// The order every scan reads a workspace's files in: directory, then `readme.qmd.md`
/// first, then file name, each compared by UTF-8 bytes. Python's `_workspace_file_sort_key`
/// and TypeScript's `compareWorkspaceFiles` mirror it. The orphan walk uses it too, since a
/// directory listing's order is the filesystem's and the later of two duplicate ids is the
/// one the query layer keeps (QMD-77). Takes `/`-separated relative paths.
pub(crate) fn compare_workspace_files(a: &str, b: &str) -> std::cmp::Ordering {
    let (a_dir, a_file) = a.rsplit_once('/').unwrap_or(("", a));
    let (b_dir, b_file) = b.rsplit_once('/').unwrap_or(("", b));
    a_dir
        .cmp(b_dir)
        .then_with(|| (a_file != "readme.qmd.md").cmp(&(b_file != "readme.qmd.md")))
        .then_with(|| a_file.cmp(b_file))
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
    // QMD-77 C3: serialized as `objectId`/`fieldName`, the SAME keys `workspace validate` uses.
    // `workspace parse` used to say `object`/`field` for the very same error, so a consumer that
    // read both commands had to know two spellings of one field. Aligning on validate's names keeps
    // the richer pair (validate also carries `fieldName`) and matches every other error surface.
    #[serde(rename = "objectId", skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(rename = "fieldName", skip_serializing_if = "Option::is_none")]
    pub field_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub candidates: Option<Vec<String>>,
    pub severity: String,
}

/// One workspace inside a result (QMD-72): where it is on disk, and where its files sit in
/// the result's `__file` values.
///
/// `__file` is relative to the result's base, and that keeps it unique within one result:
/// two workspaces' `readme.qmd.md` must never collapse into one string, because the
/// reference scanner, the error reports and the `files` list all key on it. `path` is where
/// this workspace sits inside the base (`""` when it IS the base) and `root` is where that
/// place really is, so a consumer locates any file with one rule: take the entry whose
/// `path` is the longest prefix of `__file` and join its `root` with the rest.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkspaceEntry {
    pub id: String,
    pub root: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceResult {
    /// The base every `__file` is relative to, as a canonical absolute path. `None` when the
    /// base is virtual: the `-w` form, whose workspaces need not share any directory, puts
    /// each one at its own id instead (QMD-72).
    pub root: Option<String>,
    pub workspace_id: Option<String>,
    /// Every workspace in the result, ordered by `path` (QMD-72). See [`WorkspaceEntry`].
    pub workspaces: Vec<WorkspaceEntry>,
    pub files: Vec<String>,
    pub objects: Vec<Value>,
    pub errors: Vec<WorkspaceError>,
}

/// Canonical absolute form of `p` with `/` separators, for what a result reports about where
/// things are on disk (QMD-72).
///
/// Canonical rather than lexical, because the `-w` duplicate check already treats two
/// spellings of one directory (a symlink and its target) as the same path; the reported root
/// has to agree with that. Falls back to cwd-joining for a path that cannot be canonicalized.
pub(crate) fn canonical_slash(p: &Path) -> String {
    let abs = fs::canonicalize(p).unwrap_or_else(|_| {
        if p.is_absolute() {
            p.to_path_buf()
        } else {
            std::env::current_dir()
                .map(|cwd| cwd.join(p))
                .unwrap_or_else(|_| p.to_path_buf())
        }
    });
    let s = path_to_slash(&abs);
    // Windows `canonicalize` returns a verbatim path; report the form a user would type.
    if let Some(rest) = s.strip_prefix("//?/UNC/") {
        return format!("//{}", rest);
    }
    match s.strip_prefix("//?/") {
        Some(rest) => rest.to_string(),
        None => s,
    }
}

/// The entry for a single-workspace result: the workspace IS the base, so its `path` is `""`.
fn single_entry(workspace_id: &Option<String>, root: &Path) -> Vec<WorkspaceEntry> {
    workspace_id
        .iter()
        .map(|id| WorkspaceEntry {
            id: id.clone(),
            root: canonical_slash(root),
            path: String::new(),
        })
        .collect()
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
        .filter_entry(|e| !is_ignored(e.path(), root_path, &ignore_set, e.file_type().is_dir()));

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

    // `filter_entry` prunes descent into ignored dirs, as `find_nested_workspace_roots_bounded`
    // already does. The result is unchanged either way, because `is_ignored` walks every
    // ancestor of a path, so a file under an ignored dir was skipped anyway (QMD-73).
    for entry in WalkDir::new(root_path)
        .into_iter()
        .filter_entry(|e| !is_ignored(e.path(), root_path, &ignore_set, e.file_type().is_dir()))
        .filter_map(|e| e.ok())
    {
        let path = entry.path();

        // Skip files in nested workspace directories
        if nested_roots.iter().any(|nr| path.starts_with(nr)) {
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

    files.sort_by(|a, b| compare_workspace_files(a, b));

    files
}

/// Find __Workspace object in parsed objects.
fn find_workspace_object(objects: &[Value]) -> Option<&Value> {
    objects
        .iter()
        .find(|obj| obj.get("__kind").and_then(|v| v.as_str()) == Some("__Workspace"))
}

/// The readable stem a file's synthesised ids are built from: its path relative to the directory
/// that declared its namespace (relative to the workspace root when it has none), lowercased with
/// every run of other characters folded to `_`. Two files can fold to one stem;
/// [`assign_synthetic_ids`] is what makes the ids unique.
///
/// A single-file `parse` has no workspace and no namespace, so it keeps the counter form it has
/// always had — one document cannot collide with itself. Only the workspace layer, which is where
/// two files meet and where the graph key `<workspace>:<namespace>:<id>` is formed, qualifies them.
pub fn synthetic_id_stem(file_path: &str, namespace_dir: Option<&str>) -> String {
    let rel = match namespace_dir {
        Some(dir) if !dir.is_empty() => file_path
            .strip_prefix(dir)
            .map(|r| r.trim_start_matches('/'))
            .unwrap_or(file_path),
        _ => file_path,
    };
    let rel = rel.strip_suffix(".qmd.md").unwrap_or(rel);
    let mut out = String::with_capacity(rel.len());
    let mut pending_sep = false;
    for ch in rel.chars() {
        let lower = ch.to_ascii_lowercase();
        if lower.is_ascii_lowercase() || lower.is_ascii_digit() {
            if pending_sep && !out.is_empty() {
                out.push('_');
            }
            pending_sep = false;
            out.push(lower);
        } else {
            pending_sep = true;
        }
    }
    out
}

/// One file's input to [`assign_synthetic_ids`].
struct SyntheticIdInput<'a> {
    file_path: &'a str,
    namespace: Option<&'a str>,
    namespace_dir: Option<&'a str>,
    objects: &'a [Value],
}

/// The final id of every synthesised `__Document` / `__TextBlock` in a workspace, per file:
/// `file path -> (parser id -> workspace id)`.
///
/// The stem alone is not unique — folding to `[a-z0-9_]` maps `a-b`, `a_b` and `a/b` to one stem,
/// and a name with no ASCII letter or digit folds to nothing. So uniqueness is assigned, not
/// derived: the files of one namespace are taken in path byte order, the first keeps the plain
/// stem, and a file whose ids are already taken — by an earlier file, or by an id an author wrote
/// anywhere in that namespace — takes the smallest free `_<k>` suffix (k = 1, 2, …), the scheme
/// GitHub's heading-anchor slugger uses. A stem that folds to nothing is `file`.
fn assign_synthetic_ids(inputs: &[SyntheticIdInput]) -> HashMap<String, HashMap<String, String>> {
    let is_synthesised = |obj: &Value| {
        matches!(
            obj.get("__kind").and_then(|v| v.as_str()),
            Some("__Document") | Some("__TextBlock")
        )
    };
    let mut taken: HashMap<String, HashSet<String>> = HashMap::new();
    for input in inputs {
        let ids = taken
            .entry(input.namespace.unwrap_or("").to_string())
            .or_default();
        for obj in input.objects {
            let kind = obj.get("__kind").and_then(|v| v.as_str());
            if is_synthesised(obj) || kind == Some("__ParsingError") {
                continue;
            }
            if let Some(id) = obj.get("__id").and_then(|v| v.as_str()) {
                ids.insert(id.to_string());
            }
        }
    }

    let mut order: Vec<&SyntheticIdInput> = inputs.iter().collect();
    order.sort_by(|a, b| a.file_path.as_bytes().cmp(b.file_path.as_bytes()));

    let mut assigned = HashMap::new();
    for input in order {
        let synthesised: Vec<(bool, &str)> = input
            .objects
            .iter()
            .filter(|obj| is_synthesised(obj))
            .map(|obj| {
                let is_doc = obj.get("__kind").and_then(|v| v.as_str()) == Some("__Document");
                (
                    is_doc,
                    obj.get("__id").and_then(|v| v.as_str()).unwrap_or(""),
                )
            })
            .collect();
        if synthesised.is_empty() {
            continue;
        }
        let mut base = synthetic_id_stem(input.file_path, input.namespace_dir);
        if base.is_empty() {
            base = "file".to_string();
        }
        let ids = taken
            .entry(input.namespace.unwrap_or("").to_string())
            .or_default();
        let mut k = 0usize;
        let renames: HashMap<String, String> = loop {
            let stem = if k == 0 {
                base.clone()
            } else {
                format!("{}_{}", base, k)
            };
            let candidate: HashMap<String, String> = synthesised
                .iter()
                .map(|(is_doc, id)| {
                    let new_id = if *is_doc {
                        format!("doc_{}", stem)
                    } else {
                        // `text_<n>` keeps its ordinal: a file can hold several text blocks.
                        let ordinal = id.strip_prefix("text_").unwrap_or(id);
                        format!("text_{}_{}", stem, ordinal)
                    };
                    (id.to_string(), new_id)
                })
                .collect();
            if candidate.values().all(|new_id| !ids.contains(new_id)) {
                break candidate;
            }
            k += 1;
        };
        ids.extend(renames.values().cloned());
        assigned.insert(input.file_path.to_string(), renames);
    }
    assigned
}

/// Apply the file's synthesised-id renames to one object: its own id, the `content` array a
/// `__Document` uses to list what it holds, and the `__container` every sibling points back with.
/// Author-written references are deliberately NOT rewritten — a reference to an id that no longer
/// exists has to surface as a broken link rather than be silently retargeted.
fn rename_synthetic_ids(obj: &mut Value, renames: &HashMap<String, String>) {
    if renames.is_empty() {
        return;
    }
    let map = match obj {
        Value::Object(m) => m,
        _ => return,
    };
    if let Some(new_id) = map
        .get("__id")
        .and_then(|v| v.as_str())
        .and_then(|id| renames.get(id))
    {
        let new_id = new_id.clone();
        map.insert("__id".to_string(), json!(new_id));
    }
    if let Some(container) = map.get("__container").and_then(|v| v.as_str()) {
        if let Some(new_ref) = rename_reference(container, renames) {
            map.insert("__container".to_string(), json!(new_ref));
        }
    }
    if let Some(Value::Array(items)) = map.get_mut("content") {
        for item in items.iter_mut() {
            if let Some(new_ref) = item.as_str().and_then(|s| rename_reference(s, renames)) {
                *item = json!(new_ref);
            }
        }
    }
}

/// Rewrite a `[[#id]]` reference when its target was renamed.
fn rename_reference(raw: &str, renames: &HashMap<String, String>) -> Option<String> {
    let inner = raw.strip_prefix("[[#")?.strip_suffix("]]")?;
    renames.get(inner).map(|new| format!("[[#{}]]", new))
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

/// The `nested_workspace` report's wording, shared by every emitter.
///
/// Nesting is reported because the outer scan leaves the inner workspace's files out, so they
/// are missing from the graph the caller just asked for — not because the two may never coexist.
/// They may: the container form composes both and drops this report (see
/// [`compose_workspace_roots`]), which is how a repository whose root is a workspace and which
/// also holds a `.qmdc` model workspace is validated. The message therefore states the
/// consequence and the remedy instead of claiming nesting is forbidden, which sent a user
/// looking for a defect in a layout the tooling itself creates (QMD-76).
///
/// `nested_rel_dir` is the inner workspace's directory relative to the outer root, so the
/// remedy is copy-pasteable and carries no absolute path.
fn nested_workspace_message(nested_id: &str, nested_rel_dir: &str) -> String {
    format!(
        "Nested workspace '{}' inside this workspace: its files are excluded from this graph. \
         Validate both together: --with <root> --with <root>/{}",
        nested_id, nested_rel_dir
    )
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
                let rel_dir = nested_root
                    .strip_prefix(&root)
                    .map(path_to_slash)
                    .unwrap_or_default();

                errors.push(WorkspaceError {
                    error_type: "nested_workspace".to_string(),
                    message: nested_workspace_message(ws_id, &rel_dir),
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
            if !is_ignored(&pf.full_path, &root, &ignore_set, false) {
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

    // Resolve namespace for any directory, with memoization. Returns the namespace id and the
    // directory that declared it — the second is what a synthesised id is made relative to.
    let mut ns_cache: HashMap<String, Option<(String, String)>> = HashMap::new();
    let mut resolve_namespace_for_dir = |dir: &str| -> Option<(String, String)> {
        if let Some(v) = ns_cache.get(dir) {
            return v.clone();
        }
        let mut check_dir = dir.to_string();
        loop {
            if let Some(ns) = namespace_map.get(&check_dir) {
                let v = Some((ns.clone(), check_dir.clone()));
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
    let ns_by_file: Vec<Option<(String, String)>> = parsed_files
        .iter()
        .map(|pf| resolve_namespace_for_dir(&pf.file_dir))
        .collect();
    let synthetic_inputs: Vec<SyntheticIdInput> = parsed_files
        .iter()
        .zip(&ns_by_file)
        .map(|(pf, ns)| SyntheticIdInput {
            file_path: &pf.file_path,
            namespace: ns.as_ref().map(|(id, _)| id.as_str()),
            namespace_dir: ns.as_ref().map(|(_, dir)| dir.as_str()),
            objects: &pf.objects,
        })
        .collect();
    let synthetic_ids = assign_synthetic_ids(&synthetic_inputs);
    let no_renames: HashMap<String, String> = HashMap::new();
    let mut all_objects: Vec<Value> = Vec::new();
    for (pf, ns_resolved) in parsed_files.iter().zip(&ns_by_file) {
        let namespace_id = ns_resolved.as_ref().map(|(ns, _)| ns.clone());
        let synthetic_renames = synthetic_ids.get(&pf.file_path).unwrap_or(&no_renames);
        for mut obj in pf.objects.clone() {
            rename_synthetic_ids(&mut obj, synthetic_renames);
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
        root: Some(canonical_slash(&root)),
        workspaces: single_entry(&workspace_id, &root),
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

    // `filter_entry` prunes descent into ignored dirs; the result is unchanged, because
    // `is_ignored` walks every ancestor of a path (QMD-73).
    for entry in WalkDir::new(root_path)
        .into_iter()
        .filter_entry(|e| !is_ignored(e.path(), root_path, &ignore_set, e.file_type().is_dir()))
        .filter_map(|e| e.ok())
    {
        let path = entry.path();

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

/// The `__Workspace` id a directory's `readme.qmd.md` declares, if any.
///
/// A cheap marker read, not a parse: used to decide whether a set of candidate workspaces
/// can be composed at all (QMD-72), on a path where parsing every candidate would be
/// wasteful.
pub fn workspace_id_of(dir: &Path) -> Option<String> {
    let readme = dir.join("readme.qmd.md");
    let content = fs::read_to_string(readme).ok()?;
    for line in content.lines() {
        if let Some(open) = line.find("[[") {
            let rest = &line[open + 2..];
            if let Some(close) = rest.find("]]") {
                let inner = &rest[..close];
                if let Some((id, kind)) = inner.split_once(':') {
                    if kind.trim() == "__Workspace" {
                        return Some(id.trim().to_string());
                    }
                }
            }
        }
    }
    None
}

/// Whether a set of workspace roots can be composed into one graph (QMD-72).
///
/// Two workspaces carrying the same id cannot: every id in one would collide with the
/// other's, so no reference could resolve to a single object. That — not "there are several
/// workspaces" — is what makes a path genuinely unanswerable.
pub fn colliding_workspace_id(roots: &[PathBuf]) -> Option<String> {
    let mut seen: Vec<String> = Vec::new();
    for r in roots {
        if let Some(id) = workspace_id_of(r) {
            if seen.contains(&id) {
                return Some(id);
            }
            seen.push(id);
        }
    }
    None
}

/// Where [`compose_workspace_roots`] puts each workspace's files in the result (QMD-72).
#[derive(Debug, Clone, Copy)]
pub enum Mount<'a> {
    /// Relative to a real directory that contains every root: the container form.
    Under(&'a Path),
    /// At the workspace's own id, in a virtual base: the `-w` form. Its roots need not share
    /// any directory, so a real common ancestor can degenerate to `/` and would put the
    /// host's own directory names into `__file`. The id depends only on the content, so the
    /// same repositories give the same `__file` wherever they are checked out; ids are
    /// distinct within a composable set, so the values stay unique.
    ById,
}

/// What [`compose_workspace_roots`] produces.
pub struct Composition {
    pub objects: Vec<Value>,
    pub files: Vec<String>,
    /// The real location of each entry of `files`, index-aligned. `files` is relative to the
    /// result's base, which is virtual under [`Mount::ById`] and so cannot be joined.
    pub file_paths: Vec<PathBuf>,
    pub errors: Vec<WorkspaceError>,
    /// One entry per composed root, ordered by `path`.
    pub workspaces: Vec<WorkspaceEntry>,
}

/// Compose an explicit set of workspace ROOTS into one graph (QMD-72).
///
/// This is the one composition primitive: every surface that composes workspaces — the
/// container form below, the CLI's `-w` / `--with` form, and the MCP/LSP seam — reaches
/// composition through it, so none of them can resolve references differently from the
/// others. Each root must already be a single workspace; discovery is the caller's job.
///
/// `mount` decides where each workspace's files appear in `__file` (see [`Mount`]). Findings
/// from the reference resolver are dropped per workspace and recomputed once over the
/// composed object set, because a workspace validated in isolation cannot see its siblings'
/// objects.
pub fn compose_workspace_roots(
    roots: &[PathBuf],
    mount: Mount,
    format: OutputFormat,
) -> Composition {
    let mut out = Composition {
        objects: Vec::new(),
        files: Vec::new(),
        file_paths: Vec::new(),
        errors: Vec::new(),
        workspaces: Vec::new(),
    };

    // A workspace inside another one is reported as `nested_workspace` because its files are
    // then missing from the outer workspace's graph. When the inner one is itself a member of
    // this set, nothing is missing: the outer scan already leaves its files out, and they are
    // composed under the inner workspace. The report would contradict the composition the
    // caller asked for (`-w repo -w repo/.qmdc`), so it is dropped — matched by path, not by
    // id, since the container form does not refuse two members sharing an id (QMD-72).
    let canon = |p: &Path| fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    let members: Vec<PathBuf> = roots.iter().map(|r| canon(r)).collect();

    for ws_dir in roots {
        let ws_result = parse_workspace(ws_dir, format);

        // Where this workspace sits in the base. `None` only when a container member is
        // somehow not under the container, which discovery never produces; its values are
        // then left as they came, the historical behaviour.
        let prefix: Option<String> = match mount {
            Mount::Under(base) => ws_dir.strip_prefix(base).ok().map(path_to_slash),
            Mount::ById => ws_result.workspace_id.clone(),
        };
        let relocate = |rel: &str| -> Option<String> {
            prefix.as_ref().map(|p| {
                if p.is_empty() {
                    rel.to_string()
                } else {
                    format!("{}/{}", p, rel)
                }
            })
        };

        if let (Some(id), Some(p)) = (&ws_result.workspace_id, &prefix) {
            out.workspaces.push(WorkspaceEntry {
                id: id.clone(),
                root: canonical_slash(ws_dir),
                path: p.clone(),
            });
        }

        for mut obj in ws_result.objects {
            // Skip __ParsingError objects - they are handled separately
            let kind = obj.get("__kind").and_then(|v| v.as_str()).unwrap_or("");
            if kind == "__ParsingError" {
                continue;
            }
            if let Some(obj_map) = obj.as_object_mut() {
                if let Some(file) = obj_map.get("__file").and_then(|v| v.as_str()) {
                    if let Some(moved) = relocate(file) {
                        obj_map.insert("__file".to_string(), json!(moved));
                    }
                }
            }
            out.objects.push(obj);
        }

        for file in ws_result.files {
            if let Some(moved) = relocate(&file) {
                out.file_paths.push(ws_dir.join(&file));
                out.files.push(moved);
            }
        }

        // QMD-69: reference findings are dropped here and recomputed once over the
        // composed object set — in isolation this workspace could not see its
        // siblings' objects, so any cross-workspace reference looked broken.
        for mut error in ws_result.errors {
            if is_reference_finding(&error.error_type) {
                continue;
            }
            if error.error_type == "nested_workspace" {
                let inner = error
                    .file
                    .as_deref()
                    .and_then(|f| ws_dir.join(f).parent().map(&canon));
                if inner.is_some_and(|p| members.contains(&p)) {
                    continue;
                }
            }
            if let Some(ref file) = error.file {
                if let Some(moved) = relocate(file) {
                    error.file = Some(moved);
                }
            }
            out.errors.push(error);
        }
    }

    out.workspaces
        .sort_by(|a, b| a.path.cmp(&b.path).then_with(|| a.id.cmp(&b.id)));
    out
}

/// Re-run reference validation over a COMPOSED object set (QMD-69).
///
/// Each workspace was parsed and validated in isolation, so its reference findings were
/// computed against an index that could not see the other workspaces' objects: a
/// workspace-qualified cross-workspace reference was therefore always reported broken. The
/// stale findings are dropped by [`compose_workspace_roots`] and the single shared engine
/// runs once here over every object in the composed set.
///
/// `files` are the result's `__file` values and `file_paths` their real locations,
/// index-aligned: the scanner reads a file's lines by the `__file` of the object that
/// referred, and under [`Mount::ById`] that value cannot be joined onto any directory.
///
/// Structural findings (duplicate_id, workspace_in_wrong_file, parsing errors) stay
/// per-workspace: they are scoped to one workspace by definition, and identity is
/// workspace-scoped (QMD-67), so a composed set must not report a duplicate across members.
pub fn rescan_composed_references(
    objects: &[Value],
    files: &[String],
    file_paths: &[PathBuf],
    errors: &mut Vec<WorkspaceError>,
) {
    let mut file_content_cache: HashMap<String, Vec<String>> = HashMap::new();
    for (rel_file, real) in files.iter().zip(file_paths) {
        if let Ok(content) = fs::read_to_string(real) {
            file_content_cache.insert(
                rel_file.clone(),
                content.lines().map(String::from).collect(),
            );
        }
    }
    for f in
        crate::core::reference_scan::reference_scan(objects, objects, Some(&file_content_cache))
    {
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
    if root_readme.exists() && !is_ignored(&root_readme, root_path, &ignore_set, false) {
        if let Ok(content) = fs::read_to_string(&root_readme) {
            if content_has_workspace_marker(&content) {
                // Root is a workspace - use single workspace parsing
                return parse_workspace(root_path, format);
            }
        }
    }

    // Root is not a workspace - find all workspaces in subdirectories. `find_all_workspace_dirs`
    // already drops a marker the root's `.qmdcignore` hides, using the same root and the same
    // rules, so there is nothing left for a second filter to remove (QMD-73).
    let workspace_dirs: Vec<PathBuf> = find_all_workspace_dirs(root_path);

    if workspace_dirs.is_empty() {
        // No explicit workspaces found - check if root has .qmd.md files
        // If yes, treat root as a virtual workspace
        // IMPORTANT: Must respect .qmdcignore when checking for files. `filter_entry` prunes
        // descent into ignored dirs; the result is unchanged, because `is_ignored` walks every
        // ancestor of a path (QMD-73).
        let has_qmdc_files = WalkDir::new(root_path)
            .max_depth(5)
            .into_iter()
            .filter_entry(|e| !is_ignored(e.path(), root_path, &ignore_set, e.file_type().is_dir()))
            .filter_map(|e| e.ok())
            .any(|e| {
                let path = e.path();
                path.extension().map(|ext| ext == "md").unwrap_or(false)
                    && path.to_string_lossy().contains(".qmd.")
            });

        if has_qmdc_files {
            // Treat root as a virtual workspace
            return parse_workspace(root_path, format);
        }

        // No workspaces and no QMD.md files - return empty result
        return WorkspaceResult {
            root: Some(canonical_slash(root_path)),
            workspace_id: None,
            workspaces: vec![],
            files: vec![],
            objects: vec![],
            errors: vec![],
        };
    }

    // Parse each workspace and combine results — through the one composition primitive.
    let Composition {
        objects: mut all_objects,
        files: mut all_files,
        file_paths: mut all_file_paths,
        errors: mut all_errors,
        workspaces,
    } = compose_workspace_roots(&workspace_dirs, Mount::Under(root_path), format);

    // After parsing explicit workspaces, check for orphan .qmd.md files
    // (files outside any workspace directory that should be loaded too)
    let mut orphan_files = Vec::new();
    // `filter_entry` prunes descent into ignored dirs; the result is unchanged, because
    // `is_ignored` walks every ancestor of a path (QMD-73).
    for entry in WalkDir::new(root_path)
        .max_depth(5)
        .into_iter()
        .filter_entry(|e| !is_ignored(e.path(), root_path, &ignore_set, e.file_type().is_dir()))
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if path.extension().map(|e| e == "md").unwrap_or(false)
            && path.to_string_lossy().contains(".qmd.")
        {
            // Exclude files inside explicit workspace directories
            if !workspace_dirs.iter().any(|ws_dir| path.starts_with(ws_dir)) {
                orphan_files.push(path.to_path_buf());
            }
        }
    }
    // The same order as a workspace's own files (see `compare_workspace_files`).
    let orphan_rel = |p: &PathBuf| {
        p.strip_prefix(root_path)
            .map(path_to_slash)
            .unwrap_or_else(|_| path_to_slash(p))
    };
    orphan_files.sort_by(|a, b| compare_workspace_files(&orphan_rel(a), &orphan_rel(b)));

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

        // Parse every orphan first: a synthesised id is only unique against all of them (QMD-77).
        let mut orphan_parsed: Vec<(PathBuf, String, String, Vec<Value>)> = Vec::new();
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
                orphan_parsed.push((file_path, rel_file, content, objects));
            }
        }
        let orphan_inputs: Vec<SyntheticIdInput> = orphan_parsed
            .iter()
            .map(|(_, rel_file, _, objects)| SyntheticIdInput {
                file_path: rel_file,
                namespace: None,
                namespace_dir: None,
                objects,
            })
            .collect();
        let orphan_ids = assign_synthetic_ids(&orphan_inputs);
        drop(orphan_inputs);
        let no_renames: HashMap<String, String> = HashMap::new();

        for (file_path, rel_file, content, objects) in orphan_parsed {
            let renames = orphan_ids.get(&rel_file).unwrap_or(&no_renames);

            // Check if this is a readme file
            let is_readme = file_path
                .file_name()
                .and_then(|n| n.to_str())
                .map(|n| n == "readme.qmd.md")
                .unwrap_or(false);

            // Add __file metadata to each object
            for mut obj in objects {
                rename_synthetic_ids(&mut obj, renames);
                // Skip __Workspace objects from non-readme files
                if !is_readme {
                    if let Some(kind) = obj.get("__kind").and_then(|v| v.as_str()) {
                        if kind == "__Workspace" {
                            // Add error for this invalid workspace (but skip if file is ignored)
                            if !is_ignored(&file_path, root_path, &ignore_set, false) {
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

            all_file_paths.push(file_path.clone());
            all_files.push(rel_file);
        }
    }

    // QMD-69: re-run reference validation over the COMPOSED object set. See
    // `rescan_composed_references` for why the per-workspace findings cannot be reused.
    rescan_composed_references(&all_objects, &all_files, &all_file_paths, &mut all_errors);

    WorkspaceResult {
        root: Some(canonical_slash(root_path)),
        workspace_id: None, // Multiple workspaces, no single ID
        workspaces,
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

/// Resolve one `-w` / `--with` path to exactly one workspace root (QMD-72).
///
/// A `--with` path names a workspace, not a container: zero and several are both usage
/// errors, because the caller asked to compose a specific workspace and the tool must not
/// guess which one was meant. Checks the path itself first, then a bounded downward scan.
fn resolve_single_workspace_root(path: &Path) -> Result<PathBuf, String> {
    if !path.exists() {
        return Err(format!("--with path does not exist: {}", path.display()));
    }
    if dir_is_workspace_root(path) {
        return Ok(path.to_path_buf());
    }
    let below = find_nested_workspace_roots_bounded(path, WORKSPACE_SCAN_MAX_DEPTH);
    // Keep only top-level roots so nested ones do not inflate the count.
    let mut top: Vec<PathBuf> = Vec::new();
    for r in below {
        if !top.iter().any(|kept| r.starts_with(kept)) {
            top.push(r);
        }
    }
    match top.len() {
        1 => Ok(top.into_iter().next().unwrap()),
        0 => Err(format!(
            "--with path is not a workspace: {} (no readme.qmd.md declaring [[id: __Workspace]])",
            path.display()
        )),
        n => Err(format!(
            "--with path contains {} workspaces: {} (pass each one as its own --with)",
            n,
            path.display()
        )),
    }
}

/// Compose the workspaces named by repeated `-w` / `--with` (QMD-72).
///
/// Every path is a peer — the first is not primary — and each must resolve to exactly one
/// workspace. Returns a usage error rather than a wrong answer for the shapes that cannot
/// mean anything: a path that is not a workspace, a path holding several, the same path
/// twice, and two paths carrying the same workspace id (which would make an id ambiguous
/// and so could not be composed into one graph).
pub fn compose_with_paths(
    paths: &[PathBuf],
    format: OutputFormat,
) -> Result<WorkspaceResult, String> {
    let mut roots: Vec<PathBuf> = Vec::new();
    for p in paths {
        let root = resolve_single_workspace_root(p)?;
        let canon = fs::canonicalize(&root).unwrap_or_else(|_| root.clone());
        if roots
            .iter()
            .any(|r| fs::canonicalize(r).unwrap_or_else(|_| r.clone()) == canon)
        {
            return Err(format!("--with path given twice: {}", p.display()));
        }
        roots.push(root);
    }

    let Composition {
        objects,
        files,
        file_paths,
        mut errors,
        workspaces,
    } = compose_workspace_roots(&roots, Mount::ById, format);

    // Two workspaces carrying the same id cannot be composed: every id in one would collide
    // with the other's, so no reference could be resolved to a single object. Under
    // `Mount::ById` their files would also land under one `__file` prefix.
    let mut seen_ids: Vec<String> = Vec::new();
    for obj in &objects {
        if obj.get("__kind").and_then(|v| v.as_str()) == Some("__Workspace") {
            let id = obj
                .get("__id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if !id.is_empty() {
                if seen_ids.contains(&id) {
                    return Err(format!(
                        "--with paths declare the same workspace id '{}'; ids must be distinct to compose",
                        id
                    ));
                }
                seen_ids.push(id);
            }
        }
    }

    rescan_composed_references(&objects, &files, &file_paths, &mut errors);

    Ok(WorkspaceResult {
        root: None,         // The base is virtual: each workspace sits at its own id
        workspace_id: None, // A composed set has no single workspace id
        workspaces,
        files,
        objects,
        errors,
    })
}

/// Entry point for a workspace-aware CLI command (QMD-72).
///
/// Accepts either the historical positional path or one or more `-w` / `--with` paths, and
/// refuses both at once: they answer different questions ("what is near this path" versus
/// "which workspaces make up this project"), so silently preferring one would give the
/// caller an answer to a question they did not ask.
pub fn resolve_workspace_input(
    path: Option<&Path>,
    with: &[PathBuf],
    format: OutputFormat,
) -> Result<WorkspaceResult, String> {
    match (path, with.is_empty()) {
        (Some(_), false) => Err(
            "a positional PATH and --with are mutually exclusive; pass every workspace as --with"
                .to_string(),
        ),
        (_, false) => compose_with_paths(with, format),
        (Some(p), true) => Ok(resolve_workspace(p, format)),
        (None, true) => Ok(resolve_workspace(Path::new("."), format)),
    }
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
