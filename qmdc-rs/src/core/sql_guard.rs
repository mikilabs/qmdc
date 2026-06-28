//! SQL read-only guard (INV-2 enforcement).
//!
//! Two layers enforce read-only access, with SQLite as the authority:
//!   1. A `sqlparser` pre-filter ([`classify_read_only`]) rejects statements it can *positively*
//!      confirm are writes/non-queries (INSERT, DELETE, ATTACH, PRAGMA, multi-statement, …).
//!   2. The engine-level guard — SQLite's `PRAGMA query_only` (see `db::query_read_only`) —
//!      is the authoritative gate: it executes read-only statements and fail-closed rejects any
//!      write, regardless of whether the parser understood the SQL.
//!
//! Because `sqlparser`'s grammar is a *subset* of SQLite's (it has no `GLOB`/`MATCH`, etc.), a
//! parse failure is NOT evidence of a write. Treating it as one would falsely reject valid
//! read-only queries (and mislabel a parser error as `not-read-only`). So the pre-filter only
//! *rejects* confirmed writes; unparseable SQL is deferred to the engine guard, not rejected.
//! This does not relax safety — `query_only` still blocks every write at execution time.
//! Rejections are logged as `security-rejection` (A09 auditability).

use serde_json::Value;

use super::error::{ErrorCode, ErrorEnvelope};
use super::log::{core_log, EventCategory, Severity};
use crate::lsp::sql_rewrite::{classify_read_only, ReadOnlyClass};

/// Pre-filter `sql` for read-only access (INV-2), layer 1 of the guard.
///
/// # Returns
/// * `Ok(())` — admitted: either confirmed read-only, or unparseable-by-`sqlparser` and thus
///   deferred to the engine-level `query_only` guard (which still rejects any write).
/// * `Err(Value)` — a `not-read-only` error envelope for a statement confirmed to be a
///   write/non-query. Logged as a `security-rejection`.
pub fn assert_read_only(sql: &str) -> Result<(), Value> {
    match classify_read_only(sql) {
        ReadOnlyClass::ReadOnly => Ok(()),
        ReadOnlyClass::Writable(reason) => {
            core_log(
                EventCategory::SecurityRejection,
                Severity::Security,
                &format!("INV-2 read-only denied: {}", reason),
            );
            Err(ErrorEnvelope::error(ErrorCode::NotReadOnly, reason))
        }
        ReadOnlyClass::Unparseable(parse_err) => {
            // Not a write — sqlparser simply can't parse this (its grammar ⊂ SQLite's, e.g.
            // GLOB/MATCH). Defer to SQLite's `query_only` guard, which executes read-only SQL
            // and rejects any write at the engine. Logged informationally, not as a rejection.
            core_log(
                EventCategory::Query,
                Severity::Info,
                &format!(
                    "read-only pre-check deferred to engine query_only (unparseable by sqlparser): {}",
                    parse_err
                ),
            );
            Ok(())
        }
    }
}
