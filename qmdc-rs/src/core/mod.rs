//! Core module — transport-agnostic, synchronous, `lsp_types`-free domain logic.
//!
//! All Core operations are pure functions over a resolved index value (`&ResolvedIndex`).
//! They never perform I/O beyond stderr logging and never depend on `tower_lsp::lsp_types`
//! or UTF-16 position encoding.

// Layer 1: shared types
pub mod envelope;
pub mod error;
pub mod fields;
pub mod guide;
pub mod log;

// Layer 2: index seam
pub mod index_seam;
pub mod nesting;
pub mod resolve;
pub mod resolved_index;
pub mod tree;

// Layer 3: invariants + ops
pub mod ops;
pub mod sql_guard;

// Layer 5: graph read view
pub mod graph;

// Re-exports: shared types (Layer 1)
pub use envelope::{bound_list, BoundedEnvelope, DEFAULT_LIMIT};
pub use error::{ErrorCode, ErrorEnvelope};
pub use log::{core_log, EventCategory, Severity};

// Re-exports: index seam (Layer 2)
pub use index_seam::{
    assert_within_root, enforce_force_root, force_root, get_index, resolve_root, set_force_root,
    REPARSE_FILE_BOUND,
};
pub use resolved_index::ResolvedIndex;

// Re-exports: ops (Layers 3-5)
pub use ops::{describe_metamodel, find_path, query, rename_plan, traverse};
