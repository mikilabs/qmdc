//! Integration tests for `core::index_seam` — Layer 2 of Core extraction.
//!
//! Tests cover:
//! - resolve_root: nested workspace resolution, two-worktree setups
//! - INV-1 path containment: path-escape → `out-of-root`
//! - Non-existent path → `not-resolved`
//! - NFR-2 reparse bound → `reparse-bound-exceeded`

use std::fs;
use std::path::Path;

use tempfile::TempDir;

use qmdc::core::error::ErrorCode;
use qmdc::core::index_seam::{assert_within_root, get_index_with_bound, resolve_root};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Create a minimal workspace at `dir` with a readme.qmd.md declaring __Workspace.
fn create_workspace(dir: &Path, ws_id: &str) {
    fs::create_dir_all(dir).unwrap();
    let readme = dir.join("readme.qmd.md");
    let content = format!(
        "# Workspace\n\n[[{}:__Workspace]]\n\nname: {}\n",
        ws_id, ws_id
    );
    fs::write(&readme, content).unwrap();
}

/// Create a namespace at `dir` with a readme.qmd.md declaring __Namespace.
fn create_namespace(dir: &Path, ns_id: &str) {
    fs::create_dir_all(dir).unwrap();
    let readme = dir.join("readme.qmd.md");
    let content = format!(
        "# Namespace\n\n[[{}:__Namespace]]\n\nname: {}\n",
        ns_id, ns_id
    );
    fs::write(&readme, content).unwrap();
}

/// Create a .git marker directory (simulates a git repo root).
fn create_git_boundary(dir: &Path) {
    fs::create_dir_all(dir.join(".git")).unwrap();
}

/// Extract the error code string from an error envelope Value.
fn error_code(val: &serde_json::Value) -> &str {
    val["error"]["code"].as_str().unwrap_or("")
}

// ---------------------------------------------------------------------------
// resolve_root tests
// ---------------------------------------------------------------------------

#[test]
fn resolve_root_finds_workspace_in_current_dir() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    create_workspace(root, "TestWs");

    let result = resolve_root(root);
    assert!(result.is_ok(), "expected Ok, got: {:?}", result);
    // Canonicalize to match what resolve_root returns
    let expected = root.canonicalize().unwrap();
    assert_eq!(result.unwrap(), expected);
}

#[test]
fn resolve_root_finds_workspace_in_parent_dir() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    create_workspace(root, "ParentWs");

    let child = root.join("subdir").join("deep");
    fs::create_dir_all(&child).unwrap();

    let result = resolve_root(&child);
    assert!(result.is_ok(), "expected Ok, got: {:?}", result);
    let expected = root.canonicalize().unwrap();
    assert_eq!(result.unwrap(), expected);
}

#[test]
fn resolve_root_finds_innermost_nested_workspace() {
    // Structure: root/outer (workspace) / inner (workspace) / file
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    let outer = root.join("outer");
    create_workspace(&outer, "OuterWs");

    let inner = outer.join("inner");
    create_workspace(&inner, "InnerWs");

    let deep = inner.join("subdir");
    fs::create_dir_all(&deep).unwrap();

    // From deep inside inner, should resolve to inner (innermost wins)
    let result = resolve_root(&deep);
    assert!(result.is_ok(), "expected Ok, got: {:?}", result);
    let expected = inner.canonicalize().unwrap();
    assert_eq!(result.unwrap(), expected);
}

#[test]
fn resolve_root_finds_namespace_root() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    create_namespace(root, "TestNs");

    let child = root.join("sub");
    fs::create_dir_all(&child).unwrap();

    let result = resolve_root(&child);
    assert!(result.is_ok(), "expected Ok, got: {:?}", result);
    let expected = root.canonicalize().unwrap();
    assert_eq!(result.unwrap(), expected);
}

#[test]
fn resolve_root_two_worktrees_independent() {
    // Two sibling worktrees, each with their own workspace
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    // Put a .git at root to act as boundary
    create_git_boundary(root);

    let ws_a = root.join("worktree_a");
    create_workspace(&ws_a, "WsA");

    let ws_b = root.join("worktree_b");
    create_workspace(&ws_b, "WsB");

    let file_in_a = ws_a.join("docs");
    fs::create_dir_all(&file_in_a).unwrap();

    let file_in_b = ws_b.join("notes");
    fs::create_dir_all(&file_in_b).unwrap();

    let result_a = resolve_root(&file_in_a);
    assert!(result_a.is_ok());
    assert_eq!(result_a.unwrap(), ws_a.canonicalize().unwrap());

    let result_b = resolve_root(&file_in_b);
    assert!(result_b.is_ok());
    assert_eq!(result_b.unwrap(), ws_b.canonicalize().unwrap());
}

