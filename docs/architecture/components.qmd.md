# Components [[components: NarrativeDoc]]

- about: [[#parsers:python_parser]], [[#parsers:typescript_parser]], [[#parsers:rust_parser]]

System components of QMDC.

## QMD.md Specification [[qmdc_specification: text]]

- about: [[#format]]

The QMD.md format specification defines syntax, rules, concepts, and behavior. It is the "law" for all parser implementations. All three parsers (Python, TypeScript, Rust) must implement the same specification.

See the [[#format]] namespace for the full specification.

## Parser Implementations [[parser_implementations: text]]

- about: [[#parsers:python_parser]], [[#parsers:typescript_parser]], [[#parsers:rust_parser]]

Three independent QMDC parser implementations sharing the same CLI interface.

All three parsers provide the same CLI:

- `qmdc parse` — QMD.md → JSON conversion
- `qmdc rebuild` — JSON → QMD.md restoration
- `qmdc workspace` — multi-file workspace parsing
- `qmdc query` — SQL queries against workspace

The Rust parser additionally provides:

- `qmdc lsp` — LSP server

| Parser | Language | Highlights |
|--------|----------|------------|
| [[#parsers:python_parser]] | Python | Reference implementation, simple code |
| [[#parsers:typescript_parser]] | TypeScript | For Node.js/browser, full typing |
| [[#parsers:rust_parser]] | Rust | High performance, LSP server |

See the [[#parsers]] namespace for detailed descriptions of each implementation.

## LSP Server [[lsp_server_section: text]]

- about: [[#parsers:rust_parser]]

Language Server Protocol server for integrating QMDC with editors.

Implemented only in the Rust parser. Provides:

- ID, Kind, namespace autocompletion
- Reference validation and error diagnostics
- Go to Definition (F12, Ctrl+Click)
- Find All References (Shift+F12)
- Hover preview of objects
- Rename with reference updates

**Transport:** stdio (default, for editor integration), socket (debug mode).

**Multi-Workspace Support:** supports VS Code Multi-root Workspace — multiple folders with different QMDC workspaces simultaneously.

## VS Code Extension [[vscode_extension_section: text]]

- about: [[#parsers:rust_parser]]

Extension for Visual Studio Code providing QMDC integration.

**Features:**

- Language Client for launching and managing the LSP server
- Custom Views — QMDC Workspace Explorer with grouping by namespace/kind/file
- Preview Webview — rendered markdown with mermaid diagrams, SQL table blocks, QMDC ref navigation (rendering logic in `preview-renderer.ts`)
- Commands for workspace operations
- Settings for LSP server configuration
