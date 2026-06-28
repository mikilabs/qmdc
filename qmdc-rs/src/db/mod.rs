//! SQLite database module for QMDC workspace queries
//!
//! Provides in-memory SQLite storage for QMD.md objects and edges (references),
//! enabling SQL queries against the workspace graph.

use rusqlite::{params, Connection, OptionalExtension, Result as SqliteResult};
use serde_json::Value;
use std::collections::HashMap;

use crate::workspace::WorkspaceResult;

/// Execute a query against a workspace result.
///
/// The query can be:
/// - A SQL query (e.g., "SELECT * FROM objects")
/// - A reference to a Query object (e.g., "#get_tables")
///
/// # Example
/// ```ignore
/// let result = parse_workspace(path, OutputFormat::Standard);
/// let data = execute_query(&result, "#get_tables")?;
/// println!("{}", data.to_table_string());
/// ```
pub fn execute_query(workspace: &WorkspaceResult, query: &str) -> Result<QueryResult, String> {
    // Create database and load objects
    let db = QmdcDatabase::new().map_err(|e| e.to_string())?;

    // Convert objects to vector for sync_objects_from_vec
    let objects_vec: Vec<Value> = workspace.objects.to_vec();

    db.sync_objects_from_vec(&objects_vec)
        .map_err(|e| e.to_string())?;

    // Resolve query
    let sql = if let Some(query_id) = query.strip_prefix('#') {
        // Find Query object by ID
        let query_obj = workspace
            .objects
            .iter()
            .find(|obj| {
                obj.get("__id").and_then(|id| id.as_str()) == Some(query_id)
                    && obj.get("__kind").and_then(|k| k.as_str()) == Some("Query")
            })
            .ok_or_else(|| format!("Query object '{}' not found", query_id))?;

        query_obj
            .get("sql")
            .and_then(|s| s.as_str())
            .ok_or_else(|| format!("Query object '{}' has no 'sql' field", query_id))?
            .to_string()
    } else {
        query.to_string()
    };

    db.query(&sql)
}

/// QMDC SQLite database wrapper
pub struct QmdcDatabase {
    conn: Connection,
}

impl QmdcDatabase {
    /// Create a new in-memory SQLite database with QMDC schema
    pub fn new() -> SqliteResult<Self> {
        let conn = Connection::open_in_memory()?;
        let db = Self { conn };
        db.create_schema()?;
        Ok(db)
    }

