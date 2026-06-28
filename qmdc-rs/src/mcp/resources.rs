//! MCP resource providers — `qmdc://` URI scheme (rmcp-typed).
//!
//! Resources:
//! - `qmdc://guide` — static, build-embedded QMDC agent guide (FR-19)
//! - `qmdc://tree` — dynamic keyset-paginated node stream (FR-20)
//! - `qmdc://object/<id>` — dynamic object description (FR-20)
//! - `qmdc://diagnostics` — dynamic workspace validation (FR-20)
//!
//! Standard MCP `resources/read` carries only a `uri`. Dynamic resources need a workspace
//! `path`, which callers supply as a `?path=<percent-encoded>` query suffix on the URI
//! (e.g. `qmdc://tree?path=/work/space`). The static guide needs no path.

use rmcp::model::{AnnotateAble, RawResource, ReadResourceResult, Resource, ResourceContents};

use crate::core::error::{ErrorCode, ErrorEnvelope};
use crate::core::guide::GUIDE_CONTENT;
use crate::core::index_seam::{enforce_force_root, get_index, resolve_root};
use crate::core::ops::{describe, tree, validate};

/// Build the static resource catalogue for `resources/list`.
pub fn resource_list() -> Vec<Resource> {
    vec![
        RawResource {
            uri: "qmdc://guide".to_string(),
            name: "QMDC Agent Guide".to_string(),
            description: Some(
                "Complete QMD.md format guide for AI agents (static, build-embedded)".to_string(),
            ),
            mime_type: Some("text/markdown".to_string()),
            size: None,
            title: None,
            icons: None,
        }
        .no_annotation(),
        RawResource {
            uri: "qmdc://tree".to_string(),
            name: "Workspace Tree".to_string(),
            description: Some(
                "Workspace node stream (keyset-paginated, flat). Append ?path=<dir> to select the workspace."
                    .to_string(),
            ),
            mime_type: Some("application/json".to_string()),
            size: None,
            title: None,
            icons: None,
        }
        .no_annotation(),
        RawResource {
            uri: "qmdc://object/{id}".to_string(),
            name: "Object Description".to_string(),
            description: Some(
                "Full object card for a given ID. URI: qmdc://object/<id>?path=<dir>.".to_string(),
            ),
            mime_type: Some("application/json".to_string()),
            size: None,
            title: None,
            icons: None,
        }
        .no_annotation(),
        RawResource {
            uri: "qmdc://diagnostics".to_string(),
            name: "Workspace Diagnostics".to_string(),
            description: Some(
                "Broken-link validation diagnostics. Append ?path=<dir> to select the workspace."
                    .to_string(),
            ),
            mime_type: Some("application/json".to_string()),
            size: None,
            title: None,
            icons: None,
        }
        .no_annotation(),
    ]
}

/// Handle `resources/read` — dispatch by URI (with optional `?path=` query suffix).
pub fn read_resource(uri: &str) -> ReadResourceResult {
    let (base, path) = split_uri(uri);

    let contents = match base.as_str() {
        "qmdc://guide" => text_md(uri, GUIDE_CONTENT.to_string()),
        "qmdc://tree" => read_dynamic(uri, path.as_deref(), |index| {
            tree(index, None, 200, None).unwrap_or_else(|e| e)
        }),
        "qmdc://diagnostics" => read_dynamic(uri, path.as_deref(), |index| {
            validate(index, None).unwrap_or_else(|e| e)
        }),
        _ if base.starts_with("qmdc://object/") => {
            let raw = &base["qmdc://object/".len()..];
            let id = percent_decode(raw);
            let id = id.trim();
            if id.is_empty() || id.len() > 4096 {
                error_json(
                    uri,
                    &ErrorEnvelope::error(
                        ErrorCode::InvalidArgument,
                        "object id must be a non-empty id of at most 4096 characters",
                    ),
                )
            } else {
                let id = id.to_string();
                read_dynamic(uri, path.as_deref(), move |index| {
                    describe(index, &id).unwrap_or_else(|e| e)
                })
            }
        }
        _ => error_json(
            uri,
            &ErrorEnvelope::error(
                ErrorCode::InvalidArgument,
                format!("unknown resource: {}", base),
            ),
        ),
    };

    ReadResourceResult {
        contents: vec![contents],
    }
}

