//! Core `query` operation — executes a read-only SQL query against the resolved index.
//!
//! Enforces INV-2 (read-only SQL) via `sql_guard`, then executes the query
//! against the in-memory `QmdcDatabase`. Results are wrapped in a `BoundedEnvelope`
//! (NFR-4: never silently drop rows).

use serde_json::{json, Value};

use crate::core::envelope::BoundedEnvelope;
use crate::core::error::{ErrorCode, ErrorEnvelope};
use crate::core::fields::QmdcObject;
use crate::core::log::{core_log, EventCategory, Severity};
use crate::core::resolved_index::ResolvedIndex;
use crate::core::sql_guard::assert_read_only;

/// Execute a read-only SQL query against the resolved index.
///
/// # Invariants enforced
/// - **INV-2**: Only SELECT-class statements are admitted. Everything else is
///   rejected with `not-read-only` before any SQL reaches the database.
///
/// # Returns
/// - `Ok(Value)` — success envelope with `BoundedEnvelope` shape (items, truncated, remaining).
/// - `Err(Value)` — error envelope (`not-read-only`, `internal-error`).
pub fn query(
    index: &ResolvedIndex,
    sql: &str,
    limit: usize,
    offset: usize,
) -> Result<Value, Value> {
    // Resolve a `#query_id` reference to the stored SQL of a `Query` object (parity with
    // the LSP `qmdc.runSqlQuery` command, which resolves the same way).
    let resolved_sql = if let Some(query_id) = sql.trim().strip_prefix('#') {
        match resolve_query_id(index, query_id) {
            Some(s) => s,
            None => {
                return Err(ErrorEnvelope::error(
                    ErrorCode::NotFound,
                    format!(
                        "Query object '{}' not found or has no 'sql' field",
                        query_id
                    ),
                ))
            }
        }
    } else {
        sql.to_string()
    };
    let sql = resolved_sql.as_str();

    // INV-2: fail-closed read-only check
    assert_read_only(sql)?;

    core_log(
        EventCategory::Query,
        Severity::Info,
        &format!("executing query: {}", truncate_for_log(sql, 120)),
    );

    // Execute against the in-memory database (read-only enforced at the SQLite layer too).
    let result = index.db.query_read_only(sql).map_err(|e| {
        // SQLite's `query_only` guard reports a blocked write as "attempt to write a readonly
        // database". That is a read-only violation, not an internal failure — surface it under
        // the accurate `not-read-only` code (this is the engine layer of the INV-2 guard,
        // catching writes that `sqlparser` couldn't pre-classify, e.g. GLOB/MATCH WHERE-clauses).
        let lower = e.to_lowercase();
        if lower.contains("readonly") || lower.contains("read-only") || lower.contains("read only")
        {
            ErrorEnvelope::error(
                ErrorCode::NotReadOnly,
                "statement is not read-only (rejected by the database engine)",
            )
        } else {
            ErrorEnvelope::error(
                ErrorCode::InternalError,
                format!("query execution error: {}", e),
            )
        }
    })?;

    // Convert rows to JSON values for the envelope
    let row_values: Vec<Value> = result
        .rows
        .iter()
        .map(|row| {
            let obj: serde_json::Map<String, Value> = result
                .columns
                .iter()
                .zip(row.iter())
                .map(|(col, val)| (col.clone(), val.clone()))
                .collect();
            Value::Object(obj)
        })
        .collect();

    // Apply bounded envelope (NFR-4)
    let envelope = BoundedEnvelope::from_items(row_values, limit, offset);

    // Wrap in success envelope with columns metadata
    let mut payload = envelope.to_value();
    if let Some(obj) = payload.as_object_mut() {
        obj.insert("columns".to_string(), json!(result.columns));
    }

    Ok(ErrorEnvelope::success(payload))
}

/// Resolve a `#query_id` to the `sql` field of the matching `Query` object in the index.
fn resolve_query_id(index: &ResolvedIndex, query_id: &str) -> Option<String> {
    index.objects().iter().find_map(|obj| {
        let id = obj.get("__id").and_then(|v| v.as_str())?;
        let kind = obj.kind();
        if id == query_id && kind == "Query" {
            obj.get("sql")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        } else {
            None
        }
    })
}

/// Truncate a string for log output, appending "..." if truncated.
///
/// Counts and slices by **characters**, never bytes, so it can never panic on a
/// multibyte UTF-8 boundary (the `sql` argument is fully caller-controlled).
fn truncate_for_log(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max).collect();
        format!("{}...", truncated)
    }
}

#[cfg(test)]
mod tests {
    use super::query;
    use crate::core::index_seam::get_index;
    use crate::core::resolved_index::ResolvedIndex;
    use serde_json::Value;