#[test]
fn resolve_root_stops_at_git_boundary_not_resolved() {
    // .git at root, no workspace anywhere → not-resolved
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    create_git_boundary(root);

    let child = root.join("some").join("path");
    fs::create_dir_all(&child).unwrap();

    let result = resolve_root(&child);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(error_code(&err), ErrorCode::NotResolved.as_str());
}

#[test]
fn resolve_root_nonexistent_path_not_resolved() {
    let tmp = TempDir::new().unwrap();
    let nonexistent = tmp.path().join("does").join("not").join("exist");

    let result = resolve_root(&nonexistent);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(error_code(&err), ErrorCode::NotResolved.as_str());
}

// ---------------------------------------------------------------------------
// INV-1: path containment (assert_within_root)
// ---------------------------------------------------------------------------

#[test]
fn assert_within_root_accepts_contained_path() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    let child = root.join("sub").join("file.qmd.md");
    fs::create_dir_all(root.join("sub")).unwrap();
    fs::write(&child, "content").unwrap();

    let result = assert_within_root(root, &child);
    assert!(result.is_ok());
}

#[test]
fn assert_within_root_rejects_escaped_path() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path().join("workspace");
    fs::create_dir_all(&root).unwrap();

    // Sibling directory — outside root
    let sibling = tmp.path().join("other");
    fs::create_dir_all(&sibling).unwrap();

    let result = assert_within_root(&root, &sibling);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(error_code(&err), ErrorCode::OutOfRoot.as_str());
}

#[test]
#[cfg(unix)]
fn assert_within_root_rejects_symlink_escape() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path().join("workspace");
    fs::create_dir_all(&root).unwrap();

    let outside = tmp.path().join("secrets");
    fs::create_dir_all(&outside).unwrap();
    fs::write(outside.join("key.txt"), "secret").unwrap();

    // Create a symlink inside root pointing outside (unix-only; symlink creation
    // needs elevated privileges on Windows, so this escape test is unix-gated).
    let link_path = root.join("escape_link");
    std::os::unix::fs::symlink(&outside, &link_path).unwrap();

    let target = link_path.join("key.txt");
    let result = assert_within_root(&root, &target);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(error_code(&err), ErrorCode::OutOfRoot.as_str());
}

#[test]
fn assert_within_root_rejects_dotdot_traversal() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path().join("workspace");
    fs::create_dir_all(&root).unwrap();

    let other = tmp.path().join("other");
    fs::create_dir_all(&other).unwrap();

    // Path with .. that resolves outside root
    let tricky = root.join("..").join("other");
    let result = assert_within_root(&root, &tricky);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(error_code(&err), ErrorCode::OutOfRoot.as_str());
}

#[test]
fn assert_within_root_nonexistent_target_fails_closed() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    // Target doesn't exist — canonicalization fails — fail-closed
    let target = root.join("nonexistent").join("file.txt");
    let result = assert_within_root(root, &target);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(error_code(&err), ErrorCode::OutOfRoot.as_str());
}

// ---------------------------------------------------------------------------
// NFR-2: reparse bound
// ---------------------------------------------------------------------------

#[test]
fn get_index_reparse_bound_exceeded() {
    // Create a workspace with more files than the bound
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    create_workspace(root, "BigWs");

    // Create a few .qmd.md files (more than bound=1 to trigger the test)
    let sub = root.join("docs");
    fs::create_dir_all(&sub).unwrap();
    fs::write(sub.join("a.qmd.md"), "# A\n\n[[A:Thing]]\n\nname: a\n").unwrap();
    fs::write(sub.join("b.qmd.md"), "# B\n\n[[B:Thing]]\n\nname: b\n").unwrap();

    // Use a very low bound (1) to trigger the exceeded condition
    let result = get_index_with_bound(root, 1);
    assert!(result.is_err(), "expected Err, got Ok");
    let err = result.unwrap_err();
    assert_eq!(error_code(&err), ErrorCode::ReparseBoundExceeded.as_str());
}

#[test]
fn get_index_within_bound_succeeds() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    create_workspace(root, "SmallWs");

    let sub = root.join("docs");
    fs::create_dir_all(&sub).unwrap();
    fs::write(sub.join("a.qmd.md"), "# A\n\n[[A:Thing]]\n\nname: a\n").unwrap();

    // Bound of 1000 is plenty
    let result = get_index_with_bound(root, 1000);
    assert!(
        result.is_ok(),
        "expected Ok, got: {:?}",
        result.unwrap_err()
    );
    let index = result.unwrap();
    assert_eq!(index.root, root.canonicalize().unwrap());
    assert!(!index.workspace.objects.is_empty());
    assert!(index.file_count > 0);
}
