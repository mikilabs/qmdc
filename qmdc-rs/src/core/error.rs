//! Typed error codes and error-as-data envelope for Core operations.
//!
//! Every Core operation returns `Result<serde_json::Value, serde_json::Value>` at its boundary,
//! where both variants are in-band envelopes (never transport faults). This module provides
//! the 8 logical error codes (exhaustive enum) and helpers to construct the envelope shapes.

use serde_json::{json, Value};

/// The 8 logical error categories (cross-cutting §5).
///
/// Using a typed enum ensures exhaustive, fail-closed handling — any new error category
/// requires updating all match sites.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCode {
    /// A resolved path escapes the resolved workspace root (INV-1 / FR-13).
    OutOfRoot,
    /// Path resolves to no workspace, or points at a non-existent location (FR-12).
    NotResolved,
    /// A query argument is not a read-only, select-class statement (INV-2 / FR-14).
    NotReadOnly,
    /// An untrusted argument fails boundary validation.
    InvalidArgument,
    /// A well-formed ref/id resolves to no object.
    NotFound,
    /// `find_path` finds no connecting path (explicit, clearly-marked result).
    NoPath,
    /// Per-call reparse exceeds the NFR-2 bound.
    ReparseBoundExceeded,
    /// Any other failure, surfaced as data rather than a crash.
    InternalError,
}

impl ErrorCode {
    /// Stable string representation used in the `error.code` field of the envelope.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::OutOfRoot => "out-of-root",
            Self::NotResolved => "not-resolved",
            Self::NotReadOnly => "not-read-only",
            Self::InvalidArgument => "invalid-argument",
            Self::NotFound => "not-found",
            Self::NoPath => "no-path",
            Self::ReparseBoundExceeded => "reparse-bound-exceeded",
            Self::InternalError => "internal-error",
        }
    }
}

/// Helper to build error-as-data and success envelopes.
#[derive(Debug, Clone)]
pub struct ErrorEnvelope;

impl ErrorEnvelope {
    /// Construct the canonical error envelope: `{ "success": false, "error": { "code": …, "message": … } }`.
    pub fn error(code: ErrorCode, message: impl Into<String>) -> Value {
        json!({
            "success": false,
            "error": {
                "code": code.as_str(),
                "message": message.into()
            }
        })
    }

    /// Construct a success envelope wrapping an arbitrary payload.
    ///
    /// The payload's fields are merged at the top level alongside `"success": true`.
    pub fn success(payload: Value) -> Value {
        match payload {
            Value::Object(mut map) => {
                map.insert("success".to_string(), Value::Bool(true));
                Value::Object(map)
            }
            other => {
                json!({
                    "success": true,
                    "result": other
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_envelope_has_correct_shape_for_all_codes() {
        let codes = [
            (ErrorCode::OutOfRoot, "out-of-root"),
            (ErrorCode::NotResolved, "not-resolved"),
            (ErrorCode::NotReadOnly, "not-read-only"),
            (ErrorCode::InvalidArgument, "invalid-argument"),
            (ErrorCode::NotFound, "not-found"),
            (ErrorCode::NoPath, "no-path"),
            (ErrorCode::ReparseBoundExceeded, "reparse-bound-exceeded"),
            (ErrorCode::InternalError, "internal-error"),
        ];

        for (code, expected_str) in codes {
            let envelope = ErrorEnvelope::error(code, format!("test message for {expected_str}"));
            assert_eq!(envelope["success"], json!(false));
            assert_eq!(envelope["error"]["code"], json!(expected_str));
            assert_eq!(
                envelope["error"]["message"],
                json!(format!("test message for {expected_str}"))
            );
        }
    }

    #[test]
    fn success_envelope_merges_object_payload() {
        let payload = json!({"items": [1, 2, 3], "count": 3});
        let envelope = ErrorEnvelope::success(payload);
        assert_eq!(envelope["success"], json!(true));
        assert_eq!(envelope["items"], json!([1, 2, 3]));
        assert_eq!(envelope["count"], json!(3));
    }

    #[test]
    fn success_envelope_wraps_non_object_payload() {
        let payload = json!([1, 2, 3]);
        let envelope = ErrorEnvelope::success(payload);
        assert_eq!(envelope["success"], json!(true));
        assert_eq!(envelope["result"], json!([1, 2, 3]));
    }

    #[test]
    fn error_code_as_str_is_stable() {
        // Ensure string representations don't accidentally drift
        assert_eq!(ErrorCode::OutOfRoot.as_str(), "out-of-root");
        assert_eq!(ErrorCode::NotResolved.as_str(), "not-resolved");
        assert_eq!(ErrorCode::NotReadOnly.as_str(), "not-read-only");
        assert_eq!(ErrorCode::InvalidArgument.as_str(), "invalid-argument");
        assert_eq!(ErrorCode::NotFound.as_str(), "not-found");
        assert_eq!(ErrorCode::NoPath.as_str(), "no-path");
        assert_eq!(
            ErrorCode::ReparseBoundExceeded.as_str(),
            "reparse-bound-exceeded"
        );
        assert_eq!(ErrorCode::InternalError.as_str(), "internal-error");
    }
}
