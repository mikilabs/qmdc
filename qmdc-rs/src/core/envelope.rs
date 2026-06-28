//! Bounded-result / truncation envelope (data-models D3, NFR-4).
//!
//! Applied uniformly to every heavy/list output: `tree`, `query`, `traverse`, `search`.
//! Results are **never silently dropped** — the envelope always reports `remaining` when
//! truncation occurs.

use serde_json::{json, Value};

/// Default limit for bounded results. Per-tool overrides may apply at the op level.
pub const DEFAULT_LIMIT: usize = 200;

// ---------------------------------------------------------------------------
// Keyset (cursor) pagination
// ---------------------------------------------------------------------------

/// A keyset-paginated page: the items, whether more remain, and the cursor to fetch them.
#[derive(Debug, Clone)]
pub struct CursorPage {
    pub items: Vec<Value>,
    pub total: usize,
    pub truncated: bool,
    pub next_cursor: Option<String>,
}

/// Keyset-paginate `keyed` (each entry is `(sort_key, item)`). Sorts by `sort_key` ascending,
/// skips entries whose key is `<= cursor`, and returns up to `limit` items.
///
/// The cursor is the stable sort key itself (we use `__global_id` for all paginated tools) —
/// readable and reviewable, treated as opaque by clients (they echo `next_cursor` back
/// verbatim). `next_cursor` is the key of the LAST returned item when more remain, so the
/// next call resumes strictly after it. Unlike offset/skip-take this is robust to
/// inserts/deletes outside the page boundary — no duplicated or skipped items.
pub fn cursor_page(
    mut keyed: Vec<(String, Value)>,
    limit: usize,
    cursor: Option<&str>,
) -> CursorPage {
    keyed.sort_by(|a, b| a.0.cmp(&b.0));
    let total = keyed.len();
    let start = match cursor {
        Some(c) => keyed.partition_point(|(key, _)| key.as_str() <= c),
        None => 0,
    };
    let rest = &keyed[start..];
    let truncated = rest.len() > limit;
    let take = rest.len().min(limit);
    let items: Vec<Value> = rest[..take].iter().map(|(_, v)| v.clone()).collect();
    // Guard `take - 1`: only emit a cursor when we actually returned something AND more remain.
    let next_cursor = if truncated {
        take.checked_sub(1)
            .and_then(|i| rest.get(i))
            .map(|(k, _)| k.clone())
    } else {
        None
    };
    CursorPage {
        items,
        total,
        truncated,
        next_cursor,
    }
}

/// A bounded envelope that wraps a list of items with pagination/truncation metadata.
///
/// Shape: `{ items, limit, offset, truncated, remaining }`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundedEnvelope {
    /// The (possibly truncated) list of result items.
    pub items: Vec<Value>,
    /// The limit that was applied.
    pub limit: usize,
    /// The offset from which items were taken.
    pub offset: usize,
    /// Whether the result was truncated.
    pub truncated: bool,
    /// Number of items beyond the limit (0 when not truncated).
    pub remaining: usize,
}

impl BoundedEnvelope {
    /// Construct a bounded envelope from a full list of items.
    ///
    /// Applies `limit` and `offset`, reporting truncation and remaining count.
    /// Items beyond `limit` are never silently dropped — `remaining` is always set.
    pub fn from_items(all_items: Vec<Value>, limit: usize, offset: usize) -> Self {
        let total = all_items.len();
        let start = offset.min(total);
        let available = total.saturating_sub(start);

        let (items, truncated, remaining) = if available > limit {
            (
                all_items[start..start + limit].to_vec(),
                true,
                available - limit,
            )
        } else {
            (all_items[start..].to_vec(), false, 0)
        };

        Self {
            items,
            limit,
            offset,
            truncated,
            remaining,
        }
    }

    /// Convenience: construct with default limit and zero offset.
    pub fn with_default_limit(all_items: Vec<Value>) -> Self {
        Self::from_items(all_items, DEFAULT_LIMIT, 0)
    }

    /// Serialize this envelope to a `serde_json::Value`.
    pub fn to_value(&self) -> Value {
        let mut obj = json!({
            "items": self.items,
            "limit": self.limit,
            "offset": self.offset,
            "truncated": self.truncated,
        });
        if self.truncated {
            obj["remaining"] = json!(self.remaining);
        }
        obj
    }
}

