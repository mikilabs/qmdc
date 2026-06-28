pub mod core;
pub mod db;
pub mod lsp;
pub mod mcp;
pub mod parser;
mod parser_modules;
pub mod rebuild;
pub mod workspace;

pub use core::{
    assert_within_root, get_index, resolve_root, BoundedEnvelope, ErrorCode, ErrorEnvelope,
    ResolvedIndex, REPARSE_FILE_BOUND,
};
pub use db::{execute_query, QmdcDatabase, QueryResult};
pub use lsp::run_lsp;
pub use mcp::run_mcp_server;
pub use parser::{parse, OutputFormat, ParseOptions, QmdcObject};
pub use rebuild::rebuild;
pub use workspace::{
    find_nested_workspace_roots, find_workspace_root, parse_all_workspaces, parse_workspace,
    resolve_workspace, scan_workspace, WorkspaceError, WorkspaceResult,
};
