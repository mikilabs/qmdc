//! Index seam — workspace root resolution and index materialisation.
//!
//! Provides three entry points:
//! - [`resolve_root`]: bounded upward walk to find the nearest enclosing QMDC workspace root.
//! - [`resolve_root_bidirectional`]: down-first (then up) resolver used by the MCP server
//!   (QMD-63); composes a container of sibling workspaces (QMD-72) and returns `ambiguous`
//!   only when the set cannot be composed because two members share a workspace id.
//! - [`get_index`]: reparse + DB sync to produce a [`ResolvedIndex`].
//!
//! Invariants enforced:
//! - **INV-1** path containment: [`assert_within_root`] fails-closed with `out-of-root`.
//! - **NFR-2** bounded reparse: [`get_index`] refuses if file count exceeds the bound.
//!
//! No `lsp_types`. Pure `PathBuf`/`&Path` interfaces.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Instant;

use regex::Regex;
use serde_json::Value;

use crate::db::QmdcDatabase;
use crate::parser::OutputFormat;
use crate::workspace::{
    colliding_workspace_id, dir_is_workspace_root, find_nested_workspace_roots_bounded,
    find_workspace_root, parse_all_workspaces, WORKSPACE_SCAN_MAX_DEPTH,
};

use super::error::{ErrorCode, ErrorEnvelope};
use super::log::{core_log, EventCategory, Severity};
use super::resolved_index::ResolvedIndex;

// ---------------------------------------------------------------------------
// Configuration constants
// ---------------------------------------------------------------------------

/// Maximum number of parent directories to walk upward when resolving root.
const MAX_UPWARD_WALK: usize = 64;

/// Maximum number of files allowed in a single reparse pass (NFR-2).
/// Exceeding this triggers `reparse-bound-exceeded`.
pub const REPARSE_FILE_BOUND: usize = 10_000;

/// `[[…:__Workspace]]` marker regex — compiled once (hot path: every `resolve_root`).
fn workspace_marker_re() -> &'static Regex {
    static RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\[\[[^\]]+:\s*__Workspace\]\]").unwrap())
}

/// `[[…:__Namespace]]` marker regex — compiled once.
fn namespace_marker_re() -> &'static Regex {
    static RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\[\[[^\]]+:\s*__Namespace\]\]").unwrap())
}

// ---------------------------------------------------------------------------
// resolve_root — bounded upward walk
// ---------------------------------------------------------------------------

/// Resolve the nearest enclosing QMDC workspace root for a given path.
///
/// Walks upward from `path` (or its parent if `path` is a file) looking for a directory
/// containing `readme.qmd.md` that declares `[[…:__Workspace]]` or `[[…:__Namespace]]`.
/// Stops at:
/// - First match (innermost wins)
/// - A `.git` directory (project/checkout boundary)
/// - Filesystem root
/// - `MAX_UPWARD_WALK` steps
///
/// Returns `Err(Value)` with `ErrorCode::NotResolved` on failure.
pub fn resolve_root(path: &Path) -> Result<PathBuf, Value> {
    let start = if path.is_file() {
        path.parent().unwrap_or(path)
    } else {
        path
    };

    // Canonicalize the start to resolve symlinks and get absolute path.
    let start = start.canonicalize().map_err(|e| {
        core_log(
            EventCategory::Resolution,
            Severity::Warning,
            &format!("cannot canonicalize '{}': {}", path.display(), e),
        );
        ErrorEnvelope::error(
            ErrorCode::NotResolved,
            format!(
                "path does not exist or is not accessible: {}",
                path.display()
            ),
        )
    })?;

    let workspace_re = workspace_marker_re();
    let namespace_re = namespace_marker_re();

    let mut current = start.as_path();
    let mut steps = 0;

    loop {
        if steps >= MAX_UPWARD_WALK {
            core_log(
                EventCategory::Resolution,
                Severity::Warning,
                &format!(
                    "upward walk exhausted ({} steps) from '{}'",
                    MAX_UPWARD_WALK,
                    path.display()
                ),
            );
            return Err(ErrorEnvelope::error(
                ErrorCode::NotResolved,
                format!(
                    "no workspace root found within {} levels of '{}'",
                    MAX_UPWARD_WALK,
                    path.display()
                ),
            ));
        }

        // Check for readme.qmd.md declaring __Workspace or __Namespace
        let readme = current.join("readme.qmd.md");
        if readme.is_file() {
            if let Ok(content) = std::fs::read_to_string(&readme) {
                if workspace_re.is_match(&content) || namespace_re.is_match(&content) {
                    core_log(
                        EventCategory::Resolution,
                        Severity::Info,
                        &format!("resolved root: '{}'", current.display()),
                    );
                    return Ok(current.to_path_buf());
                }
            }
        }

        // Stop at .git boundary (project/checkout root)
        if current.join(".git").exists() {
            core_log(
                EventCategory::Resolution,
                Severity::Info,
                &format!(
                    "hit .git boundary at '{}', no workspace found",
                    current.display()
                ),
            );
            return Err(ErrorEnvelope::error(
                ErrorCode::NotResolved,
                format!(
                    "no workspace root found (hit .git boundary at '{}')",
                    current.display()
                ),
            ));
        }

        // Move to parent
        match current.parent() {
            Some(parent) if parent != current => {
                current = parent;
                steps += 1;
            }
            _ => {
                // Reached filesystem root
                core_log(
                    EventCategory::Resolution,
                    Severity::Warning,
                    &format!(
                        "reached filesystem root without finding workspace for '{}'",
                        path.display()
                    ),
                );
                return Err(ErrorEnvelope::error(
                    ErrorCode::NotResolved,
                    format!(
                        "no workspace root found (reached filesystem root) for '{}'",
                        path.display()
                    ),
                ));
            }
        }
    }
}

