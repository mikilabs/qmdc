//! Workspace tree rendering — transport-agnostic (Core / LSP / CLI share this).
//!
//! Split by mode to keep each file small and focused:
//! - [`builders`] — pure flat-list → hierarchy builders (`__level` and `__parent` based).
//! - [`by_namespace`] — `namespace` mode orchestration + per-namespace kind groups.
//! - [`by_namespace_toplevel`] — `namespace` mode's no-namespace top-level grouping.
//! - [`by_file`] — `file` mode.
//! - [`by_smart`] — `smart` mode.

mod builders;
mod by_file;
mod by_namespace;
mod by_namespace_toplevel;
mod by_smart;

use crate::db::QmdcDatabase;

/// Local result alias — these builders never return `Err` (errors are surfaced in the
/// returned JSON as `{success:false}`), so a plain String-error result keeps this module
/// transport-agnostic (no `tower_lsp` dependency) and usable from Core, LSP, and CLI alike.
type Result<T> = std::result::Result<T, String>;

pub use builders::{build_object_tree, build_object_tree_smart};
pub use by_file::get_tree_by_file;
pub use by_namespace::get_tree_by_namespace;
pub use by_smart::get_tree_by_smart;

/// Query all `__Workspace` objects (the common header of every tree mode).
///
/// On query failure returns `Err(json)` where the value is the `{success:false}` envelope
/// the caller should return directly — preserving the original per-mode error behaviour.
pub(crate) fn query_workspaces(
    db: &QmdcDatabase,
) -> std::result::Result<crate::db::QueryResult, serde_json::Value> {
    let workspaces_query = "SELECT __id, __label, __file, __line FROM objects WHERE __kind = '__Workspace' ORDER BY __label COLLATE NOCASE";
    db.query(workspaces_query).map_err(|e| {
        serde_json::json!({
            "success": false,
            "error": format!("Failed to query workspaces: {}", e)
        })
    })
}