/// Bound a semantically-named list (NFR-4) without renaming it to the generic `items`.
///
/// Caps `all_items` to `limit` and reports truncation. Returns `(items, truncated, remaining)`
/// where `remaining` is the overflow count (0 when not truncated). Items beyond `limit` are
/// never silently dropped — callers surface `truncated`/`remaining` alongside the domain key
/// (`diagnostics`, `references`, `edits`, …). Single source for the "bounded named list" shape
/// used by ops that keep a meaningful key name instead of `BoundedEnvelope`'s `items`.
pub fn bound_list(all_items: Vec<Value>, limit: usize) -> (Vec<Value>, bool, usize) {
    let total = all_items.len();
    if total > limit {
        let items: Vec<Value> = all_items.into_iter().take(limit).collect();
        (items, true, total - limit)
    } else {
        (all_items, false, 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cursor_page_first_page_truncates() {
        let keyed: Vec<(String, Value)> = (0..5)
            .map(|i| (format!("k{i}"), json!({ "n": i })))
            .collect();
        let page = cursor_page(keyed, 2, None);
        assert_eq!(page.items.len(), 2);
        assert!(page.truncated);
        assert_eq!(page.items[0]["n"], 0);
        assert_eq!(page.items[1]["n"], 1);
        // next_cursor is the last returned key ("k1"), readable and reviewable.
        assert_eq!(page.next_cursor.as_deref(), Some("k1"));
    }

    #[test]
    fn cursor_page_resumes_after_cursor() {
        let keyed: Vec<(String, Value)> = (0..5)
            .map(|i| (format!("k{i}"), json!({ "n": i })))
            .collect();
        let page = cursor_page(keyed, 2, Some("k1"));
        assert_eq!(page.items[0]["n"], 2, "must resume strictly after k1");
        assert_eq!(page.items[1]["n"], 3);
        assert!(page.truncated);
    }

    #[test]
    fn cursor_page_last_page_not_truncated() {
        let keyed: Vec<(String, Value)> = (0..3)
            .map(|i| (format!("k{i}"), json!({ "n": i })))
            .collect();
        let page = cursor_page(keyed, 10, Some("k0"));
        assert_eq!(page.items.len(), 2);
        assert!(!page.truncated);
        assert!(page.next_cursor.is_none());
    }

    #[test]
    fn cursor_page_robust_to_deletion_outside_boundary() {
        // Page 1 over k0..k4 with limit 2 -> [k0,k1], cursor=k1.
        // Delete k0 (before the cursor) and re-page: must still resume after k1 -> [k2,k3].
        let keyed: Vec<(String, Value)> =
            (1..5) // k0 deleted
                .map(|i| (format!("k{i}"), json!({ "n": i })))
                .collect();
        let page = cursor_page(keyed, 2, Some("k1"));
        assert_eq!(page.items[0]["n"], 2);
        assert_eq!(page.items[1]["n"], 3);
    }

    #[test]
    fn bound_list_caps_and_reports_remaining() {
        let items: Vec<Value> = (0..250).map(|i| json!(i)).collect();
        let (capped, truncated, remaining) = bound_list(items, DEFAULT_LIMIT);
        assert_eq!(capped.len(), DEFAULT_LIMIT);
        assert!(truncated);
        assert_eq!(remaining, 50);
    }

    #[test]
    fn bound_list_under_limit_is_untouched() {
        let items: Vec<Value> = (0..5).map(|i| json!(i)).collect();
        let (capped, truncated, remaining) = bound_list(items, DEFAULT_LIMIT);
        assert_eq!(capped.len(), 5);
        assert!(!truncated);
        assert_eq!(remaining, 0);
    }

    #[test]
    fn within_limit_is_not_truncated() {
        let items: Vec<Value> = (0..5).map(|i| json!(i)).collect();
        let envelope = BoundedEnvelope::from_items(items.clone(), 10, 0);

        assert_eq!(envelope.items.len(), 5);
        assert!(!envelope.truncated);
        assert_eq!(envelope.remaining, 0);
        assert_eq!(envelope.limit, 10);
        assert_eq!(envelope.offset, 0);
    }

    #[test]
    fn exceeding_limit_is_truncated_with_correct_remaining() {
        let items: Vec<Value> = (0..15).map(|i| json!(i)).collect();
        let envelope = BoundedEnvelope::from_items(items, 10, 0);

        assert_eq!(envelope.items.len(), 10);
        assert!(envelope.truncated);
        assert_eq!(envelope.remaining, 5);
        assert_eq!(envelope.items[0], json!(0));
        assert_eq!(envelope.items[9], json!(9));
    }

    #[test]
    fn offset_skips_items_correctly() {
        let items: Vec<Value> = (0..20).map(|i| json!(i)).collect();
        let envelope = BoundedEnvelope::from_items(items, 5, 10);

        assert_eq!(envelope.items.len(), 5);
        assert!(envelope.truncated);
        assert_eq!(envelope.remaining, 5); // 20 - 10 offset = 10 available, 10 - 5 limit = 5 remaining
        assert_eq!(envelope.items[0], json!(10));
        assert_eq!(envelope.items[4], json!(14));
    }

    #[test]
    fn offset_beyond_total_yields_empty() {
        let items: Vec<Value> = (0..5).map(|i| json!(i)).collect();
        let envelope = BoundedEnvelope::from_items(items, 10, 100);

        assert!(envelope.items.is_empty());
        assert!(!envelope.truncated);
        assert_eq!(envelope.remaining, 0);
    }

    #[test]
    fn exact_limit_is_not_truncated() {
        let items: Vec<Value> = (0..10).map(|i| json!(i)).collect();
        let envelope = BoundedEnvelope::from_items(items, 10, 0);

        assert_eq!(envelope.items.len(), 10);
        assert!(!envelope.truncated);
        assert_eq!(envelope.remaining, 0);
    }

    #[test]
    fn to_value_includes_remaining_only_when_truncated() {
        // Not truncated
        let items: Vec<Value> = (0..3).map(|i| json!(i)).collect();
        let envelope = BoundedEnvelope::from_items(items, 10, 0);
        let val = envelope.to_value();
        assert_eq!(val["truncated"], json!(false));
        assert!(val.get("remaining").is_none());

        // Truncated
        let items: Vec<Value> = (0..15).map(|i| json!(i)).collect();
        let envelope = BoundedEnvelope::from_items(items, 10, 0);
        let val = envelope.to_value();
        assert_eq!(val["truncated"], json!(true));
        assert_eq!(val["remaining"], json!(5));
    }

    #[test]
    fn default_limit_applies_correctly() {
        let items: Vec<Value> = (0..250).map(|i| json!(i)).collect();
        let envelope = BoundedEnvelope::with_default_limit(items);

        assert_eq!(envelope.limit, DEFAULT_LIMIT);
        assert_eq!(envelope.items.len(), DEFAULT_LIMIT);
        assert!(envelope.truncated);
        assert_eq!(envelope.remaining, 50);
    }
}