// ---------------------------------------------------------------------------
// resolve_root_bidirectional — down-first, then up (QMD-63)
// ---------------------------------------------------------------------------

/// Resolve the workspace root for an MCP `path`, searching **down first, then up**
/// (QMD-63). Reuses the workspace-discovery primitives (`dir_is_workspace_root`,
/// `find_nested_workspace_roots_bounded`, `find_workspace_root`) that the CLI resolver
/// (`resolve_workspace`) also builds on — no MCP-specific discovery logic.
///
/// 1. If `path` (or its parent, when a file) is itself a workspace root → it.
/// 2. Else discover `__Workspace` roots below it:
///    - exactly one → that root,
///    - more than one → the CONTAINER, which `get_index` composes (QMD-72) — unless two
///      candidates declare the same workspace id, which is the one case a composed graph
///      cannot represent and so still returns [`ErrorCode::Ambiguous`] with the candidates.
/// 3. Else walk upward to the enclosing workspace (`find_workspace_root`,
///    `__Workspace`-only, matching the QMD-59 walk-up contract). Note this walk
///    intentionally does NOT stop at a `.git` boundary — removing that false
///    boundary is the point of QMD-63; when a `force_root` is set the MCP caller
///    still bounds the result via `enforce_force_root`.
/// 4. Else [`ErrorCode::NotResolved`].
pub fn resolve_root_bidirectional(path: &Path) -> Result<PathBuf, Value> {
    let start = if path.is_file() {
        path.parent().unwrap_or(path)
    } else {
        path
    };

    let start = start.canonicalize().map_err(|e| {
        core_log(
            EventCategory::Resolution,
            Severity::Warning,
            &format!("cannot canonicalize '{}': {}", path.display(), e),
        );
        ErrorEnvelope::error(
            ErrorCode::NotResolved,
            format!(
                "path does not exist or is not accessible: {}",
                path.display()
            ),
        )
    })?;

    // 1. The path itself is a workspace root.
    if dir_is_workspace_root(&start) {
        core_log(
            EventCategory::Resolution,
            Severity::Info,
            &format!("resolved root (self): '{}'", start.display()),
        );
        return Ok(start);
    }

    // 2. Down: workspaces below `start` (bounded to WORKSPACE_SCAN_MAX_DEPTH levels).
    let below = find_nested_workspace_roots_bounded(&start, WORKSPACE_SCAN_MAX_DEPTH);
    // Keep only top-level roots. Nested workspaces are illegal; if any slipped in,
    // treat the outermost as the workspace so candidates stay disjoint siblings.
    // `below` is sorted shortest-first, so a prefix check against kept roots works.
    let mut top: Vec<PathBuf> = Vec::new();
    for r in below {
        if !top.iter().any(|kept| r.starts_with(kept)) {
            top.push(r);
        }
    }
    match top.len() {
        1 => {
            let root = top.into_iter().next().unwrap();
            core_log(
                EventCategory::Resolution,
                Severity::Info,
                &format!("resolved root (down): '{}'", root.display()),
            );
            return Ok(root);
        }
        n if n > 1 => {
            // QMD-72: a container of sibling workspaces is COMPOSED, not refused. The CLI
            // has always composed one; refusing it here was the whole MCP/CLI split — and
            // neither candidate alone can see the other's objects, so picking one could not
            // answer the question either. `get_index` composes via `parse_all_workspaces`,
            // so returning the container is all that is needed.
            //
            // `ambiguous` survives with a sharper meaning: a set that cannot be composed
            // because two members carry the same workspace id, where every id in one would
            // collide with the other's and no reference could resolve to a single object.
            if let Some(dup) = colliding_workspace_id(&top) {
                let candidates: Vec<String> = top.iter().map(|p| p.display().to_string()).collect();
                core_log(
                    EventCategory::Resolution,
                    Severity::Info,
                    &format!(
                        "ambiguous: {} workspaces under '{}' share id '{}'",
                        n,
                        start.display(),
                        dup
                    ),
                );
                return Err(ErrorEnvelope::error_with_candidates(
                    ErrorCode::Ambiguous,
                    format!(
                        "path '{}' contains {} workspaces that cannot be composed: id '{}' is \
                         declared more than once; re-call with one of `candidates` as `path`",
                        start.display(),
                        n,
                        dup
                    ),
                    candidates,
                ));
            }
            core_log(
                EventCategory::Resolution,
                Severity::Info,
                &format!(
                    "resolved root (down, composing {} workspaces): '{}'",
                    n,
                    start.display()
                ),
            );
            return Ok(start);
        }
        _ => {}
    }

    // 3. Up: nearest enclosing workspace (skips namespaces, QMD-59 contract).
    if let Some(root) = find_workspace_root(&start) {
        core_log(
            EventCategory::Resolution,
            Severity::Info,
            &format!("resolved root (up): '{}'", root.display()),
        );
        return Ok(root);
    }

    // 4. Nothing at, below, or above.
    Err(ErrorEnvelope::error(
        ErrorCode::NotResolved,
        format!(
            "no workspace found at '{}', within {} levels below it, or in any ancestor",
            path.display(),
            WORKSPACE_SCAN_MAX_DEPTH
        ),
    ))
}

