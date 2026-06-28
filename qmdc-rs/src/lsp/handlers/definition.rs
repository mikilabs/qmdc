use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;

use crate::lsp::server::{byte_offset_to_utf16_offset, Backend};

/// Handle textDocument/definition request.
///
/// This is a direct extraction of the logic previously inline in the
/// `LanguageServer::goto_definition` trait method on `Backend`.
pub async fn handle(
    backend: &Backend,
    params: GotoDefinitionParams,
) -> Result<Option<GotoDefinitionResponse>> {
    let uri = &params.text_document_position_params.text_document.uri;
    let position = params.text_document_position_params.position;

    eprintln!(
        "[qmdc] GotoDefinition at {}:{}:{}",
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

    // First check if cursor is on ID or Kind in a definition (e.g., [[id:Kind]] in heading)
    // If so, return this location so VS Code shows "peek references"
    // Check for ID part
    if let Some((id, id_range)) = backend.find_id_in_definition_at_position(&doc, position) {
        eprintln!("[qmdc]   Cursor on ID '{}' in definition", id);
        return Ok(Some(GotoDefinitionResponse::Scalar(Location {
            uri: uri.clone(),
            range: id_range,
        })));
    }
    // Check for Kind part - only in headings (object definitions)
    let lines: Vec<&str> = doc.content.lines().collect();
    if let Some(line_content) = lines.get(position.line as usize) {
        if line_content.trim_start().starts_with('#') {
            if let Some(kind) = backend.find_kind_at_position(&doc, position) {
                eprintln!("[qmdc]   Cursor on Kind '{}' in definition", kind);
                // Return this line's location so VS Code shows peek references
                let line_len = byte_offset_to_utf16_offset(line_content, line_content.len());
                return Ok(Some(GotoDefinitionResponse::Scalar(Location {
                    uri: uri.clone(),
                    range: Range {
                        start: Position {
                            line: position.line,
                            character: 0,
                        },
                        end: Position {
                            line: position.line,
                            character: line_len,
                        },
                    },
                })));
            }
        }
    }

    eprintln!("[qmdc]   Document has {} references", doc.references.len());
    let parsed_ref = backend.find_reference_at_position(&doc, position).cloned();
    if let Some(ref r) = &parsed_ref {
        eprintln!(
            "[qmdc]   Found reference: {:?} at line {} cols {}-{}",
            r.target, r.line, r.start_col, r.end_col
        );
    } else {
        eprintln!("[qmdc]   No reference at position");
    }

    if let Some(parsed_ref) = parsed_ref {
        let target = &parsed_ref.target;

        // Check if this is a cross-workspace reference: ws_id:id or ws_id:ns:id
        let parts: Vec<&str> = target.trim_start_matches('#').split(':').collect();

        if parts.len() >= 2 {
            // Could be Kind:id or workspace:id - try workspace first
            let potential_ws_id = parts[0];
            let ref_id = parts.last().unwrap_or(&"");

            // Check if first part is a workspace ID
            {
                let ws_index = backend.workspaces.read().await;
                if ws_index.by_id.contains_key(potential_ws_id) {
                    // This is a cross-workspace reference
                    if let Some((obj, target_uri)) = backend
                        .resolve_cross_workspace_ref(potential_ws_id, ref_id)
                        .await
                    {
                        if let Some(line) = obj.get("__line").and_then(|v| v.as_u64()) {
                            let line = line as u32 - 1;
                            let line_len = backend.get_line_end_character(&target_uri, line).await;
                            return Ok(Some(GotoDefinitionResponse::Scalar(Location {
                                uri: target_uri,
                                range: Range {
                                    start: Position { line, character: 0 },
                                    end: Position {
                                        line,
                                        character: line_len,
                                    },
                                },
                            })));
                        }
                    }
                    return Ok(None);
                }
            }
        }

        // Regular reference - search in current document, then workspace
        // First try field-ref resolution (e.g., #quickstart.content)
        let raw_target = target.trim_start_matches('#');
        if raw_target.contains('.') && !raw_target.contains(':') {
            if let Some((target_uri, line)) =
                backend.resolve_field_ref_location(raw_target, uri).await
            {
                let line_len = backend.get_line_end_character(&target_uri, line).await;
                return Ok(Some(GotoDefinitionResponse::Scalar(Location {
                    uri: target_uri,
                    range: Range {
                        start: Position { line, character: 0 },
                        end: Position {
                            line,
                            character: line_len,
                        },
                    },
                })));
            }
        }

        let ref_id = backend.extract_id_from_target(target);

        // Parse namespace from target if present (format: ns:id or ns:Kind:id).
        // Single-source the rule via core::resolve (mirrors compute_diagnostics).
        let ref_namespace = crate::core::resolve::parse_ref_namespace(target);

        if let Some((obj, target_uri)) = backend
            .find_object_in_workspace_with_namespace(&ref_id, ref_namespace.as_deref(), uri)
            .await
        {
            if let Some(line) = obj.get("__line").and_then(|v| v.as_u64()) {
                let line = line as u32 - 1;
                let target_uri = target_uri.unwrap_or_else(|| uri.clone());

                let line_len = backend.get_line_end_character(&target_uri, line).await;

                return Ok(Some(GotoDefinitionResponse::Scalar(Location {
                    uri: target_uri,
                    range: Range {
                        start: Position { line, character: 0 },
                        end: Position {
                            line,
                            character: line_len,
                        },
                    },
                })));
            }
        }
    }

    Ok(None)
}
