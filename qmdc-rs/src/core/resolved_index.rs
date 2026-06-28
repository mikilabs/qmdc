//! Resolved index value — the materialised workspace state that Core ops consume.
//!
//! This is a pure data-model (D1): root `PathBuf`, parsed objects, in-memory `QmdcDatabase`,
//! and freshness metadata. No `lsp_types`, no UTF-16, no network I/O.

use std::path::PathBuf;
use std::time::Instant;

use serde_json::Value;

use crate::db::QmdcDatabase;
use crate::WorkspaceResult;

/// The resolved, ready-to-query index for a single workspace root.
///
/// Produced by [`super::index_seam::get_index`]. Consumed by all Core operations
/// as an immutable reference (`&ResolvedIndex`).
pub struct ResolvedIndex {
    /// Canonical, absolute root directory of the resolved workspace.
    pub root: PathBuf,

    /// The workspace parse result (objects, files, errors).
    pub workspace: WorkspaceResult,

    /// In-memory SQLite database populated via `sync_objects_from_vec`.
    pub db: QmdcDatabase,

    /// Instant at which this index was built (monotonic clock).
    pub built_at: Instant,

    /// Number of `.qmd.md` files that were parsed to produce this index.
    pub file_count: usize,
}

impl std::fmt::Debug for ResolvedIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ResolvedIndex")
            .field("root", &self.root)
            .field("file_count", &self.file_count)
            .field("object_count", &self.workspace.objects.len())
            .field("built_at", &self.built_at)
            .finish_non_exhaustive()
    }
}

impl ResolvedIndex {
    /// Returns the parsed objects slice.
    pub fn objects(&self) -> &[Value] {
        &self.workspace.objects
    }

    /// Returns the list of relative file paths in this workspace.
    pub fn files(&self) -> &[String] {
        &self.workspace.files
    }
}