// ---------------------------------------------------------------------------
// get_index — reparse + DB sync
// ---------------------------------------------------------------------------

/// Materialise a [`ResolvedIndex`] for the given resolved root.
///
/// 1. Runs `parse_all_workspaces(root, OutputFormat::Full)` to get parsed objects.
/// 2. Checks NFR-2 file-count bound; fails with `reparse-bound-exceeded` if exceeded.
/// 3. Creates an in-memory `QmdcDatabase` and syncs objects into it.
///
/// Returns `Err(Value)` with the appropriate error envelope on failure.
pub fn get_index(root: &Path) -> Result<ResolvedIndex, Value> {
    build_index(root, REPARSE_FILE_BOUND)
}

/// Like [`get_index`] but with a configurable file-count bound (for testing NFR-2).
pub fn get_index_with_bound(root: &Path, max_files: usize) -> Result<ResolvedIndex, Value> {
    build_index(root, max_files)
}

/// Shared implementation for [`get_index`] / [`get_index_with_bound`].
///
/// Single source of the reparse → NFR-2 bound check → in-memory DB sync pipeline.
fn build_index(root: &Path, max_files: usize) -> Result<ResolvedIndex, Value> {
    let canon_root = root.canonicalize().map_err(|e| {
        ErrorEnvelope::error(
            ErrorCode::NotResolved,
            format!("cannot access root '{}': {}", root.display(), e),
        )
    })?;

    let ws_result = parse_all_workspaces(&canon_root, OutputFormat::Full);
    index_from_result(canon_root, ws_result, max_files)
}

// ---------------------------------------------------------------------------
// Explicit composition (`qmdc mcp -w`, GitHub #10)
// ---------------------------------------------------------------------------

/// The `-w` paths the MCP server was started with. When set, every tool answers over the
/// composition of exactly these workspaces — the same graph `qmdc query -w …` sees.
static COMPOSE_WITH: OnceLock<Vec<PathBuf>> = OnceLock::new();