/// Split a resource URI into its base and an optional `path` query value.
/// `qmdc://tree?path=/work` → (`qmdc://tree`, Some("/work")). Percent-decodes the path value.
fn split_uri(uri: &str) -> (String, Option<String>) {
    match uri.split_once('?') {
        None => (uri.to_string(), None),
        Some((base, query)) => {
            let path = query
                .split('&')
                .find_map(|kv| kv.strip_prefix("path=").map(percent_decode));
            (base.to_string(), path)
        }
    }
}

/// Shared dynamic-resource reader: validate `path`, enforce the force-root boundary (INV-1),
/// resolve the workspace root, build the index, run `op`, and serialize the JSON result.
fn read_dynamic<F>(uri: &str, path: Option<&str>, op: F) -> ResourceContents
where
    F: FnOnce(&crate::core::resolved_index::ResolvedIndex) -> serde_json::Value,
{
    let path = match path {
        Some(p) if !p.is_empty() => p,
        _ => {
            return error_json(
                uri,
                &ErrorEnvelope::error(
                    ErrorCode::InvalidArgument,
                    "missing 'path' (append ?path=<dir> to the resource URI)",
                ),
            )
        }
    };
    let p = std::path::Path::new(path);
    if let Err(e) = enforce_force_root(p) {
        return error_json(uri, &e);
    }
    let root = match resolve_root(p) {
        Ok(r) => r,
        Err(e) => return error_json(uri, &e),
    };
    if let Err(e) = enforce_force_root(&root) {
        return error_json(uri, &e);
    }
    let index = match get_index(&root) {
        Ok(idx) => idx,
        Err(e) => return error_json(uri, &e),
    };
    let result = op(&index);
    let text = serde_json::to_string_pretty(&result).unwrap_or_default();
    ResourceContents::TextResourceContents {
        uri: uri.to_string(),
        mime_type: Some("application/json".to_string()),
        text,
        meta: None,
    }
}

/// A `text/markdown` content block.
fn text_md(uri: &str, text: String) -> ResourceContents {
    ResourceContents::TextResourceContents {
        uri: uri.to_string(),
        mime_type: Some("text/markdown".to_string()),
        text,
        meta: None,
    }
}

/// An `application/json` content block carrying a serialized error envelope.
fn error_json(uri: &str, envelope: &serde_json::Value) -> ResourceContents {
    ResourceContents::TextResourceContents {
        uri: uri.to_string(),
        mime_type: Some("application/json".to_string()),
        text: serde_json::to_string_pretty(envelope).unwrap_or_default(),
        meta: None,
    }
}

/// Decode `%XX` percent-escapes. Invalid escapes are left literal; decoded bytes are UTF-8 (lossy).
fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(hi), Some(lo)) = (hex_val(bytes[i + 1]), hex_val(bytes[i + 2])) {
                out.push(hi * 16 + lo);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percent_decode_handles_escapes_and_passthrough() {
        assert_eq!(percent_decode("users"), "users");
        assert_eq!(percent_decode("ns%3Ausers"), "ns:users");
        assert_eq!(percent_decode("a%20b"), "a b");
        assert_eq!(percent_decode("50%"), "50%");
        assert_eq!(percent_decode("%zz"), "%zz");
    }

    #[test]
    fn split_uri_extracts_path() {
        assert_eq!(
            split_uri("qmdc://guide"),
            ("qmdc://guide".to_string(), None)
        );
        assert_eq!(
            split_uri("qmdc://tree?path=%2Fwork%2Fspace"),
            ("qmdc://tree".to_string(), Some("/work/space".to_string()))
        );
    }
}
