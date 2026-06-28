//! The QMDC MCP server tool surface (rmcp `#[tool_router]`).
//!
//! Each `#[tool]` method validates+deserializes its arguments via the `Parameters<T>` wrapper
//! (schema generated from the `T` param struct), resolves the workspace root, builds the index,
//! and calls the matching Core op. Core ops return the shared `{success, ...}` / error envelope
//! as a `serde_json::Value`; [`envelope_to_result`] converts that into an rmcp `CallToolResult`.

use std::path::Path;

use rmcp::handler::server::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, Content};
use rmcp::{tool, tool_router, ErrorData as McpError};
use serde_json::Value;

use crate::core::index_seam::{enforce_force_root, get_index, resolve_root};
use crate::core::ops;
use crate::core::resolved_index::ResolvedIndex;

use super::params::*;

/// Default bounded-result limit (mirrors `core::envelope::DEFAULT_LIMIT`).
const DEFAULT_LIMIT: usize = 200;

/// The QMDC MCP server. Stateless apart from the generated tool router; per-call workspace
/// indexes are resolved fresh from the supplied `path` (the local single-user model, with an
/// optional process-wide force-root boundary configured at startup).
#[derive(Clone)]
pub struct QmdcServer {
    pub tool_router: ToolRouter<Self>,
}

impl Default for QmdcServer {
    fn default() -> Self {
        Self::new()
    }
}

/// Resolve `limit`/`offset` arguments into concrete `(usize, usize)` with the default limit
/// (200) and a hard ceiling of 1000. Absent values fall back to defaults. Used by `query_sql`,
/// which keeps offset pagination (a free-form SQL result has no stable keyset cursor).
fn paginate(limit: Option<u32>, offset: Option<u32>) -> (usize, usize) {
    let limit = limit
        .map(|l| (l as usize).clamp(1, 1000))
        .unwrap_or(DEFAULT_LIMIT);
    let offset = offset.unwrap_or(0) as usize;
    (limit, offset)
}

/// Clamp a caller `limit` to `1..=1000`, defaulting to 200. Used by the keyset-cursor tools.
fn clamp_limit(limit: Option<u32>) -> usize {
    limit
        .map(|l| (l as usize).clamp(1, 1000))
        .unwrap_or(DEFAULT_LIMIT)
}

/// Convert a Core op's `Result<Value, Value>` envelope into an rmcp `CallToolResult`.
///
/// Both arms serialize the envelope JSON to a single text content block (parity with the
/// previous custom server). The error arm sets `is_error`, matching the old `isError: true`.
fn envelope_to_result(result: Result<Value, Value>) -> Result<CallToolResult, McpError> {
    match result {
        Ok(v) => {
            let text = serde_json::to_string(&v).unwrap_or_default();
            Ok(CallToolResult::success(vec![Content::text(text)]))
        }
        Err(e) => {
            let text = serde_json::to_string(&e).unwrap_or_default();
            Ok(CallToolResult::error(vec![Content::text(text)]))
        }
    }
}