    /// Build a real in-memory index from a temp workspace (readme marker + one data file).
    fn index_for(data: &str) -> (tempfile::TempDir, ResolvedIndex) {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("readme.qmd.md"),
            "# Workspace [[ws: __Workspace]]\n",
        )
        .unwrap();
        std::fs::write(tmp.path().join("data.qmd.md"), data).unwrap();
        let index = get_index(tmp.path()).expect("index built");
        (tmp, index)
    }

    fn items(result: &Value) -> Vec<Value> {
        result["items"].as_array().cloned().unwrap_or_default()
    }

    const FEATURES: &str = "# Features\n\
        \n\
        ## Feature A [[feat_a: Feature]]\n\
        \n\
        - qmd1_status: done\n\
        - qmd2_status: planned\n\
        - note: hello\n\
        \n\
        ## Feature B [[feat_b: Feature]]\n\
        \n\
        - qmd1_status: planned\n";

    /// The reported bug: a valid read-only query using `GLOB` + `json_each` (SQLite syntax that
    /// sqlparser cannot parse) must run and return correct rows — not be rejected as
    /// `not-read-only`.
    #[test]
    fn glob_json_each_query_runs_and_returns_correct_rows() {
        let (_tmp, index) = index_for(FEATURES);
        let sql = "SELECT key, COUNT(*) c FROM objects, json_each(objects.data) \
                   WHERE key GLOB 'qmd[0-9]*' GROUP BY key ORDER BY key";
        let result = query(&index, sql, 200, 0).expect("GLOB/json_each query should succeed");
        assert_eq!(result["success"], true, "envelope: {result}");

        let rows = items(&result);
        // qmd1_status appears on both features (2), qmd2_status only on A (1); `note` excluded.
        assert_eq!(rows.len(), 2, "rows: {rows:?}");
        assert_eq!(rows[0]["key"], "qmd1_status");
        assert_eq!(rows[0]["c"], 2);
        assert_eq!(rows[1]["key"], "qmd2_status");
        assert_eq!(rows[1]["c"], 1);
    }

    /// A plain SELECT still works (sanity that the engine path is wired through the guard).
    #[test]
    fn plain_select_runs() {
        let (_tmp, index) = index_for(FEATURES);
        let result = query(
            &index,
            "SELECT __id FROM objects WHERE __kind='Feature' ORDER BY __id",
            200,
            0,
        )
        .unwrap();
        let rows = items(&result);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0]["__id"], "feat_a");
        assert_eq!(rows[1]["__id"], "feat_b");
    }

    fn object_count(index: &ResolvedIndex) -> i64 {
        let r = query(index, "SELECT COUNT(*) n FROM objects", 200, 0).unwrap();
        items(&r)[0]["n"].as_i64().unwrap()
    }

    /// A parseable write is rejected by the sqlparser pre-filter (never reaches the engine),
    /// and the data is untouched.
    #[test]
    fn delete_is_rejected_and_does_not_mutate() {
        let (_tmp, index) = index_for(FEATURES);
        let before = object_count(&index);

        let err = query(&index, "DELETE FROM objects", 200, 0).unwrap_err();
        assert_eq!(err["error"]["code"], "not-read-only", "err: {err}");

        assert_eq!(object_count(&index), before, "DELETE must not mutate");
    }

    /// The key engine-backstop case: a write WRAPPED in a CTE and using `GLOB` is unparseable by
    /// sqlparser AND starts as a read query (`WITH`), so it is deferred to SQLite — where the
    /// `query_only` guard blocks the write. It must error as `not-read-only` AND leave the DB
    /// unchanged (proving the write never executed).
    #[test]
    fn cte_wrapped_write_blocked_by_engine_without_mutation() {
        let (_tmp, index) = index_for(FEATURES);
        let before = object_count(&index);

        let sql = "WITH t AS (SELECT 1) DELETE FROM objects WHERE __id GLOB 'feat*'";
        let err = query(&index, sql, 200, 0).unwrap_err();
        assert_eq!(
            err["error"]["code"], "not-read-only",
            "engine should reject the CTE-wrapped write: {err}"
        );

        assert_eq!(
            object_count(&index),
            before,
            "no rows may be deleted by a blocked write"
        );
    }

    /// `PRAGMA query_only=OFF` (an attempt to disable the guard) is unparseable by sqlparser and
    /// does NOT start as a read query, so it is rejected at the pre-filter — it never reaches the
    /// engine to flip the pragma.
    #[test]
    fn pragma_to_disable_guard_is_rejected() {
        let (_tmp, index) = index_for(FEATURES);
        let err = query(&index, "PRAGMA query_only=OFF", 200, 0).unwrap_err();
        assert_eq!(err["error"]["code"], "not-read-only", "err: {err}");
    }

    /// `#query_id` resolves to a stored Query object's `sql` and executes it.
    #[test]
    fn query_id_reference_resolves_and_runs() {
        let data = "# Queries\n\
            \n\
            ## Count [[count_features: Query]]\n\
            \n\
            - sql: SELECT COUNT(*) n FROM objects WHERE __kind='Query'\n";
        let (_tmp, index) = index_for(data);
        let result = query(&index, "#count_features", 200, 0).expect("stored query should run");
        let rows = items(&result);
        assert_eq!(rows[0]["n"], 1);

        // Unknown id is a clear not-found, not a silent empty result.
        let err = query(&index, "#nonexistent", 200, 0).unwrap_err();
        assert_eq!(err["error"]["code"], "not-found");
    }
}
