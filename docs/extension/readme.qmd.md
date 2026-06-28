# VS Code Extension [[extension:__Namespace]]

- commands: [[#ext_cmd_goto]], [[#ext_cmd_show_refs]], [[#ext_cmd_parse]], [[#ext_cmd_validate]], [[#ext_cmd_sql]], [[#ext_cmd_query_block]], [[#ext_cmd_preview]], [[#ext_cmd_preview_beside]], [[#ext_cmd_refresh]], [[#ext_cmd_restart]], [[#ext_cmd_copy_global_id]], [[#ext_cmd_reveal_in_explorer]], [[#ext_cmd_preview_object_file]]

VS Code extension for QMDC — syntax highlighting, LSP integration, views, commands, and settings.

Install from the [VS Code Marketplace](https://marketplace.visualstudio.com/items?itemName=MiKiLabs.qmdc-vscode) or [Open VSX](https://open-vsx.org/extension/mikilabs/qmdc-vscode) — search **QMDC** (publisher `mikilabs`).

## Capabilities [[capabilities: NarrativeDoc]]

- about: [[#lsp:completion]], [[#lsp:definition]], [[#lsp:references]], [[#lsp:document_symbol]], [[#lsp:workspace_symbol]], [[#lsp:rename]], [[#lsp:diagnostics]], [[#lsp:hover]]

### Editor Features [[editor_features: text]]

The extension provides full IDE support for `.qmd.md` files:

- Syntax Highlighting via TextMate grammar
- Autocompletion for object IDs, Kinds, namespaces, references
- Hover preview of objects with their fields
- Go to Definition (Cmd+Click / F12) for `[[#id]]` references
- Find All References (Shift+F12) for objects
- Document Symbols (Cmd+Shift+O) — outline with object hierarchy
- Workspace Symbols (Cmd+T) — search objects across all files
- Rename Symbol (F2) — rename ID and update all references
- Diagnostics — broken links, duplicate IDs, validation errors

### Requirements [[requirements: text]]

The extension bundles the `qmdc` binary (QMDC Language Server) for each supported platform — no separate installation needed. Install it from the [VS Code Marketplace](https://marketplace.visualstudio.com/items?itemName=MiKiLabs.qmdc-vscode) or [Open VSX](https://open-vsx.org/extension/mikilabs/qmdc-vscode) (or sideload the `.vsix` for your platform) and it works out of the box.

If you need to use a custom build of `qmdc` (e.g., a development build), override the path in settings:

```json
{
  "qmdc.server.path": "/path/to/custom/qmdc"
}
```

## Views [[views: NarrativeDoc]]

- about: [[#lsp:workspace_symbol]]

### QMDC Objects Explorer [[objects_explorer: text]]

Tree view of workspace objects with grouping by namespace, files, or smart hierarchy.

Shows the hierarchical workspace structure:

- Workspaces — all QMDC workspaces in the project (multi-root support)
- Namespaces — logical object groupings
- Objects — all QMD.md objects with their kinds

Clicking an item opens the object definition. The explorer auto-refreshes on file changes.

- view_id: qmdcObjects
- location: activitybar
- container_id: qmdc-explorer
- grouping_modes: [namespace, file, smart]

![QMDC Objects Explorer tree in the activity bar](../.assets/vscode-explorer.png)

### QMDC Preview [[preview_panel: text]]

Webview panel for previewing QMD.md documents with rendered markdown and executed SQL queries.

Opened via the `QMDC: Open Preview` command. Shows:

- Rendered markdown with styled formatting
- Executed `table` blocks with SQL query results
- Clickable `[[#ref]]` links for navigating to definitions (via LSP go-to-definition)
- Dimmed metadata `[[id]]` and `[[id: Kind]]` (hidden via CSS `display: none`)
- Mermaid diagrams (`mermaid` code blocks → SVG, bundled offline via mermaid.min.js)
- Back/forward navigation (mouse back/forward buttons, Alt+Left/Right)
- Scroll-to-anchor on link navigation (auto-scroll to target object)

Preview auto-refreshes on document changes.

Rendering logic is extracted into `src/preview-renderer.ts` with a `QueryExecutor` interface for testability. Covered by 31 Playwright e2e tests (`e2e/preview-render.spec.ts`).

![The QMDC rendered preview panel](../.assets/vscode-preview.png)

## Settings [[settings: NarrativeDoc]]

### Server Path [[server_path_setting: text]]

Path to the `qmdc` binary.

If empty, uses `qmdc` from PATH or searches in the workspace.

- key: qmdc.server.path
- type: string
- default: ""

### Trace Server [[trace_setting: text]]

Tracing of communication between VS Code and the QMDC language server.

Levels:

- `off` — no tracing
- `messages` — message logging
- `verbose` — detailed logging

- key: qmdc.trace.server
- type: string
- default: off
- enum: [off, messages, verbose]

### Dim Metadata [[dim_metadata_setting: text]]

Dims `[[id]]` and `[[id: Kind]]` metadata for better readability.

When enabled, metadata is displayed in gray italic so it doesn't distract from the main content.

- key: qmdc.dimMetadata
- type: boolean
- default: true

### Show Preview Button [[show_preview_button_setting: text]]

Shows the Open Preview button in the editor title bar for `.qmd.md` files.

- key: qmdc.showPreviewButton
- type: boolean
- default: true

## Build and Publish [[build_publish: NarrativeDoc]]

### Platforms [[platforms: text]]

The extension supports 6 platforms:

- macOS Apple Silicon (darwin-arm64)
- macOS Intel (darwin-x64)
- Linux x64 (linux-x64)
- Linux ARM64 (linux-arm64)
- Windows x64 (win32-x64)
- Windows ARM64 (win32-arm64)

### Build Scripts [[build_scripts: text]]

See `qmdc-vscode/package.json` scripts section:

- `npm run build` — quick build for current platform (dev)
- `npm run package:darwin-arm64` — build for macOS Apple Silicon
- `npm run package:darwin-x64` — build for macOS Intel
- `npm run package:linux-x64` — build for Linux x64
- `npm run package:linux-arm64` — build for Linux ARM64
- `npm run package:win32-x64` — build for Windows x64
- `npm run package:win32-arm64` — build for Windows ARM64
- `npm run package:all` — build for all platforms

To release the extension, use the single canonical path from the repo root: `make ext-bump` then `make ext-release` (builds all 6 platforms and publishes to Open VSX, plus the VS Code Marketplace).

### Versioning [[versioning: text]]

- `npm run bump-version` — increment patch version (0.2.0 → 0.2.1)
- `npm run bump-version minor` — increment minor version (0.2.0 → 0.3.0)
- `npm run bump-version major` — increment major version (0.2.0 → 1.0.0)
- `npm run test:e2e` — run Playwright e2e tests for preview rendering

### Configuration Files [[config_files: text]]

- `qmdc-vscode/package.json` — extension manifest (contributes, scripts, dependencies)
- `qmdc-vscode/language-configuration.json` — language configuration (brackets, comments, auto-closing)
- `qmdc-vscode/syntaxes/qmdc.tmLanguage.json` — TextMate grammar for syntax highlighting
- `qmdc-vscode/src/extension.ts` — main extension file
- `qmdc-vscode/src/preview-renderer.ts` — rendering logic for preview webview (extracted for testability)
- `qmdc-vscode/src/qmdcTreeProvider.ts` — QMDC Explorer tree view implementation
- `qmdc-vscode/e2e/preview-render.spec.ts` — Playwright e2e tests for preview rendering
- `qmdc-vscode/playwright.config.ts` — Playwright configuration