/// Check `paths` the way `qmdc query -w` does (each one exactly one workspace, no path or
/// workspace id twice) and, when a force-root is configured, that every path and every
/// workspace it resolves to lies inside it. Returns the usage-error message on refusal.
pub fn check_compose_with(paths: &[PathBuf]) -> Result<(), String> {
    for p in paths {
        if let Err(e) = enforce_force_root(p) {
            return Err(force_root_refusal(p, &e));
        }
    }
    let composed = crate::workspace::compose_with_paths(paths, OutputFormat::Minimal)?;
    for w in &composed.workspaces {
        let root = Path::new(&w.root);
        if let Err(e) = enforce_force_root(root) {
            return Err(force_root_refusal(root, &e));
        }
    }
    Ok(())
}

fn force_root_refusal(p: &Path, e: &Value) -> String {
    let why = e
        .pointer("/error/message")
        .and_then(|m| m.as_str())
        .unwrap_or("outside the force-root");
    format!(
        "--with path {} is outside --force-root: {}",
        p.display(),
        why
    )
}

/// Configure the server-wide `-w` set. Set once at startup, after [`check_compose_with`].
pub fn set_compose_with(paths: Vec<PathBuf>) {
    let _ = COMPOSE_WITH.set(paths);
}

/// The configured `-w` set, if any.
pub fn compose_with() -> Option<&'static [PathBuf]> {
    COMPOSE_WITH.get().map(|v| v.as_slice())
}

/// Materialise the index of the composed `-w` set. Its base is virtual
/// ([`ResolvedIndex::is_composed`]), so `root` is empty and files are located with
/// [`ResolvedIndex::disk_path`].
pub fn get_composed_index(paths: &[PathBuf]) -> Result<ResolvedIndex, Value> {
    let ws_result = crate::workspace::compose_with_paths(paths, OutputFormat::Full)
        .map_err(|msg| ErrorEnvelope::error(ErrorCode::NotResolved, msg))?;
    index_from_result(PathBuf::new(), ws_result, REPARSE_FILE_BOUND)
}

/// The workspace root of a composed index that holds `path`, or `out-of-root`. A tool call
/// against a `-w` server still names a path, and it must be inside one of the composed
/// workspaces: anything else would be answered from a graph it is not part of.
pub fn assert_in_composition(index: &ResolvedIndex, path: &Path) -> Result<(), Value> {
    let inside = index
        .workspace
        .workspaces
        .iter()
        .any(|w| assert_within_root(Path::new(&w.root), path).is_ok());
    if inside {
        Ok(())
    } else {
        Err(ErrorEnvelope::error(
            ErrorCode::OutOfRoot,
            format!(
                "path '{}' is not inside any workspace this server was started with (-w)",
                path.display()
            ),
        ))
    }
}

/// The index an MCP tool or resource call naming `path` is answered from.
///
/// Without `-w`: the workspace found from `path` (down first, then up), with the force-root
/// checked on both the path and the root it resolves to. With `-w`: the composed set the
/// server was started with, and `path` must lie inside one of its workspaces.
pub fn index_for_path(path: &Path) -> Result<ResolvedIndex, Value> {
    enforce_force_root(path)?;
    if let Some(paths) = compose_with() {
        let index = get_composed_index(paths)?;
        assert_in_composition(&index, path)?;
        return Ok(index);
    }
    let root = resolve_root_bidirectional(path)?;
    enforce_force_root(&root)?;
    get_index(&root)
}

