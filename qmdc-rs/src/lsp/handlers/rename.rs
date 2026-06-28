use std::collections::HashMap;

use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;

use crate::lsp::server::{byte_offset_to_utf16_offset, Backend};

/// Handle textDocument/prepareRename request.
///
/// This is a direct extraction of the logic previously inline in the
/// `LanguageServer::prepare_rename` trait method on `Backend`.
pub async fn handle_prepare(
    backend: &Backend,
    params: TextDocumentPositionParams,
) -> Result<Option<PrepareRenameResponse>> {
    let uri = &params.text_document.uri;
    let position = params.position;

    // Get document from cache or load from disk
    let doc = match backend.get_or_load_document(uri).await {
        Some(d) => d,
        None => return Ok(None),
    };

    // Check if cursor is on a reference
    if let Some(parsed_ref) = backend.find_reference_at_position(&doc, position) {
        let id = backend.extract_id_from_target(&parsed_ref.target);
        let line = parsed_ref.line - 1;
        let range = Range {
            start: Position {
                line,
                character: parsed_ref.start_col,
            },
            end: Position {
                line,
                character: parsed_ref.end_col,
            },
        };
        return Ok(Some(PrepareRenameResponse::RangeWithPlaceholder {
            range,
            placeholder: id,
        }));
    }

    // Check if cursor is on an object definition header
    let line = position.line;
    for obj in &doc.objects {
        if let (Some(id), Some(obj_line)) = (
            obj.get("__id").and_then(|v| v.as_str()),
            obj.get("__line").and_then(|v| v.as_u64()),
        ) {
            if obj_line as u32 - 1 == line {
                // Find the [[id]] pattern in the line
                let lines: Vec<&str> = doc.content.lines().collect();
                let line_content = lines.get(line as usize).unwrap_or(&"");

                // Look for [[id]] or [[id:...]]
                let pattern = format!("[[{}", id);
                if let Some(start_pos) = line_content.find(&pattern) {
                    let bracket_start = byte_offset_to_utf16_offset(line_content, start_pos);
                    // Find the closing ]]
                    if let Some(rel_end) = line_content[start_pos..].find("]]") {
                        let bracket_end =
                            byte_offset_to_utf16_offset(line_content, start_pos + rel_end + 2);
                        let range = Range {
                            start: Position {
                                line,
                                character: bracket_start,
                            },
                            end: Position {
                                line,
                                character: bracket_end,
                            },
                        };
                        return Ok(Some(PrepareRenameResponse::RangeWithPlaceholder {
                            range,
                            placeholder: id.to_string(),
                        }));
                    }
                }
            }
        }
    }

    Ok(None)
}