#[tool_router]
impl QmdcServer {
    pub fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }

    /// Resolve a workspace root from `path` and build its index. Errors are converted directly
    /// into an error `CallToolResult` (the precise core error code is preserved in the body).
    fn resolve_and_index(path: &str) -> Result<ResolvedIndex, Box<CallToolResult>> {
        let path = path.trim();
        if path.is_empty() {
            return Err(Box::new(CallToolResult::error(vec![Content::text(
                r#"{"success":false,"error":{"code":"invalid-argument","message":"path must not be empty"}}"#.to_string()
            )])));
        }
        let p = Path::new(path);
        let to_err = |e: Value| {
            let text = serde_json::to_string(&e).unwrap_or_default();
            Box::new(CallToolResult::error(vec![Content::text(text)]))
        };
        enforce_force_root(p).map_err(to_err)?;
        let root = resolve_root(p).map_err(to_err)?;
        enforce_force_root(&root).map_err(to_err)?;
        get_index(&root).map_err(to_err)
    }

    #[tool(
        description = "Locate WHERE a QMD.md object is defined: returns its file, line, id, kind, \
        and namespace. To read an object's fields/content instead, use qmdc_describe_object."
    )]
    async fn qmdc_locate_object(
        &self,
        Parameters(p): Parameters<LocateObjectParams>,
    ) -> Result<CallToolResult, McpError> {
        let index = match Self::resolve_and_index(&p.path) {
            Ok(i) => i,
            Err(e) => return Ok(*e),
        };
        envelope_to_result(ops::locate(&index, &p.r#ref))
    }

    #[tool(
        description = "Find every place that references a QMD.md object (reverse lookup). Returns \
        each referring object's file, line, id, and kind. Resolves namespaced and hierarchical \
        references by identity, not naive text matching."
    )]
    async fn qmdc_find_references(
        &self,
        Parameters(p): Parameters<FindReferencesParams>,
    ) -> Result<CallToolResult, McpError> {
        let index = match Self::resolve_and_index(&p.path) {
            Ok(i) => i,
            Err(e) => return Ok(*e),
        };
        let limit = clamp_limit(p.limit);
        envelope_to_result(ops::find_references(
            &index,
            &p.id,
            limit,
            p.cursor.as_deref(),
        ))
    }

    #[tool(
        description = "Preview a workspace-wide rename of a QMD.md object as a diff — NO files are \
        written. Returns the exact text edits (file, line, old_text, new_text) for the definition \
        and every reference, cascading to descendant ids. Apply the returned edits yourself."
    )]
    async fn qmdc_rename_object(
        &self,
        Parameters(p): Parameters<RenameObjectParams>,
    ) -> Result<CallToolResult, McpError> {
        let index = match Self::resolve_and_index(&p.path) {
            Ok(i) => i,
            Err(e) => return Ok(*e),
        };
        envelope_to_result(ops::rename_plan(&index, &p.old_id, &p.new_id))
    }

    #[tool(
        description = "Describe WHAT a QMD.md object is: returns its full card (label, id, kind, \
        namespace, file, and all fields), or a single field's value and type when 'ref' is a \
        dot-path like 'users.email'. For just the location, use qmdc_locate_object."
    )]
    async fn qmdc_describe_object(
        &self,
        Parameters(p): Parameters<DescribeObjectParams>,
    ) -> Result<CallToolResult, McpError> {
        let index = match Self::resolve_and_index(&p.path) {
            Ok(i) => i,
            Err(e) => return Ok(*e),
        };
        envelope_to_result(ops::describe(&index, &p.r#ref))
    }

    #[tool(
        description = "Outline a single QMD.md file as a nested tree of its objects (id, kind, \
        name, line, children), ordered like the workspace tree. For the whole workspace use \
        qmdc_get_tree."
    )]
    async fn qmdc_outline_file(
        &self,
        Parameters(p): Parameters<OutlineFileParams>,
    ) -> Result<CallToolResult, McpError> {
        let index = match Self::resolve_and_index(&p.path) {
            Ok(i) => i,
            Err(e) => return Ok(*e),
        };
        envelope_to_result(ops::outline(&index, &p.file))
    }

    #[tool(
        description = "Find objects whose id or name contains the query (case-insensitive \
        substring). Returns matching objects with id, name, kind, file, line, and namespace. \
        Use for fuzzy lookup when you don't know the exact id."
    )]
    async fn qmdc_search_objects(
        &self,
        Parameters(p): Parameters<SearchObjectsParams>,
    ) -> Result<CallToolResult, McpError> {
        let index = match Self::resolve_and_index(&p.path) {
            Ok(i) => i,
            Err(e) => return Ok(*e),
        };
        let limit = clamp_limit(p.limit);
        envelope_to_result(ops::search(
            &index,
            &p.query,
            p.namespace.as_deref(),
            limit,
            p.cursor.as_deref(),
        ))
    }

    #[tool(
        description = "Check the workspace (or one file) for broken or ambiguous references. \
        Returns diagnostics, each with file, line, code (QMDC001 = not-found, QMDC002 = ambiguous), \
        and a message."
    )]
    async fn qmdc_validate_references(
        &self,
        Parameters(p): Parameters<ValidateReferencesParams>,
    ) -> Result<CallToolResult, McpError> {
        let index = match Self::resolve_and_index(&p.path) {
            Ok(i) => i,
            Err(e) => return Ok(*e),
        };
        envelope_to_result(ops::validate(&index, p.file.as_deref()))
    }

    #[tool(
        description = "Get the workspace structure as a keyset-paginated pre-order node \
        stream. Each node carries level + parent so the client rebuilds the hierarchy \
        incrementally (load N nodes, scroll indefinitely). Optional 'namespace' filter; \
        page with 'limit' + the opaque 'cursor' from the previous response."
    )]
    async fn qmdc_get_tree(
        &self,
        Parameters(p): Parameters<GetTreeParams>,
    ) -> Result<CallToolResult, McpError> {
        let index = match Self::resolve_and_index(&p.path) {
            Ok(i) => i,
            Err(e) => return Ok(*e),
        };
        let limit = clamp_limit(p.limit);
        envelope_to_result(ops::tree(
            &index,
            p.namespace.as_deref(),
            limit,
            p.cursor.as_deref(),
        ))
    }

    #[tool(
        description = "Run a read-only SQL SELECT over the workspace's in-memory index. \
        Tables: objects(__id, __kind, __label, __namespace, __file, __line, __parent, __level, \
        data) and edges(source_id, target_id, edge_type, source_field). Pass '#query_id' to run \
        a stored Query object's SQL. Use 'limit'/'offset' to page large results."
    )]
    async fn qmdc_query_sql(
        &self,
        Parameters(p): Parameters<QuerySqlParams>,
    ) -> Result<CallToolResult, McpError> {
        let index = match Self::resolve_and_index(&p.path) {
            Ok(i) => i,
            Err(e) => return Ok(*e),
        };
        let (limit, offset) = paginate(p.limit, p.offset);
        envelope_to_result(ops::query(&index, &p.sql, limit, offset))
    }

    #[tool(
        description = "Dump the parsed index (objects and files) as JSON. Supports pagination \
        and filtering by namespace/kind to avoid overwhelming the context window."
    )]
    async fn qmdc_dump_index(
        &self,
        Parameters(p): Parameters<DumpIndexParams>,
    ) -> Result<CallToolResult, McpError> {
        let index = match Self::resolve_and_index(&p.path) {
            Ok(i) => i,
            Err(e) => return Ok(*e),
        };
        let limit = clamp_limit(p.limit);
        envelope_to_result(ops::dump(
            &index,
            p.namespace.as_deref(),
            p.kind.as_deref(),
            limit,
            p.cursor.as_deref(),
        ))
    }

    #[tool(
        description = "Walk the reference graph from a start object and return the connected \
        objects and typed edges within 'depth' hops. For the link between two specific objects, \
        use qmdc_find_path instead."
    )]
    async fn qmdc_traverse_graph(
        &self,
        Parameters(p): Parameters<TraverseGraphParams>,
    ) -> Result<CallToolResult, McpError> {
        let index = match Self::resolve_and_index(&p.path) {
            Ok(i) => i,
            Err(e) => return Ok(*e),
        };
        let direction = p.direction.as_deref().unwrap_or("outgoing");
        let depth = p.depth.map(|d| d as usize).unwrap_or(3);
        envelope_to_result(ops::traverse(
            &index,
            &p.start_id,
            p.edge_type.as_deref(),
            direction,
            depth,
        ))
    }

    #[tool(
        description = "Find a connecting path between two QMD.md objects through their references. \
        Returns the ordered chain of objects and edges, or a clearly-marked no-path result."
    )]
    async fn qmdc_find_path(
        &self,
        Parameters(p): Parameters<FindPathParams>,
    ) -> Result<CallToolResult, McpError> {
        let index = match Self::resolve_and_index(&p.path) {
            Ok(i) => i,
            Err(e) => return Ok(*e),
        };
        envelope_to_result(ops::find_path(
            &index,
            &p.from_id,
            &p.to_id,
            p.edge_type.as_deref(),
        ))
    }

    #[tool(
        description = "Discover the workspace's vocabulary: which object kinds exist, the count \
        per kind, the fields seen on each kind, and edge-type counts. Call this first to learn \
        the schema before writing qmdc_query_sql or qmdc_traverse_graph."
    )]
    async fn qmdc_describe_metamodel(
        &self,
        Parameters(p): Parameters<DescribeMetamodelParams>,
    ) -> Result<CallToolResult, McpError> {
        let index = match Self::resolve_and_index(&p.path) {
            Ok(i) => i,
            Err(e) => return Ok(*e),
        };
        envelope_to_result(ops::describe_metamodel(&index, p.namespace.as_deref()))
    }

    #[tool(
        description = "Return the QMD.md format guide for agents (the syntax of objects, \
        references, fields, and namespaces). Read this first if you are unfamiliar with QMDC."
    )]
    async fn qmdc_get_guide(&self) -> Result<CallToolResult, McpError> {
        Ok(CallToolResult::success(vec![Content::text(
            crate::core::guide::GUIDE_CONTENT.to_string(),
        )]))
    }
}
