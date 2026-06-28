use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;

use crate::lsp::server::{byte_offset_to_utf16_offset, Backend};

/// Handle textDocument/references request.
///
/// This is a direct extraction of the logic previously inline in the
/// `LanguageServer::references` trait method on `Backend`.
pub async fn handle(backend: &Backend, params: ReferenceParams) -> Result<Option<Vec<Location>>> {
    let uri = &params.text_document_position.text_document.uri;
    let position = params.text_document_position.position;
    let include_declaration = params.context.include_declaration;

    // Get document from cache or load from disk
    let doc = match backend.get_or_load_document(uri).await {
        Some(d) => d,
        None => return Ok(None),
    };

    // Check if cursor is on Kind in a definition - if so, find all objects of that Kind
    if let Some(kind) = backend.find_kind_at_position(&doc, position) {
        // Find all objects with this __kind
        return backend
            .find_all_objects_by_kind(&kind, include_declaration)
            .await;
    }

    // First, find the target ID we're searching for
    let target_id = if let Some(parsed_ref) = backend.find_reference_at_position(&doc, position) {
        backend.extract_id_from_target(&parsed_ref.target)
    } else {
        // Maybe cursor is on a definition - check if position is on a header line
        let line = position.line;
        let mut found_id = None;
        for obj in &doc.objects {
            if let (Some(id), Some(obj_line)) = (
                obj.get("__id").and_then(|v| v.as_str()),
                obj.get("__line").and_then(|v| v.as_u64()),
            ) {
                if obj_line as u32 - 1 == line {
                    found_id = Some(id.to_string());
                    break;
                }
            }
        }
        match found_id {
            Some(id) => id,
            None => return Ok(None),
        }
    };

    // Now search for references across all documents
    let mut locations = Vec::new();

    // Search in all open documents
    let docs = backend.documents.read().await;

    // Build a shared identity index over all open-document objects so reference membership
    // is decided by RESOLVED identity (handles `ns:id`, hierarchical ids, and `__local_id`)
    // via `core::resolve` — the same logic the MCP `find_references` op uses — instead of
    // naive string equality on the extracted id.
    let all_objects: Vec<serde_json::Value> = docs
        .values()
        .flat_map(|d| d.objects.iter().cloned())
        .collect();
    let obj_index = crate::core::resolve::ObjectIndex::build(&all_objects);

    // Canonicalise the search target to its `__id` so every comparison is identity-based.
    let canonical_id = obj_index
        .resolve_object(&target_id, "")
        .and_then(|o| o.get("__id").and_then(|v| v.as_str()))
        .unwrap_or(&target_id)
        .to_string();

    for (doc_uri, doc) in docs.iter() {
        // Add declaration if requested and this document contains it
        if include_declaration {
            if let Some(obj) = backend.find_object_by_id(doc, &canonical_id) {
                if let Some(line_num) = obj.get("__line").and_then(|v| v.as_u64()) {
                    let line = line_num as u32 - 1;
                    let lines: Vec<&str> = doc.content.lines().collect();
                    let line_content = lines.get(line as usize).unwrap_or(&"");

                    let pattern = format!("[[{}]]", canonical_id);
                    let (start_char, end_char) = if let Some(pos) = line_content.find(&pattern) {
                        (
                            byte_offset_to_utf16_offset(line_content, pos),
                            byte_offset_to_utf16_offset(line_content, pos + pattern.len()),
                        )
                    } else if let Some(pos) = line_content.find("[[") {
                        let end_pos = line_content[pos..]
                            .find("]]")
                            .map(|p| pos + p + 2)
                            .unwrap_or(line_content.len());
                        (
                            byte_offset_to_utf16_offset(line_content, pos),
                            byte_offset_to_utf16_offset(line_content, end_pos),
                        )
                    } else {
                        (
                            0,
                            byte_offset_to_utf16_offset(line_content, line_content.len()),
                        )
                    };

                    locations.push(Location {
                        uri: doc_uri.clone(),
                        range: Range {
                            start: Position {
                                line,
                                character: start_char,
                            },
                            end: Position {
                                line,
                                character: end_char,
                            },
                        },
                    });
                }
            }
        }

        // Add all references that RESOLVE to the target object from this document.
        for r in &doc.references {
            let resolves = obj_index
                .resolve_object(&r.target, "")
                .and_then(|o| o.get("__id").and_then(|v| v.as_str()))
                == Some(canonical_id.as_str());
            if resolves {
                let line = r.line - 1;
                locations.push(Location {
                    uri: doc_uri.clone(),
                    range: Range {
                        start: Position {
                            line,
                            character: r.start_col,
                        },
                        end: Position {
                            line,
                            character: r.end_col,
                        },
                    },
                });
            }
        }
    }

    Ok(Some(locations))
}
