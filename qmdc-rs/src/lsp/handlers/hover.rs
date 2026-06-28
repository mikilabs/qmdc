use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;

use crate::lsp::server::Backend;

/// Handle textDocument/hover request.
///
/// This is a direct extraction of the logic previously inline in the
/// `LanguageServer::hover` trait method on `Backend`.
pub async fn handle(backend: &Backend, params: HoverParams) -> Result<Option<Hover>> {
    let uri = &params.text_document_position_params.text_document.uri;
    let position = params.text_document_position_params.position;

    eprintln!(
        "[qmdc] Hover request at {}:{}:{}",
        uri.path(),
        position.line,
        position.character
    );

    // Get document from cache or load from disk
    let doc = match backend.get_or_load_document(uri).await {
        Some(d) => d,
        None => {
            eprintln!("[qmdc]   Document not found (not in cache and failed to load from disk)");
            return Ok(None);
        }
    };

    eprintln!("[qmdc]   Document has {} references", doc.references.len());
    let parsed_ref = backend.find_reference_at_position(&doc, position).cloned();
    if parsed_ref.is_some() {
        eprintln!("[qmdc]   Found reference at position");
    } else {
        eprintln!("[qmdc]   No reference at position");
    }

    // Find reference at position using parsed __references
    if let Some(parsed_ref) = parsed_ref {
        let ref_id = backend.extract_id_from_target(&parsed_ref.target);

        // Try field-ref resolution first (e.g., #guide.content → show the field)
        let raw_target = parsed_ref.target.trim_start_matches('#');
        if let Some((obj_prefix, field_part)) = Backend::split_field_ref(raw_target) {
            // Look up the parent object
            if let Some((obj, _target_uri)) =
                backend.find_object_in_workspace(obj_prefix, uri).await
            {
                // Check the field exists on this object
                if let Some(field_value) = obj.get(field_part) {
                    let parent_label = obj
                        .get("__label")
                        .and_then(|v| v.as_str())
                        .unwrap_or(obj_prefix);
                    let parent_kind = obj.get("__kind").and_then(|v| v.as_str()).unwrap_or("");

                    // Determine field type from __types or __syntax
                    let field_type = obj
                        .get("__syntax")
                        .and_then(|s| s.get(field_part))
                        .and_then(|v| v.as_str())
                        .unwrap_or_else(|| {
                            obj.get("__types")
                                .and_then(|t| t.get(field_part))
                                .and_then(|v| v.as_str())
                                .unwrap_or("string")
                        });

                    // Format the field value (truncate long text)
                    let val_str = match field_value {
                        serde_json::Value::String(s) => {
                            if s.len() > 80 {
                                format!("{}…", &s[..77])
                            } else {
                                s.clone()
                            }
                        }
                        serde_json::Value::Array(arr) => {
                            format!("[{} items]", arr.len())
                        }
                        other => other.to_string(),
                    };

                    let mut hover_text = format!("**{}** `.{}`", parent_label, field_part);
                    if !parent_kind.is_empty() && !parent_kind.starts_with("__") {
                        hover_text.push_str(&format!(" on `{}`", parent_kind));
                    }
                    hover_text.push_str(&format!("\n\nType: `{}`", field_type));
                    hover_text.push_str(&format!("\n\n{}", val_str));

                    return Ok(Some(Hover {
                        contents: HoverContents::Markup(MarkupContent {
                            kind: MarkupKind::Markdown,
                            value: hover_text,
                        }),
                        range: None,
                    }));
                }
            }
        }

        // Try to find in document or workspace
        if let Some((obj, _target_uri)) = backend.find_object_in_workspace(&ref_id, uri).await {
            let label = obj
                .get("__label")
                .and_then(|v| v.as_str())
                .unwrap_or(&ref_id);
            let kind = obj.get("__kind").and_then(|v| v.as_str()).unwrap_or("");
            let file = obj.get("__file").and_then(|v| v.as_str());
            let workspace_str = obj
                .get("__workspace")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let namespace = obj
                .get("__namespace")
                .and_then(|v| v.as_str())
                .unwrap_or("");

            // Fallback: if workspace is empty, use virtual workspace ID
            // Use "workspace" as default instead of directory name for consistency
            let workspace = if workspace_str.is_empty() {
                "workspace".to_string()
            } else {
                workspace_str.to_string()
            };

            // Compute __global_id for diagnostics
            let global_id = if namespace.is_empty() {
                format!("{}::{}", workspace, ref_id)
            } else {
                format!("{}:{}:{}", workspace, namespace, ref_id)
            };

            let mut hover_text = format!("**{}**", label);
            if !kind.is_empty() && !kind.starts_with("__") {
                hover_text.push_str(&format!(" `{}`", kind));
            }

            // Add __global_id for diagnostics
            hover_text.push_str(&format!("\n\n🔍 `{}`", global_id));

            // Add file info for cross-file references
            if let Some(file) = file {
                hover_text.push_str(&format!("\n📁 {}", file));
            }

            // Add fields (no extra newline before first item)
            if let Some(obj_map) = obj.as_object() {
                let obj_id = obj_map.get("__id").and_then(|v| v.as_str()).unwrap_or("");
                let fields: Vec<_> = obj_map
                    .iter()
                    .filter(|(k, _)| !k.starts_with("__"))
                    .collect();

                for (key, value) in fields {
                    let val_str = match value {
                        serde_json::Value::String(s) => {
                            // For child references, display local ID
                            let prefix = format!("[[#{}", obj_id);
                            if s.starts_with(&prefix) && s.ends_with("]]") {
                                let inner = &s[3..s.len() - 2]; // strip [[# and ]]
                                let local = inner
                                    .strip_prefix(obj_id)
                                    .and_then(|r| r.strip_prefix('.'))
                                    .unwrap_or(inner);
                                format!("[[#{}]]", local)
                            } else {
                                s.clone()
                            }
                        }
                        _ => value.to_string(),
                    };
                    hover_text.push_str(&format!("\n- {}: {}", key, val_str));
                }
            }

            return Ok(Some(Hover {
                contents: HoverContents::Markup(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: hover_text,
                }),
                range: None,
            }));
        } else {
            return Ok(Some(Hover {
                contents: HoverContents::Markup(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: format!("⚠️ Object '{}' not found", ref_id),
                }),
                range: None,
            }));
        }
    }

    Ok(None)
}
