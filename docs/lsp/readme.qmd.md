# LSP Server [[lsp:__Namespace]]

- features: [[#lsp:completion]], [[#lsp:diagnostics]], [[#lsp:definition]], [[#lsp:references]], [[#lsp:document_symbol]], [[#lsp:workspace_symbol]], [[#lsp:hover]], [[#lsp:formatting]], [[#lsp:rename]], [[#lsp:code_action]], [[#lsp:semantic_tokens]], [[#lsp:folding_range]], [[#lsp:document_link]], [[#lsp:inlay_hint]]
- categories: [[#lsp_navigation]], [[#lsp_completion]], [[#lsp_diagnostics]], [[#lsp_information]], [[#lsp_refactoring]], [[#lsp_formatting]], [[#lsp_visual]], [[#lsp_commands]]
- depends: [[#parsers:rust_parser]]
- related_to: [[#mcp]]

Language Server Protocol implementation for QMDC. Provides IDE features: completion, diagnostics, navigation, formatting, hover, rename, and semantic highlighting.

The LSP server is implemented only in the Rust parser ([[#parsers:rust_parser]]). Use the `status` field to track implementation progress of each feature.

The reference-resolution, tree, rename, and validation logic lives in a transport-agnostic `core` layer that the LSP shares with the [[#mcp]] server, so both surfaces behave identically.
