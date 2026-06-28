use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;

use crate::lsp::server::Backend;

/// Handle workspace/symbol request.
///
/// This is a direct extraction of the logic previously inline in the
/// `LanguageServer::symbol` trait method on `Backend`.
pub async fn handle(
    backend: &Backend,
    params: WorkspaceSymbolParams,
) -> Result<Option<Vec<SymbolInformation>>> {
    let query = params.query.to_lowercase();
    let mut symbols = Vec::new();

    // Search across all workspaces
    let ws_index = backend.workspaces.read().await;

    for ws in ws_index.by_uri.values() {
        for (id, objs) in &ws.objects {
            for obj in objs {
                let label = obj.get("__label").and_then(|v| v.as_str()).unwrap_or(id);

                // Filter by query (fuzzy match on id or label)
                if !query.is_empty()
                    && !id.to_lowercase().contains(&query)
                    && !label.to_lowercase().contains(&query)
                {
                    continue;
                }

                let kind = obj
                    .get("__kind")
                    .and_then(|v| v.as_str())
                    .unwrap_or("__Object");

                // Map kind to SymbolKind
                let symbol_kind = match kind {
                    "Table" | "Entity" | "Model" => SymbolKind::CLASS,
                    "Column" | "Field" | "Property" => SymbolKind::FIELD,
                    "View" | "Query" => SymbolKind::INTERFACE,
                    "Index" | "Constraint" => SymbolKind::KEY,
                    "Function" | "Procedure" => SymbolKind::FUNCTION,
                    "Enum" => SymbolKind::ENUM,
                    "__Workspace" => SymbolKind::NAMESPACE,
                    _ => SymbolKind::OBJECT,
                };

                let line = obj
                    .get("__line")
                    .and_then(|v| v.as_u64())
                    .map(|l| l as u32 - 1)
                    .unwrap_or(0);

                // Get file URI - __file is relative to project_root (adjusted in scan_workspace_folder)
                let file_uri = obj.get("__file").and_then(|v| v.as_str()).and_then(|f| {
                    let full_path = ws.project_root.join(f);
                    Url::from_file_path(&full_path).ok()
                });

                if let Some(uri) = file_uri {
                    let detail = if kind != "__Object" {
                        Some(kind.to_string())
                    } else {
                        None
                    };

                    #[allow(deprecated)]
                    symbols.push(SymbolInformation {
                        name: format!(
                            "{} ({})",
                            label,
                            obj.get("__local_id").and_then(|v| v.as_str()).unwrap_or(id)
                        ),
                        kind: symbol_kind,
                        tags: None,
                        deprecated: None,
                        location: Location {
                            uri,
                            range: Range {
                                start: Position { line, character: 0 },
                                end: Position {
                                    line,
                                    character: 100, // Approximate; file not necessarily open
                                },
                            },
                        },
                        container_name: detail,
                    });
                }
            }
        }
    }

    // Sort by name
    symbols.sort_by(|a, b| a.name.cmp(&b.name));

    // Limit results
    symbols.truncate(100);

    Ok(Some(symbols))
}
