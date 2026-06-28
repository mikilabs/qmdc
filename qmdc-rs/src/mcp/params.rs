//! Tool parameter structs for the rmcp-based MCP server.
//!
//! Each struct derives `Deserialize` + `JsonSchema`, so the input schema advertised in
//! `tools/list` is generated from the SAME type that deserializes the call arguments —
//! a single source of truth (no hand-written JSON schemas that can drift). `#[schemars(...)]`
//! descriptions become the per-field docs the client sees.

use rmcp::schemars::{self, JsonSchema};
use serde::Deserialize;

/// Shared description for the ubiquitous `path` argument.
pub const PATH_DESC: &str = "Any file or directory inside the target QMDC workspace. Used only to \
    locate the workspace root; the operation covers the whole workspace, not just this path. \
    Tip: pass the file you are working in.";

#[derive(Debug, Deserialize, JsonSchema)]
pub struct LocateObjectParams {
    #[schemars(
        description = "Any file or directory inside the target QMDC workspace (locates the workspace root)."
    )]
    pub path: String,
    #[schemars(
        description = "Object reference: a bare id ('users'), namespaced ('storage:users'), kind-qualified, or a field dot-path ('users.email'). A leading '#' is optional."
    )]
    pub r#ref: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct FindReferencesParams {
    #[schemars(
        description = "Any file or directory inside the target QMDC workspace (locates the workspace root)."
    )]
    pub path: String,
    #[schemars(
        description = "Object id to find references to (e.g. 'users'). Leading '#' is optional."
    )]
    pub id: String,
    #[schemars(description = "Maximum number of results to return (1-1000, default 200).")]
    pub limit: Option<u32>,
    #[schemars(
        description = "Opaque pagination cursor from a previous response's next_cursor; results resume strictly after it."
    )]
    pub cursor: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RenameObjectParams {
    #[schemars(
        description = "Any file or directory inside the target QMDC workspace (locates the workspace root)."
    )]
    pub path: String,
    #[schemars(description = "Current object id to rename (e.g. 'team'). Descendants cascade.")]
    pub old_id: String,
    #[schemars(description = "New id. Allowed characters: alphanumeric, dash, underscore, dot.")]
    pub new_id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DescribeObjectParams {
    #[schemars(
        description = "Any file or directory inside the target QMDC workspace (locates the workspace root)."
    )]
    pub path: String,
    #[schemars(
        description = "Object reference: a bare id, namespaced, kind-qualified, or a field dot-path ('users.email'). Leading '#' optional."
    )]
    pub r#ref: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct OutlineFileParams {
    #[schemars(
        description = "Any file or directory inside the target QMDC workspace (locates the workspace root)."
    )]
    pub path: String,
    #[schemars(
        description = "Relative path of the file to outline within the workspace (e.g. 'schema.qmd.md')."
    )]
    pub file: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SearchObjectsParams {
    #[schemars(
        description = "Any file or directory inside the target QMDC workspace (locates the workspace root)."
    )]
    pub path: String,
    #[schemars(
        description = "Case-insensitive substring matched against each object's id and name."
    )]
    pub query: String,
    #[schemars(description = "Optional: limit results to this namespace.")]
    pub namespace: Option<String>,
    #[schemars(description = "Maximum number of results to return (1-1000, default 200).")]
    pub limit: Option<u32>,
    #[schemars(
        description = "Opaque pagination cursor from a previous response's next_cursor; results resume strictly after it."
    )]
    pub cursor: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ValidateReferencesParams {
    #[schemars(
        description = "Any file or directory inside the target QMDC workspace (locates the workspace root)."
    )]
    pub path: String,
    #[schemars(
        description = "Optional: limit the check to this relative file path within the workspace."
    )]
    pub file: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetTreeParams {
    #[schemars(
        description = "Any file or directory inside the target QMDC workspace (locates the workspace root)."
    )]
    pub path: String,
    #[schemars(description = "Optional: limit the node stream to this namespace.")]
    pub namespace: Option<String>,
    #[schemars(description = "Maximum number of nodes to return per page (1-1000, default 200).")]
    pub limit: Option<u32>,
    #[schemars(
        description = "Opaque pagination cursor from a previous response's next_cursor; the pre-order node stream resumes strictly after it."
    )]
    pub cursor: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct QuerySqlParams {
    #[schemars(
        description = "Any file or directory inside the target QMDC workspace (locates the workspace root)."
    )]
    pub path: String,
    #[schemars(
        description = "A read-only SELECT over the 'objects' and 'edges' tables, or '#query_id' to run a stored Query object's SQL."
    )]
    pub sql: String,
    #[schemars(description = "Maximum number of rows to return (1-1000, default 200).")]
    pub limit: Option<u32>,
    #[schemars(description = "Number of rows to skip for pagination (default 0).")]
    pub offset: Option<u32>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DumpIndexParams {
    #[schemars(
        description = "Any file or directory inside the target QMDC workspace (locates the workspace root)."
    )]
    pub path: String,
    #[schemars(description = "Optional: filter objects to this namespace.")]
    pub namespace: Option<String>,
    #[schemars(description = "Optional: filter objects to this __kind.")]
    pub kind: Option<String>,
    #[schemars(description = "Maximum number of objects to return (1-1000, default 200).")]
    pub limit: Option<u32>,
    #[schemars(
        description = "Opaque pagination cursor from a previous response's next_cursor; results resume strictly after it."
    )]
    pub cursor: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct TraverseGraphParams {
    #[schemars(
        description = "Any file or directory inside the target QMDC workspace (locates the workspace root)."
    )]
    pub path: String,
    #[schemars(description = "Object id to start the walk from (e.g. 'users').")]
    pub start_id: String,
    #[schemars(
        description = "Which references to follow: 'outgoing' (default), 'incoming', or 'both'."
    )]
    pub direction: Option<String>,
    #[schemars(description = "Maximum hops from the start object (1-50, default 3).")]
    pub depth: Option<u32>,
    #[schemars(description = "Optional: only follow references whose field name equals this.")]
    pub edge_type: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct FindPathParams {
    #[schemars(
        description = "Any file or directory inside the target QMDC workspace (locates the workspace root)."
    )]
    pub path: String,
    #[schemars(description = "Source object id.")]
    pub from_id: String,
    #[schemars(description = "Target object id.")]
    pub to_id: String,
    #[schemars(description = "Optional: only follow references whose field name equals this.")]
    pub edge_type: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DescribeMetamodelParams {
    #[schemars(
        description = "Any file or directory inside the target QMDC workspace (locates the workspace root)."
    )]
    pub path: String,
    #[schemars(description = "Optional: limit the summary to this namespace.")]
    pub namespace: Option<String>,
}
