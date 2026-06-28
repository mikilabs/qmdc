use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;

use crate::lsp::server::{utf16_offset_to_byte_offset, Backend};

/// Handle textDocument/completion request.
///
/// This is a direct extraction of the logic previously inline in the
/// `LanguageServer::completion` trait method on `Backend`.
pub async fn handle(
    backend: &Backend,
    params: CompletionParams,
) -> Result<Option<CompletionResponse>> {
    let uri = &params.text_document_position.text_document.uri;
    let position = params.text_document_position.position;

    eprintln!(
        "[qmdc] Completion at {}:{}:{}",
        uri.path(),
        position.line,
        position.character
    );

    // Get document from cache or load from disk
    let doc = match backend.get_or_load_document(uri).await {
        Some(d) => d,
        None => {
            eprintln!("[qmdc]   Document not found");
            return Ok(None);
        }
    };

    let lines: Vec<&str> = doc.content.lines().collect();
    let line = match lines.get(position.line as usize) {
        Some(l) => *l,
        None => return Ok(None),
    };

    let char_pos = utf16_offset_to_byte_offset(line, position.character);
    let prefix = &line[..char_pos];

    eprintln!("[qmdc]   prefix: {:?}", prefix);

    // Find if we're inside [[ context and extract partial text
    let (in_ref, after_bracket) = if let Some(bracket_pos) = prefix.rfind("[[") {
        let after = &prefix[bracket_pos + 2..];
        if after.contains("]]") {
            (false, "")
        } else {
            (true, after)
        }
    } else {
        (false, "")
    };

    eprintln!(
        "[qmdc]   in_ref: {}, after_bracket: {:?}",
        in_ref, after_bracket
    );

    if !in_ref {
        return Ok(None);
    }

    // Parse what's after [[
    // Formats: [[partial, [[#partial, [[Kind.partial, [[#Kind:partial, [[file#partial
    let is_hash = after_bracket.starts_with('#');
    let content = if is_hash {
        &after_bracket[1..]
    } else {
        after_bracket
    };

    // Check for cross-file reference: [[file# or [[path/file#
    // Format: file_or_path followed by # at the end (or # somewhere in the middle)
    if !is_hash && after_bracket.contains('#') {
        // Split by # to get file part and id partial
        let parts: Vec<&str> = after_bracket.splitn(2, '#').collect();
        let file_ref = parts[0];
        let id_partial = parts.get(1).unwrap_or(&"");

        // Get workspace to find the file
        let ws_index = backend.workspaces.read().await;

        if let Some(ws) = backend.find_workspace_for_file(uri, &ws_index) {
            // Try to find matching file in workspace
            let target_file = if file_ref.ends_with(".qmd.md") {
                file_ref.to_string()
            } else {
                format!("{}.qmd.md", file_ref)
            };

            // Get objects from that file
            let file_objects: Vec<&serde_json::Value> = ws
                .objects
                .values()
                .flat_map(|objs| objs.iter())
                .filter(|obj| {
                    obj.get("__file")
                        .and_then(|v| v.as_str())
                        .map(|f| f == target_file || f.ends_with(&format!("/{}", target_file)))
                        .unwrap_or(false)
                })
                .collect();

            if !file_objects.is_empty() {
                let mut items =
                    backend.complete_ids_from_objects(file_objects.into_iter(), id_partial);
                items.sort_by(|a, b| a.label.cmp(&b.label));
                return Ok(Some(CompletionResponse::Array(items)));
            }
        }

        // Fallback to current document if file not found
        return backend.complete_ids(&doc, id_partial);
    }

    // Check if we're completing Kind after [[id: (header definition)
    // e.g., [[config: -> suggest Kinds
    // content would be "config:" - ends with : and has exactly one :
    if !is_hash && content.ends_with(':') && content.matches(':').count() == 1 {
        eprintln!("[qmdc] Kind completion after [[id:");
        // Use workspace for Kinds completion
        let ws_index = backend.workspaces.read().await;
        eprintln!("[qmdc]   Workspaces count: {}", ws_index.by_uri.len());
        if let Some(ws) = backend.find_workspace_for_file(uri, &ws_index) {
            let all_objs: Vec<_> = ws.objects.values().flat_map(|v| v.iter()).collect();
            let kinds: Vec<_> = all_objs
                .iter()
                .filter_map(|obj| obj.get("__kind").and_then(|v| v.as_str()))
                .collect();
            eprintln!(
                "[qmdc]   Found workspace with {} objects, kinds: {:?}",
                ws.objects.len(),
                kinds
            );
            return backend.complete_kinds_from_objects(all_objs.into_iter());
        }
        eprintln!("[qmdc]   No workspace found, falling back to doc");
        return backend.complete_kinds(&doc);
    }

    // Check if we have Kind: prefix in hash reference (e.g., [[#Table: or [[#Table:us)
    // e.g., [[#Table: -> suggest IDs of Kind Table
    if is_hash {
        if let Some(colon_pos) = content.rfind(':') {
            let kind_prefix = &content[..colon_pos];
            let partial_after_colon = &content[colon_pos + 1..];

            // Use workspace objects for completion
            let ws_index = backend.workspaces.read().await;
            if let Some(ws) = backend.find_workspace_for_file(uri, &ws_index) {
                let all_objs: Vec<_> = ws.objects.values().flat_map(|v| v.iter()).collect();
                let mut items = backend.complete_with_kind_filter_from_objects(
                    all_objs.into_iter(),
                    kind_prefix,
                    partial_after_colon,
                );
                items.sort_by(|a, b| a.label.cmp(&b.label));
                return Ok(Some(CompletionResponse::Array(items)));
            }

            return backend.complete_with_kind_filter(&doc, kind_prefix, partial_after_colon);
        }
    }

    // Check if we have Kind. or ns. prefix (e.g., [[Table. or [[models.)
    if let Some(dot_pos) = content.rfind('.') {
        let kind_or_ns = &content[..dot_pos];
        let partial_after_dot = &content[dot_pos + 1..];

        // Use workspace objects for completion
        let ws_index = backend.workspaces.read().await;
        if let Some(ws) = backend.find_workspace_for_file(uri, &ws_index) {
            let all_objs: Vec<_> = ws.objects.values().flat_map(|v| v.iter()).collect();
            let mut items = backend.complete_with_kind_filter_from_objects(
                all_objs.into_iter(),
                kind_or_ns,
                partial_after_dot,
            );
            items.sort_by(|a, b| a.label.cmp(&b.label));
            return Ok(Some(CompletionResponse::Array(items)));
        }

        return backend.complete_with_kind_filter(&doc, kind_or_ns, partial_after_dot);
    }

    // Regular completion: [[partial or [[#partial - use workspace objects
    let ws_index = backend.workspaces.read().await;
    if let Some(ws) = backend.find_workspace_for_file(uri, &ws_index) {
        let all_objs: Vec<_> = ws.objects.values().flat_map(|v| v.iter()).collect();
        let mut items = backend.complete_ids_from_objects(all_objs.clone().into_iter(), content);

        // Fallback to fuzzy match if no results
        if items.is_empty() && !content.is_empty() {
            let partial_lower = content.to_lowercase();
            items = all_objs
                .into_iter()
                .filter_map(|obj| {
                    let id = obj.get("__id").and_then(|v| v.as_str())?;
                    let kind = obj
                        .get("__kind")
                        .and_then(|v| v.as_str())
                        .unwrap_or("__Object");

                    // Skip system objects (auto-generated IDs)
                    if id.starts_with("doc_") || id.starts_with("text_") {
                        return None;
                    }

                    let id_lower = id.to_lowercase();

                    if !backend.fuzzy_match(&partial_lower, &id_lower) {
                        return None;
                    }

                    let label_text = obj.get("__label").and_then(|v| v.as_str()).unwrap_or(id);

                    Some(CompletionItem {
                        label: id.to_string(),
                        kind: Some(CompletionItemKind::VARIABLE),
                        detail: Some(kind.to_string()),
                        documentation: Some(Documentation::String(label_text.to_string())),
                        ..Default::default()
                    })
                })
                .collect();
        }

        items.sort_by(|a, b| a.label.cmp(&b.label));
        return Ok(Some(CompletionResponse::Array(items)));
    }

    // Fallback to current document only
    backend.complete_ids(&doc, content)
}