    /// Create the database schema
    fn create_schema(&self) -> SqliteResult<()> {
        self.conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS objects (
                __workspace TEXT NOT NULL,
                __namespace TEXT NOT NULL DEFAULT '',
                __id TEXT NOT NULL,
                __global_id TEXT GENERATED ALWAYS AS (
                    __workspace || ':' || CASE WHEN __namespace = '' THEN ':' ELSE __namespace || ':' END || __id
                ) STORED UNIQUE,
                __kind TEXT,
                __label TEXT,
                __local_id TEXT,
                __file TEXT,
                __parent TEXT,
                __line INTEGER,
                __level INTEGER,
                data TEXT,
                PRIMARY KEY (__workspace, __namespace, __id)
            );

            CREATE TABLE IF NOT EXISTS edges (
                source_id TEXT NOT NULL,
                source_field TEXT NOT NULL,
                target_id TEXT NOT NULL,
                edge_type TEXT NOT NULL,
                target_field TEXT NOT NULL DEFAULT '',
                __workspace TEXT,
                UNIQUE(source_id, source_field, target_id, edge_type),
                FOREIGN KEY (source_id) REFERENCES objects(__global_id),
                FOREIGN KEY (target_id) REFERENCES objects(__global_id)
            );

            CREATE INDEX IF NOT EXISTS idx_objects_kind ON objects(__kind);
            CREATE INDEX IF NOT EXISTS idx_objects_namespace ON objects(__namespace);
            CREATE INDEX IF NOT EXISTS idx_objects_parent ON objects(__parent);
            CREATE INDEX IF NOT EXISTS idx_objects_workspace ON objects(__workspace);
            CREATE INDEX IF NOT EXISTS idx_edges_source ON edges(source_id);
            CREATE INDEX IF NOT EXISTS idx_edges_target ON edges(target_id);
            "#,
        )
    }

    /// Clear all data (for resync)
    pub fn clear(&self) -> SqliteResult<()> {
        self.conn.execute_batch(
            r#"
            DELETE FROM edges;
            DELETE FROM objects;
            "#,
        )
    }

    /// Insert or replace an object
    pub fn upsert_object(&self, obj: &Value) -> SqliteResult<()> {
        let id = obj.get("__id").and_then(|v| v.as_str()).unwrap_or("");
        let kind = obj.get("__kind").and_then(|v| v.as_str());
        let label = obj.get("__label").and_then(|v| v.as_str());
        let local_id = obj.get("__local_id").and_then(|v| v.as_str());
        // Extract namespace ID from [[#id]] format or use as-is if already plain ID
        let namespace = obj
            .get("__namespace")
            .and_then(|v| v.as_str())
            .map(|ns_str| {
                if let Some(id_part) = ns_str
                    .strip_prefix("[[#")
                    .and_then(|s| s.strip_suffix("]]"))
                {
                    id_part
                } else {
                    ns_str
                }
            });
        // Extract workspace ID from [[#id]] format or use as-is if already plain ID
        let workspace = obj
            .get("__workspace")
            .and_then(|v| v.as_str())
            .map(|ws_str| {
                if let Some(id_part) = ws_str
                    .strip_prefix("[[#")
                    .and_then(|s| s.strip_suffix("]]"))
                {
                    id_part
                } else {
                    ws_str
                }
            });
        // Extract parent ID from [[#id]] format or use as-is if already plain ID
        let parent = obj.get("__parent").and_then(|v| v.as_str()).map(|p_str| {
            if let Some(id_part) = p_str.strip_prefix("[[#").and_then(|s| s.strip_suffix("]]")) {
                id_part
            } else {
                p_str
            }
        });
        let file = obj.get("__file").and_then(|v| v.as_str());
        let line = obj.get("__line").and_then(|v| v.as_i64());

        // Build data JSON without system fields
        let data = self.extract_user_data(obj);

        let level = obj.get("__level").and_then(|v| v.as_i64());

        // Ensure workspace is not NULL (required for PRIMARY KEY)
        let workspace = workspace.unwrap_or("");
        let namespace = namespace.unwrap_or("");

        self.conn.execute(
            r#"
            INSERT OR REPLACE INTO objects (__workspace, __namespace, __id, __kind, __label, __local_id, __file, __parent, __line, __level, data)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
            "#,
            params![workspace, namespace, id, kind, label, local_id, file, parent, line, level, data],
        )?;

        Ok(())
    }

    /// Extract user data (non-system fields) as JSON string.
    ///
    /// Canonical form (cross-parser byte parity): compact JSON, raw UTF-8, and
    /// keys in document insertion order. We build a `serde_json::Map` (which,
    /// with the `preserve_order` feature, is backed by an IndexMap) by iterating
    /// the source object in order — a `HashMap` here would scramble key order.
    /// Float literals such as `1.0` keep their float form free of charge because
    /// serde_json serializes a JSON float with a trailing `.0` (the TS parser
    /// must reconstruct this from raw tokens; Python gets it from its float type).
    fn extract_user_data(&self, obj: &Value) -> String {
        if let Some(map) = obj.as_object() {
            let user_data: serde_json::Map<String, Value> = map
                .iter()
                .filter(|(k, _)| !k.starts_with("__"))
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
            serde_json::to_string(&user_data).unwrap_or_else(|_| "{}".to_string())
        } else {
            "{}".to_string()
        }
    }

    /// Compute __global_id from workspace, namespace, and id
    /// Must match the generated column formula in schema
    fn compute_global_id(workspace: &str, namespace: &str, id: &str) -> String {
        if namespace.is_empty() {
            format!("{}::{}", workspace, id)
        } else {
            format!("{}:{}:{}", workspace, namespace, id)
        }
    }

    /// Insert an edge (reference)
    /// source_id and target_id should be __global_id values computed using compute_global_id
    /// edge_type defaults to source_field if not provided
    pub fn insert_edge(
        &self,
        source_id: &str,
        source_field: &str,
        target_id: &str,
        edge_type: Option<&str>,
    ) -> SqliteResult<()> {
        self.insert_edge_with_target_field(source_id, source_field, target_id, edge_type, "")
    }

    /// Insert edge with target_field support
    pub fn insert_edge_with_target_field(
        &self,
        source_id: &str,
        source_field: &str,
        target_id: &str,
        edge_type: Option<&str>,
        target_field: &str,
    ) -> SqliteResult<()> {
        // Extract workspace from source_id (format: workspace:namespace:id or workspace:id)
        let workspace = source_id.split(':').next().unwrap_or("");
        let actual_edge_type = edge_type.unwrap_or(source_field);
        self.conn.execute(
            "INSERT OR IGNORE INTO edges (source_id, source_field, target_id, edge_type, target_field, __workspace) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![source_id, source_field, target_id, actual_edge_type, target_field, workspace],
        )?;
        Ok(())
    }

    /// Delete edges for a source object (for resync)
    pub fn delete_edges_for_source(&self, source_id: &str) -> SqliteResult<()> {
        self.conn
            .execute("DELETE FROM edges WHERE source_id = ?1", params![source_id])?;
        Ok(())
    }

    /// Execute a SQL query and return results as JSON
    /// Also supports dot-commands: .schema, .tables
    pub fn query(&self, sql: &str) -> Result<QueryResult, String> {
        self.query_with_params(sql, &[])
    }

    /// Execute a query with SQLite's `query_only` pragma enabled for its duration — a
    /// defense-in-depth layer behind the statement-level read-only guard (`sql_guard`).
    ///
    /// Even if a write statement slipped past the parser allowlist, SQLite itself rejects it
    /// while `query_only` is ON. The pragma is restored to OFF afterwards, so the connection
    /// remains usable for later writes (e.g. a re-sync); this is safe on the single-threaded
    /// query path.
    pub fn query_read_only(&self, sql: &str) -> Result<QueryResult, String> {
        self.conn
            .execute_batch("PRAGMA query_only = ON;")
            .map_err(|e| e.to_string())?;
        let result = self.query(sql);
        // Best-effort restore; failure here doesn't compromise the read just performed.
        let _ = self.conn.execute_batch("PRAGMA query_only = OFF;");
        result
    }

    /// Execute a SQL query with parameters and return results as JSON
    pub fn query_with_params(
        &self,
        sql: &str,
        params: &[&dyn rusqlite::ToSql],
    ) -> Result<QueryResult, String> {
        let sql = sql.trim();

        // Handle dot-commands
        if sql.starts_with('.') {
            return self.handle_dot_command(sql);
        }

        let mut stmt = self.conn.prepare(sql).map_err(|e| e.to_string())?;

        let column_names: Vec<String> = stmt.column_names().iter().map(|s| s.to_string()).collect();

        let rows: Result<Vec<Vec<Value>>, _> = stmt
            .query_map(params, |row| {
                let mut values = Vec::new();
                for i in 0..column_names.len() {
                    let value = Self::sqlite_value_to_json(row, i);
                    values.push(value);
                }
                Ok(values)
            })
            .map_err(|e| e.to_string())?
            .collect();

        let rows = rows.map_err(|e| e.to_string())?;

        Ok(QueryResult {
            columns: column_names,
            rows,
        })
    }

    /// Handle dot-commands (.schema, .tables, etc.)
    fn handle_dot_command(&self, cmd: &str) -> Result<QueryResult, String> {
        let parts: Vec<&str> = cmd.split_whitespace().collect();
        let command = parts.first().unwrap_or(&"");

        match *command {
            ".tables" => {
                self.query("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
            }
            ".schema" => {
                let table = parts.get(1);
                if let Some(table) = table {
                    // Use parameterized query to prevent SQL injection
                    let mut stmt = self
                        .conn
                        .prepare("SELECT sql FROM sqlite_master WHERE type='table' AND name=?1")
                        .map_err(|e| e.to_string())?;

                    let column_names: Vec<String> =
                        stmt.column_names().iter().map(|s| s.to_string()).collect();

                    let rows: Result<Vec<Vec<Value>>, _> = stmt
                        .query_map([table], |row| {
                            let mut values = Vec::new();
                            for i in 0..column_names.len() {
                                let value = Self::sqlite_value_to_json(row, i);
                                values.push(value);
                            }
                            Ok(values)
                        })
                        .map_err(|e| e.to_string())?
                        .collect();

                    let rows = rows.map_err(|e| e.to_string())?;

                    Ok(QueryResult {
                        columns: column_names,
                        rows,
                    })
                } else {
                    self.query(
                        "SELECT name, sql FROM sqlite_master WHERE type='table' ORDER BY name",
                    )
                }
            }
            ".help" => Ok(QueryResult {
                columns: vec!["command".to_string(), "description".to_string()],
                rows: vec![
                    vec![
                        Value::String(".tables".to_string()),
                        Value::String("List all tables".to_string()),
                    ],
                    vec![
                        Value::String(".schema [table]".to_string()),
                        Value::String("Show table schema".to_string()),
                    ],
                    vec![
                        Value::String(".help".to_string()),
                        Value::String("Show this help".to_string()),
                    ],
                ],
            }),
            _ => Err(format!("Unknown command: {}. Try .help", command)),
        }
    }

    /// Convert SQLite value to JSON value
    fn sqlite_value_to_json(row: &rusqlite::Row, idx: usize) -> Value {
        // Try different types in order
        if let Ok(v) = row.get::<_, i64>(idx) {
            return Value::Number(v.into());
        }
        if let Ok(v) = row.get::<_, f64>(idx) {
            return serde_json::Number::from_f64(v)
                .map(Value::Number)
                .unwrap_or(Value::Null);
        }
        if let Ok(v) = row.get::<_, String>(idx) {
            return Value::String(v);
        }
        Value::Null
    }

    /// Sync objects from workspace index
    pub fn sync_objects(&self, objects: &HashMap<String, Value>) -> SqliteResult<()> {
        self.clear()?;

        for obj in objects.values() {
            self.upsert_object(obj)?;

            // Extract and insert edges from references
            if let Some(id) = obj.get("__id").and_then(|v| v.as_str()) {
                self.extract_and_insert_edges(id, obj)?;
            }
        }

        Ok(())
    }

    /// Sync objects from a vector (allows duplicates)
    pub fn sync_objects_from_vec(&self, objects: &[Value]) -> SqliteResult<()> {
        self.clear()?;

        // First pass: insert all objects
        for obj in objects {
            self.upsert_object(obj)?;
        }

        // Second pass: extract and insert edges (all objects are now in DB)
        for obj in objects {
            if let Some(id) = obj.get("__id").and_then(|v| v.as_str()) {
                self.extract_and_insert_edges(id, obj)?;
            }
        }

        Ok(())
    }

    /// Extract references from object and insert as edges
    fn extract_and_insert_edges(&self, source_id: &str, obj: &Value) -> SqliteResult<()> {
        // Compute source_global_id
        let workspace = obj
            .get("__workspace")
            .and_then(|v| v.as_str())
            .map(|ws_str| {
                if let Some(id_part) = ws_str
                    .strip_prefix("[[#")
                    .and_then(|s| s.strip_suffix("]]"))
                {
                    id_part
                } else {
                    ws_str
                }
            })
            .unwrap_or("");
        let namespace = obj
            .get("__namespace")
            .and_then(|v| v.as_str())
            .map(|ns_str| {
                if let Some(id_part) = ns_str
                    .strip_prefix("[[#")
                    .and_then(|s| s.strip_suffix("]]"))
                {
                    id_part
                } else {
                    ns_str
                }
            })
            .unwrap_or("");
        let source_global_id = Self::compute_global_id(workspace, namespace, source_id);

        if let Some(map) = obj.as_object() {
            for (field, value) in map {
                // Skip system fields
                if field.starts_with("__") {
                    continue;
                }

                // Check for reference pattern [[#...]]
                self.extract_refs_from_value(
                    &source_global_id,
                    field,
                    value,
                    workspace,
                    namespace,
                )?;
            }
        }
        Ok(())
    }

    /// Resolve a target reference and insert an edge if the target exists.
    /// If the target doesn't exist as an object but contains a dot, try field-level resolution:
    /// split on last dot, check if prefix is an object and suffix is a field on it.
    fn resolve_and_insert_edge(
        &self,
        source_id: &str,
        field: &str,
        target_id: &str,
        workspace: &str,
        namespace: &str,
        edge_type: Option<&str>,
    ) -> SqliteResult<()> {
        if let Some(tgid) = self.resolve_target_global_id(target_id, workspace, namespace)? {
            self.insert_edge(source_id, field, &tgid, edge_type)?;
        } else if target_id.contains('.') {
            // Field-level resolution: try splitting on last dot
            if let Some(last_dot) = target_id.rfind('.') {
                let obj_part = &target_id[..last_dot];
                let field_part = &target_id[last_dot + 1..];
                if let Some(tgid) = self.resolve_target_global_id(obj_part, workspace, namespace)? {
                    self.insert_edge_with_target_field(
                        source_id, field, &tgid, edge_type, field_part,
                    )?;
                }
            }
        }
        Ok(())
    }

    /// Recursively extract references from a value
    /// Resolves target references using JOIN to find __global_id from __id
    fn extract_refs_from_value(
        &self,
        source_id: &str,
        field: &str,
        value: &Value,
        workspace: &str,
        namespace: &str,
    ) -> SqliteResult<()> {
        match value {
            Value::String(s) => {
                // Try preamble extraction for text field values
                if let Some(preamble_edges) = Self::extract_preamble_refs(s) {
                    let mut preamble_targets = std::collections::HashSet::new();
                    for (preamble_key, target_id) in &preamble_edges {
                        self.resolve_and_insert_edge(
                            source_id,
                            field,
                            target_id,
                            workspace,
                            namespace,
                            Some(preamble_key),
                        )?;
                        preamble_targets.insert(target_id.clone());
                    }
                    // Also extract remaining refs from the rest of the text
                    for target_id in self.parse_all_references(s) {
                        if !preamble_targets.contains(&target_id) {
                            self.resolve_and_insert_edge(
                                source_id, field, &target_id, workspace, namespace, None,
                            )?;
                        }
                    }
                } else {
                    // No preamble — standard extraction
                    for target_id in self.parse_all_references(s) {
                        self.resolve_and_insert_edge(
                            source_id, field, &target_id, workspace, namespace, None,
                        )?;
                    }
                }
            }
            Value::Array(arr) => {
                for item in arr {
                    self.extract_refs_from_value(source_id, field, item, workspace, namespace)?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Extract typed edges from text field preamble.
    /// A preamble is a markdown list at the start of a text field where ALL items
    /// are valid `- key: [[#ref]]` fields. All-or-nothing.
    fn extract_preamble_refs(text: &str) -> Option<Vec<(String, String)>> {
        use std::sync::OnceLock;

        static FIELD_KEY_RE: OnceLock<regex::Regex> = OnceLock::new();
        static SINGLE_REF_RE: OnceLock<regex::Regex> = OnceLock::new();
        static MULTI_REF_RE: OnceLock<regex::Regex> = OnceLock::new();
        static REF_EXTRACT_RE: OnceLock<regex::Regex> = OnceLock::new();

        let field_key_re =
            FIELD_KEY_RE.get_or_init(|| regex::Regex::new(r"^[a-zA-Z_][a-zA-Z0-9_]*$").unwrap());
        let single_ref_re =
            SINGLE_REF_RE.get_or_init(|| regex::Regex::new(r"^\[\[#[^\]]+\]\]$").unwrap());
        let multi_ref_re = MULTI_REF_RE.get_or_init(|| {
            regex::Regex::new(r"^\[\[#[^\]]+\]\](?:\s*,\s*\[\[#[^\]]+\]\])+$").unwrap()
        });
        let ref_extract_re =
            REF_EXTRACT_RE.get_or_init(|| regex::Regex::new(r"\[\[#([^\]]+)\]\]").unwrap());

        if !text.starts_with("- ") {
            return None;
        }

        let preamble_block = text.split("\n\n").next()?;
        let mut edges = Vec::new();

        for raw_line in preamble_block.lines() {
            let line = raw_line.trim();
            if line.is_empty() {
                continue;
            }
            if !line.starts_with("- ") {
                return None;
            }

            let content = line[2..].trim();
            let colon_idx = content.find(':')?;
            if colon_idx == 0 {
                return None;
            }

            let key = content[..colon_idx].trim();
            let val = content[colon_idx + 1..].trim();

            if !field_key_re.is_match(key) {
                return None;
            }

            if single_ref_re.is_match(val) || multi_ref_re.is_match(val) {
                for cap in ref_extract_re.captures_iter(val) {
                    let inner = &cap[1];
                    let target_id = inner.split(':').next_back().unwrap_or("");
                    if !target_id.is_empty() {
                        edges.push((key.to_string(), target_id.to_string()));
                    }
                }
            } else {
                return None;
            }
        }

        if edges.is_empty() {
            None
        } else {
            Some(edges)
        }
    }

    /// Resolve target __global_id from target __id
    /// First tries same workspace/namespace, then searches all workspaces
    fn resolve_target_global_id(
        &self,
        target_id: &str,
        workspace: &str,
        namespace: &str,
    ) -> SqliteResult<Option<String>> {
        // First try: same workspace and namespace
        let candidate = Self::compute_global_id(workspace, namespace, target_id);
        let exists = self
            .conn
            .query_row(
                "SELECT 1 FROM objects WHERE __global_id = ?1 LIMIT 1",
                params![candidate],
                |_| Ok(1),
            )
            .optional()?
            .is_some();

        if exists {
            return Ok(Some(candidate));
        }

        // Second try: same workspace, any namespace (including empty)
        let candidate = self
            .conn
            .query_row(
                "SELECT __global_id FROM objects WHERE __workspace = ?1 AND __id = ?2 LIMIT 1",
                params![workspace, target_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?;

        if let Some(tgid) = candidate {
            return Ok(Some(tgid));
        }

        // Third try: any workspace (cross-workspace reference)
        // This handles cases where target is in different workspace
        let candidate = self
            .conn
            .query_row(
                "SELECT __global_id FROM objects WHERE __id = ?1 LIMIT 1",
                params![target_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?;

        if let Some(tgid) = candidate {
            return Ok(Some(tgid));
        }

        // Fourth try: __local_id in same namespace (short-form references like
        // [[#child]] that resolve via the __local_id fallback when unambiguous).
        // Matches Python/TS: LIMIT 2 so that 0 or >1 matches resolve to None
        // (not found or ambiguous).
        let mut stmt = self.conn.prepare(
            "SELECT __global_id FROM objects \
             WHERE __local_id = ?1 AND __workspace = ?2 AND __namespace = ?3 LIMIT 2",
        )?;
        let local_matches: Vec<String> = stmt
            .query_map(params![target_id, workspace, namespace], |row| {
                row.get::<_, String>(0)
            })?
            .collect::<SqliteResult<Vec<String>>>()?;
        if local_matches.len() == 1 {
            return Ok(Some(local_matches[0].clone()));
        }

        Ok(None)
    }

    /// Find ALL [[#id]] references in a string (handles "[[#a]], [[#b]], [[#c]]")
    /// Also handles malformed strings like `[["#auth]], [[#user-svc"]]` from parser bugs
    fn parse_all_references(&self, s: &str) -> Vec<String> {
        let mut refs = Vec::new();
        let mut i = 0;
        let chars: Vec<char> = s.chars().collect();

        while i < chars.len() {
            // Look for [[ or ["
            if i + 1 < chars.len()
                && chars[i] == '['
                && (chars[i + 1] == '[' || chars[i + 1] == '"')
            {
                // Skip [[ or ["
                i += 2;
                // Skip optional # at start
                if i < chars.len() && chars[i] == '#' {
                    i += 1;
                }
                // Collect id until ]] or "]
                let start = i;
                while i < chars.len() {
                    if chars[i] == ']' || chars[i] == '"' {
                        break;
                    }
                    i += 1;
                }
                if i > start {
                    let inner: String = chars[start..i].iter().collect();
                    // Take last part after : as the id
                    let id = inner.rsplit(':').next().unwrap_or(&inner);
                    if !id.is_empty() && !id.contains(',') && !id.contains(' ') {
                        refs.push(id.to_string());
                    }
                }
            }
            i += 1;
        }

        refs
    }

    /// Get count of objects
    pub fn object_count(&self) -> SqliteResult<i64> {
        self.conn
            .query_row("SELECT COUNT(*) FROM objects", [], |row| row.get(0))
    }

    /// Get count of edges
    pub fn edge_count(&self) -> SqliteResult<i64> {
        self.conn
            .query_row("SELECT COUNT(*) FROM edges", [], |row| row.get(0))
    }
}

/// Result of a SQL query
#[derive(Debug, Clone)]
pub struct QueryResult {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<Value>>,
}

impl QueryResult {
    /// Convert to JSON representation
    pub fn to_json(&self) -> Value {
        let rows: Vec<Value> = self
            .rows
            .iter()
            .map(|row| {
                let obj: serde_json::Map<String, Value> = self
                    .columns
                    .iter()
                    .zip(row.iter())
                    .map(|(col, val)| (col.clone(), val.clone()))
                    .collect();
                Value::Object(obj)
            })
            .collect();

        Value::Array(rows)
    }

    /// Format as text table for display (full width, no truncation)
    pub fn to_table_string(&self) -> String {
        if self.rows.is_empty() {
            return "(empty result)\n".to_string();
        }

        // Calculate column widths based on actual content
        let mut widths: Vec<usize> = self.columns.iter().map(|c| c.len()).collect();
        for row in &self.rows {
            for (i, val) in row.iter().enumerate() {
                let val_str = Self::value_to_display_string(val);
                widths[i] = widths[i].max(val_str.len());
            }
        }

        let mut output = String::new();

        // Header
        let header: Vec<String> = self
            .columns
            .iter()
            .enumerate()
            .map(|(i, c)| Self::format_cell(c, widths[i]))
            .collect();
        output.push_str(&header.join(" | "));
        output.push('\n');

        // Separator
        let sep: Vec<String> = widths.iter().map(|w| "-".repeat(*w)).collect();
        output.push_str(&sep.join("-+-"));
        output.push('\n');

        // Rows
        for row in &self.rows {
            let row_strs: Vec<String> = row
                .iter()
                .enumerate()
                .map(|(i, val)| {
                    let s = Self::value_to_display_string(val);
                    Self::format_cell(&s, widths[i])
                })
                .collect();
            output.push_str(&row_strs.join(" | "));
            output.push('\n');
        }

        output
    }

    /// Format cell content, normalizing whitespace and padding to width
    fn format_cell(s: &str, width: usize) -> String {
        // Remove newlines and extra whitespace
        let s: String = s.split_whitespace().collect::<Vec<_>>().join(" ");
        let s = s.trim();
        format!("{:width$}", s, width = width)
    }

    fn value_to_display_string(val: &Value) -> String {
        match val {
            Value::Null => "NULL".to_string(),
            Value::String(s) => s.trim().to_string(),
            Value::Number(n) => n.to_string(),
            Value::Bool(b) => b.to_string(),
            _ => val.to_string().trim().to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_query_read_only_allows_select_blocks_writes() {
        let db = QmdcDatabase::new().unwrap();
        db.upsert_object(&json!({"__id": "a", "__kind": "Note"}))
            .unwrap();

        // SELECT works under query_only.
        let res = db.query_read_only("SELECT __id FROM objects").unwrap();
        assert_eq!(res.rows.len(), 1);

        // A write that somehow reached the DB is rejected by SQLite's query_only guard.
        let err = db
            .query_read_only("DELETE FROM objects")
            .expect_err("DELETE must be rejected under query_only");
        assert!(
            err.to_lowercase().contains("only") || err.to_lowercase().contains("readonly"),
            "unexpected error: {err}"
        );

        // Pragma is restored: normal writes work again afterwards.
        db.upsert_object(&json!({"__id": "b", "__kind": "Note"}))
            .unwrap();
        assert_eq!(db.object_count().unwrap(), 2);
    }

    #[test]
    fn test_create_database() {
        let db = QmdcDatabase::new().unwrap();
        assert_eq!(db.object_count().unwrap(), 0);
        assert_eq!(db.edge_count().unwrap(), 0);
    }

    #[test]
    fn test_insert_object() {
        let db = QmdcDatabase::new().unwrap();

        let obj = json!({
            "__id": "users",
            "__kind": "Table",
            "__label": "Users Table",
            "__namespace": "storage",
            "name": "users",
            "columns": ["id", "email", "name"]
        });

        db.upsert_object(&obj).unwrap();
        assert_eq!(db.object_count().unwrap(), 1);

        let result = db
            .query("SELECT __id, __kind, __label FROM objects")
            .unwrap();
        assert_eq!(result.rows.len(), 1);
        assert_eq!(result.rows[0][0], json!("users"));
        assert_eq!(result.rows[0][1], json!("Table"));
    }

    #[test]
    fn test_json_extract() {
        let db = QmdcDatabase::new().unwrap();

        let obj = json!({
            "__id": "users",
            "__kind": "Table",
            "name": "users_table",
            "description": "Main users"
        });

        db.upsert_object(&obj).unwrap();

        let result = db
            .query("SELECT __id, json_extract(data, '$.name') as name FROM objects")
            .unwrap();

        assert_eq!(result.rows.len(), 1);
        assert_eq!(result.rows[0][1], json!("users_table"));
    }

    #[test]
    fn test_insert_edges() {
        let db = QmdcDatabase::new().unwrap();

        let obj1 = json!({
            "__id": "orders",
            "__kind": "Table",
            "user_ref": "[[#users]]"
        });

        let obj2 = json!({
            "__id": "users",
            "__kind": "Table"
        });

        db.upsert_object(&obj1).unwrap();
        db.upsert_object(&obj2).unwrap();
        db.extract_and_insert_edges("orders", &obj1).unwrap();

        assert_eq!(db.edge_count().unwrap(), 1);

        // Use JOIN to get __id from __global_id (edges.source_id/target_id contain __global_id)
        let result = db
            .query(
                "SELECT s.__id, e.source_field, t.__id, e.edge_type
             FROM edges e
             JOIN objects s ON e.source_id = s.__global_id
             JOIN objects t ON e.target_id = t.__global_id",
            )
            .unwrap();

        assert_eq!(result.rows[0][0], json!("orders"));
        assert_eq!(result.rows[0][1], json!("user_ref"));
        assert_eq!(result.rows[0][2], json!("users"));
        // For inline fields, edge_type defaults to source_field
        assert_eq!(result.rows[0][3], json!("user_ref"));
    }

    #[test]
    fn test_edges_join() {
        let db = QmdcDatabase::new().unwrap();

        let users = json!({"__id": "users", "__kind": "Table", "__label": "Users"});
        let orders = json!({
            "__id": "orders",
            "__kind": "Table",
            "__label": "Orders",
            "user_ref": "[[#users]]"
        });

        db.upsert_object(&users).unwrap();
        db.upsert_object(&orders).unwrap();
        db.extract_and_insert_edges("orders", &orders).unwrap();

        // Use JOIN to get __id from __global_id (edges.source_id/target_id contain __global_id)
        let result = db
            .query(
                r#"
            SELECT o.__id, o.__label, t.__id, e.edge_type
            FROM objects o 
            JOIN edges e ON o.__global_id = e.source_id
            JOIN objects t ON e.target_id = t.__global_id
            "#,
            )
            .unwrap();

        assert_eq!(result.rows.len(), 1);
        assert_eq!(result.rows[0][0], json!("orders"));
        assert_eq!(result.rows[0][2], json!("users"));
        // For inline fields, edge_type defaults to source_field
        assert_eq!(result.rows[0][3], json!("user_ref"));
    }

    #[test]
    fn test_query_result_to_table() {
        let result = QueryResult {
            columns: vec!["id".to_string(), "name".to_string()],
            rows: vec![
                vec![json!("1"), json!("Alice")],
                vec![json!("2"), json!("Bob")],
            ],
        };

        let table = result.to_table_string();
        assert!(table.contains("id"));
        assert!(table.contains("Alice"));
        assert!(table.contains("Bob"));
    }

    #[test]
    fn test_level_stored_in_sqlite() {
        let db = QmdcDatabase::new().unwrap();

        // Object at level 1
        let obj1 = json!({
            "__id": "root",
            "__kind": "__Namespace",
            "__label": "Root",
            "__level": 1
        });

        // Object at level 2
        let obj2 = json!({
            "__id": "parent",
            "__kind": "Container",
            "__label": "Parent",
            "__level": 2
        });

        // Object at level 3
        let obj3 = json!({
            "__id": "child",
            "__kind": "Item",
            "__label": "Child",
            "__level": 3
        });

        db.upsert_object(&obj1).unwrap();
        db.upsert_object(&obj2).unwrap();
        db.upsert_object(&obj3).unwrap();

        // Query __level column
        let result = db
            .query("SELECT __id, __level FROM objects ORDER BY __level")
            .unwrap();

        assert_eq!(result.rows.len(), 3);
        assert_eq!(result.rows[0][0], json!("root"));
        assert_eq!(result.rows[0][1], json!(1));
        assert_eq!(result.rows[1][0], json!("parent"));
        assert_eq!(result.rows[1][1], json!(2));
        assert_eq!(result.rows[2][0], json!("child"));
        assert_eq!(result.rows[2][1], json!(3));
    }
}