/// The NFR-2 bound check and in-memory DB sync shared by rooted and composed indexes.
fn index_from_result(
    canon_root: PathBuf,
    ws_result: crate::workspace::WorkspaceResult,
    max_files: usize,
) -> Result<ResolvedIndex, Value> {
    let file_count = ws_result.files.len();

    // NFR-2: bounded reparse
    if file_count > max_files {
        core_log(
            EventCategory::Resolution,
            Severity::Warning,
            &format!(
                "reparse bound exceeded: {} files > {} limit at '{}'",
                file_count,
                max_files,
                canon_root.display()
            ),
        );
        return Err(ErrorEnvelope::error(
            ErrorCode::ReparseBoundExceeded,
            format!(
                "workspace at '{}' contains {} files, exceeding the {} file reparse bound",
                canon_root.display(),
                file_count,
                max_files
            ),
        ));
    }

    // Create in-memory DB and sync objects
    let db = QmdcDatabase::new().map_err(|e| {
        ErrorEnvelope::error(
            ErrorCode::InternalError,
            format!("failed to create in-memory database: {}", e),
        )
    })?;

    db.sync_objects_from_vec(&ws_result.objects).map_err(|e| {
        ErrorEnvelope::error(
            ErrorCode::InternalError,
            format!("failed to sync objects to database: {}", e),
        )
    })?;

    core_log(
        EventCategory::Resolution,
        Severity::Info,
        &format!(
            "index built: {} files, {} objects at '{}'",
            file_count,
            ws_result.objects.len(),
            canon_root.display()
        ),
    );

    Ok(ResolvedIndex {
        root: canon_root,
        workspace: ws_result,
        db,
        built_at: Instant::now(),
        file_count,
    })
}

// ---------------------------------------------------------------------------
// INV-1: path containment assertion
// ---------------------------------------------------------------------------

/// Assert that `target` is contained within `root` (INV-1 path containment).
///
/// Both paths are canonicalized before comparison. If `target` escapes `root`,
/// logs `security-rejection` and returns `Err` with `ErrorCode::OutOfRoot`.
///
/// This is the **fail-closed** invariant: any failure to verify containment
/// (e.g., canonicalization error) is treated as a denial.
pub fn assert_within_root(root: &Path, target: &Path) -> Result<PathBuf, Value> {
    let canon_root = root.canonicalize().map_err(|e| {
        core_log(
            EventCategory::SecurityRejection,
            Severity::Security,
            &format!(
                "INV-1 denial: cannot canonicalize root '{}': {}",
                root.display(),
                e
            ),
        );
        ErrorEnvelope::error(
            ErrorCode::OutOfRoot,
            "path containment check failed: root not accessible",
        )
    })?;

    let canon_target = target.canonicalize().map_err(|e| {
        core_log(
            EventCategory::SecurityRejection,
            Severity::Security,
            &format!(
                "INV-1 denial: cannot canonicalize target '{}': {}",
                target.display(),
                e
            ),
        );
        ErrorEnvelope::error(
            ErrorCode::OutOfRoot,
            "path containment check failed: target not accessible",
        )
    })?;

    if !canon_target.starts_with(&canon_root) {
        core_log(
            EventCategory::SecurityRejection,
            Severity::Security,
            &format!(
                "INV-1 denial: '{}' escapes root '{}'",
                canon_target.display(),
                canon_root.display()
            ),
        );
        return Err(ErrorEnvelope::error(
            ErrorCode::OutOfRoot,
            format!(
                "path '{}' is outside workspace root '{}'",
                target.display(),
                root.display()
            ),
        ));
    }

    Ok(canon_target)
}

// ---------------------------------------------------------------------------
// Force-root boundary (INV-1 enforcement point)
// ---------------------------------------------------------------------------

/// Process-wide configured workspace root. When set (via `qmdc mcp --force-root <DIR>`),
/// every MCP-resolved path and workspace root must canonicalize inside it.
static FORCE_ROOT: OnceLock<PathBuf> = OnceLock::new();

/// Configure the server-wide force-root. Set once at startup; subsequent calls are ignored.
///
/// This is the production wiring for INV-1: without it, the MCP server trusts whatever
/// `path` each caller supplies (the local single-user stdio model). With it set, the seam
/// fails closed for any path outside the configured root.
pub fn set_force_root(root: PathBuf) {
    let _ = FORCE_ROOT.set(root);
}

/// The configured force-root, if any.
pub fn force_root() -> Option<&'static Path> {
    FORCE_ROOT.get().map(|p| p.as_path())
}

/// Enforce INV-1 against the configured force-root (no-op when none is configured).
///
/// When a force-root is set, `target` must canonicalize to a path contained within it,
/// failing closed (`out-of-root`) otherwise. When no force-root is set this returns
/// `Ok(())` — the caller-supplied path is trusted.
pub fn enforce_force_root(target: &Path) -> Result<(), Value> {
    match force_root() {
        Some(root) => assert_within_root(root, target).map(|_| ()),
        None => Ok(()),
    }
}