/// Handle textDocument/rename request.
///
/// Computes the rename plan via the shared, transport-agnostic `core::ops::rename_plan`
/// planner (identity-resolved, cascading) over ALL open documents, then projects each plan
/// edit onto a minimal-diff UTF-16 `TextEdit` in its owning buffer. The matching/cascade
/// logic is shared verbatim with the MCP `rename_object` tool and the CLI — there is no
/// LSP-specific rename algorithm any more, only the buffer→range projection.
pub async fn handle(backend: &Backend, params: RenameParams) -> Result<Option<WorkspaceEdit>> {
    let uri = &params.text_document_position.text_document.uri;
    let position = params.text_document_position.position;
    let new_name = params.new_name.as_str();

    // Get document from cache or load from disk
    let doc = match backend.get_or_load_document(uri).await {
        Some(d) => d,
        None => return Ok(None),
    };

    // Determine which id the cursor is renaming (a reference target, or a definition header).
    let target_id = if let Some(parsed_ref) = backend.find_reference_at_position(&doc, position) {
        backend.extract_id_from_target(&parsed_ref.target)
    } else {
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

    let docs = backend.documents.read().await;

    // Feed the shared planner the union of all open-document objects, tagging each with its
    // document URI as the `__file` key so plan edits map back to the right buffer.
    let mut objects: Vec<serde_json::Value> = Vec::new();
    for (doc_uri, d) in docs.iter() {
        for obj in &d.objects {
            let mut o = obj.clone();
            if let Some(map) = o.as_object_mut() {
                map.insert(
                    "__file".to_string(),
                    serde_json::Value::String(doc_uri.to_string()),
                );
            }
            objects.push(o);
        }
    }

    // Source lines come from the in-memory buffers (so unsaved edits are reflected).
    let read_line = |file: &str, line_1based: i64| -> Option<String> {
        if line_1based < 1 {
            return None;
        }
        let u = Url::parse(file).ok()?;
        let d = docs.get(&u)?;
        d.content
            .lines()
            .nth((line_1based - 1) as usize)
            .map(|s| s.to_string())
    };

    let edits = match crate::core::ops::rename_plan::plan_rename_edits(
        &objects, &target_id, new_name, &read_line,
    ) {
        Ok(e) => e,
        Err(_) => return Ok(None), // invalid new id, empty old id, or target not found
    };

    // Project each plan edit onto a minimal UTF-16 TextEdit in its owning buffer.
    let mut changes: HashMap<Url, Vec<TextEdit>> = HashMap::new();
    for e in &edits {
        let u = match Url::parse(&e.file) {
            Ok(u) => u,
            Err(_) => continue,
        };
        let d = match docs.get(&u) {
            Some(d) => d,
            None => continue,
        };
        if e.line < 1 {
            continue;
        }
        let line0 = (e.line - 1) as u32;
        let line_content = d.content.lines().nth(line0 as usize).unwrap_or("");
        if let Some(edit) = minimal_text_edit(line_content, line0, &e.old_text, &e.new_text) {
            changes.entry(u).or_default().push(edit);
        }
    }

    // Deterministic per-file ordering (line, then character) — matches fixtures and aids review.
    for file_edits in changes.values_mut() {
        file_edits.sort_by(|a, b| {
            (a.range.start.line, a.range.start.character)
                .cmp(&(b.range.start.line, b.range.start.character))
        });
    }

    if changes.is_empty() {
        return Ok(None);
    }

    Ok(Some(WorkspaceEdit {
        changes: Some(changes),
        document_changes: None,
        change_annotations: None,
    }))
}

/// Produce a clean `TextEdit` for an `old_text`→`new_text` token change on `line_content`.
///
/// Locates `old_text`, skips the common structural prefix (`[[`, `[[#`, `[[#ns:`, …), then
/// replaces exactly the **id segment** (up to the next id boundary `.`/`:`/`]`/whitespace)
/// with the new id segment. This reproduces the human-intuitive "replace the id token" edit
/// (e.g. `user`→`customer`) rather than a coincidental minimal char-diff. Returns `None` if
/// `old_text` isn't on the line or nothing changes.
fn minimal_text_edit(
    line_content: &str,
    line: u32,
    old_text: &str,
    new_text: &str,
) -> Option<TextEdit> {
    let pos = line_content.find(old_text)?;
    let prefix = common_prefix_len(old_text, new_text);
    let old_seg_len = id_segment_len(&old_text[prefix..]);
    let new_seg_len = id_segment_len(&new_text[prefix..]);
    let old_start = pos + prefix;
    let old_end = old_start + old_seg_len;
    let new_seg = &new_text[prefix..prefix + new_seg_len];
    if old_start == old_end && new_seg.is_empty() {
        return None;
    }
    let start_char = byte_offset_to_utf16_offset(line_content, old_start);
    let end_char = byte_offset_to_utf16_offset(line_content, old_end);
    Some(TextEdit {
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
        new_text: new_seg.to_string(),
    })
}

/// Byte length of the longest common prefix of `a` and `b`, aligned to char boundaries.
fn common_prefix_len(a: &str, b: &str) -> usize {
    let mut len = 0;
    for (ca, cb) in a.chars().zip(b.chars()) {
        if ca != cb {
            break;
        }
        len += ca.len_utf8();
    }
    len
}

/// Byte length of the leading id segment of `s` — chars up to the first id boundary
/// (`.`, `:`, `]`, or whitespace). Used to bound a rename edit to a single id token.
fn id_segment_len(s: &str) -> usize {
    let mut len = 0;
    for c in s.chars() {
        if c == '.' || c == ':' || c == ']' || c.is_whitespace() {
            break;
        }
        len += c.len_utf8();
    }
    len
}
