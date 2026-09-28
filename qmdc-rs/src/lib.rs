pub mod core;
pub mod db;
pub mod ignore;
pub mod lsp;
pub mod mcp;
pub mod parser;
mod parser_modules;
pub mod rebuild;
pub mod workspace;

pub use core::{
    assert_within_root, get_index, resolve_root, resolve_root_bidirectional, BoundedEnvelope,
    ErrorCode, ErrorEnvelope, ResolvedIndex, REPARSE_FILE_BOUND,
};
pub use db::{execute_query, QmdcDatabase, QueryResult};
pub use lsp::run_lsp;
pub use mcp::run_mcp_server;
pub use parser::{parse, OutputFormat, ParseOptions, QmdcObject};
pub use rebuild::rebuild;
pub use workspace::{
    compose_with_paths, compose_workspace_roots, dir_is_workspace_root,
    find_nested_workspace_roots, find_workspace_root, parse_all_workspaces, parse_workspace,
    rescan_composed_references, resolve_workspace, resolve_workspace_input, scan_workspace,
    Composition, Mount, WorkspaceEntry, WorkspaceError, WorkspaceResult,
};
