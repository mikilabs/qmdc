use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;

use crate::lsp::server::{byte_offset_to_utf16_offset, Backend};

/// Handle textDocument/documentSymbol request.
///
/// This is a direct extraction of the logic previously inline in the
/// `LanguageServer::document_symbol` trait method on `Backend`.
pub async fn handle(
    backend: &Backend,
    params: DocumentSymbolParams,
) -> Result<Option<DocumentSymbolResponse>> {
    let uri = &params.text_document.uri;

    // Get document from cache or load from disk
    let doc = match backend.get_or_load_document(uri).await {
        Some(d) => d,
        None => return Ok(None),
    };

    let lines: Vec<&str> = doc.content.lines().collect();

    // Step 1: Create flat list of symbols with their levels
    let mut symbols_with_levels: Vec<(DocumentSymbol, u32)> = Vec::new();

    for obj in &doc.objects {
        let kind_str = obj
            .get("__kind")
            .and_then(|v| v.as_str())
            .unwrap_or("__Object");

        // Skip internal/meta objects that shouldn't appear in outline
        if kind_str == "__Document" || kind_str == "__TextBlock" {
            continue;
        }

        // Skip objects without __level (array/table elements)
        let level = match obj.get("__level").and_then(|v| v.as_u64()) {
            Some(l) => l as u32,
            None => continue,
        };

        let id = obj.get("__id").and_then(|v| v.as_str()).unwrap_or("?");
        let label = obj.get("__label").and_then(|v| v.as_str()).unwrap_or(id);
        let line = obj.get("__line").and_then(|v| v.as_u64()).unwrap_or(1) as u32 - 1;

        // Map QMDC kind to LSP SymbolKind
        let symbol_kind = match kind_str {
            "__Workspace" => SymbolKind::NAMESPACE,
            "__Namespace" => SymbolKind::NAMESPACE,
            "__Object" => SymbolKind::CLASS,
            "Table" => SymbolKind::STRUCT,
            "Enum" => SymbolKind::ENUM,
            _ => SymbolKind::CLASS,
        };

        // Get line content for range
        let line_len = lines
            .get(line as usize)
            .map(|l| byte_offset_to_utf16_offset(l, l.len()))
            .unwrap_or(0);

        // detail: Kind (if not __Object)
        let detail = if kind_str != "__Object" {
            Some(kind_str.to_string())
        } else {
            None
        };

        // Collect text fields as children symbols
        let mut text_field_children: Vec<DocumentSymbol> = Vec::new();

        // Check for text fields via __syntax metadata
        if let Some(syntax_obj) = obj.get("__syntax").and_then(|v| v.as_object()) {
            // Get positions for fields
            let positions_obj = obj.get("__positions").and_then(|v| v.as_object());

            for (field_name, syntax_value) in syntax_obj {
                // Check if this is a multiline_text field
                if syntax_value.as_str() == Some("multiline_text") {
                    // Skip internal fields
                    if field_name.starts_with("__") {
                        continue;
                    }

                    // Get position from __positions
                    let (field_line, field_col) = if let Some(positions) = positions_obj {
                        if let Some(pos) = positions.get(field_name) {
                            if let Some(pos_obj) = pos.as_object() {
                                let line = pos_obj
                                    .get("line")
                                    .and_then(|v| v.as_u64())
                                    .map(|l| l as u32 - 1)
                                    .unwrap_or(line);
                                // For heading-defined text fields, use start of heading (character 0)
                                // For list item fields, use the column from __positions
                                // Note: __positions.col is stored as byte offset, convert to UTF-16
                                let col = if let Some(heading_line_text) = lines.get(line as usize)
                                {
                                    if heading_line_text.trim_start().starts_with('#') {
                                        0 // Start of heading for heading-defined fields
                                    } else {
                                        let byte_col = pos_obj
                                            .get("col")
                                            .and_then(|v| v.as_u64())
                                            .unwrap_or(0)
                                            as usize;
                                        byte_offset_to_utf16_offset(heading_line_text, byte_col)
                                    }
                                } else {
                                    0
                                };
                                (line, col)
                            } else {
                                (line, 0)
                            }
                        } else {
                            (line, 0)
                        }
                    } else {
                        (line, 0)
                    };

                    // Get field label - try to extract from heading if it's a heading-defined field
                    let field_label =
                        if let Some(heading_line_text) = lines.get(field_line as usize) {
                            // Check if this is a heading-defined text field (pattern: ## Label [[id:text]])
                            if heading_line_text.trim_start().starts_with('#') {
                                // Extract label from heading: remove #, spaces, and [[id:text]] part
                                let heading_text = heading_line_text.trim_start_matches('#').trim();
                                // Remove [[id:text]] or [[id]] part
                                let label = heading_text
                                    .split("[[")
                                    .next()
                                    .unwrap_or(heading_text)
                                    .trim()
                                    .to_string();
                                if !label.is_empty() {
                                    label
                                } else {
                                    field_name.clone()
                                }
                            } else {
                                field_name.clone()
                            }
                        } else {
                            field_name.clone()
                        };

                    // Get line content for range
                    let field_line_len = lines
                        .get(field_line as usize)
                        .map(|l| byte_offset_to_utf16_offset(l, l.len()))
                        .unwrap_or(0);

                    #[allow(deprecated)]
                    let field_symbol = DocumentSymbol {
                        name: field_label,
                        detail: Some("text".to_string()),
                        kind: SymbolKind::STRING,
                        tags: None,
                        deprecated: None,
                        range: Range {
                            start: Position {
                                line: field_line,
                                character: field_col,
                            },
                            end: Position {
                                line: field_line,
                                character: field_line_len.min(field_col + 20),
                            },
                        },
                        selection_range: Range {
                            start: Position {
                                line: field_line,
                                character: field_col,
                            },
                            end: Position {
                                line: field_line,
                                character: field_line_len.min(field_col + 20),
                            },
                        },
                        children: None,
                    };

                    text_field_children.push(field_symbol);
                }
            }
        }

        // Sort text field children by line number
        text_field_children.sort_by_key(|s| s.range.start.line);

        #[allow(deprecated)]
        let symbol = DocumentSymbol {
            name: label.to_string(),
            detail,
            kind: symbol_kind,
            tags: None,
            deprecated: None,
            range: Range {
                start: Position { line, character: 0 },
                end: Position {
                    line,
                    character: line_len,
                },
            },
            selection_range: Range {
                start: Position { line, character: 0 },
                end: Position {
                    line,
                    character: line_len,
                },
            },
            children: if text_field_children.is_empty() {
                None
            } else {
                Some(text_field_children)
            },
        };

        symbols_with_levels.push((symbol, level));
    }

    // Step 2: Build hierarchy based on levels using the shared core nesting algorithm.
    let mut flat_store: Vec<DocumentSymbol> =
        symbols_with_levels.iter().map(|(s, _)| s.clone()).collect();
    let levels: Vec<u32> = symbols_with_levels.iter().map(|(_, l)| *l).collect();

    // Build parent-child mapping (single source of truth: core::nesting).
    let parent_map = crate::core::nesting::parent_map_by_level(&levels);

    // Build hierarchy bottom-up: process from end to start
    for i in (0..flat_store.len()).rev() {
        if let Some(parent_idx) = parent_map[i] {
            // Add this symbol to parent's children
            let child = flat_store[i].clone();
            if flat_store[parent_idx].children.is_none() {
                flat_store[parent_idx].children = Some(Vec::new());
            }
            flat_store[parent_idx]
                .children
                .as_mut()
                .unwrap()
                .push(child);
        }
    }

    // Collect root symbols (those without parents)
    let mut root_symbols: Vec<DocumentSymbol> = Vec::new();
    for i in 0..flat_store.len() {
        if parent_map[i].is_none() {
            root_symbols.push(flat_store[i].clone());
        }
    }

    // Sort children in each symbol recursively
    fn sort_children_recursive(symbol: &mut DocumentSymbol) {
        if let Some(ref mut children) = symbol.children {
            children.sort_by_key(|s| s.range.start.line);
            for child in children.iter_mut() {
                sort_children_recursive(child);
            }
        }
    }

    for symbol in root_symbols.iter_mut() {
        sort_children_recursive(symbol);
    }

    Ok(Some(DocumentSymbolResponse::Nested(root_symbols)))
}
