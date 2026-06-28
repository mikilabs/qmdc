# Extension Commands

VS Code extension commands for QMDC.

## Restart Language Server [[ext_cmd_restart: ExtCommand]]

Restarts the LSP server. Used when settings change or the server has issues.

- command_id: qmdc.restartServer
- keybinding: null
- when: null

### Description [[description: text]]

Stops the running QMDC language server process and starts a new one. All workspace indexing is re-executed from scratch.

## Go to Object [[ext_cmd_goto: ExtCommand]]

Quick picker for navigating to any object in the workspace.

- command_id: qmdc.goToObject
- keybinding: Ctrl+Shift+O (Mac: Cmd+Shift+O)
- when: editorLangId == qmdmd
- depends: [[#lsp:workspace_symbol]]

### Description [[description: text]]

Opens a list of all objects with search by ID and Kind. Selecting an item navigates to the object definition.

## Show References [[ext_cmd_show_refs: ExtCommand]]

Finds all references to the object under the cursor.

- command_id: qmdc.showReferences
- keybinding: Shift+F12
- when: editorLangId == qmdmd
- depends: [[#lsp:references]]

### Description [[description: text]]

Shows a list of all places where the given object is referenced across the entire workspace.

## Parse Workspace [[ext_cmd_parse: ExtCommand]]

Re-parses all QMD.md files in the workspace.

- command_id: qmdc.parseWorkspace
- keybinding: null
- when: null

### Description [[description: text]]

Updates the object and reference index by re-parsing every `.qmd.md` file in the workspace.

## Validate Workspace [[ext_cmd_validate: ExtCommand]]

Checks all files for errors and shows the Problems panel.

- command_id: qmdc.validateWorkspace
- keybinding: null
- when: null
- depends: [[#lsp:diagnostics]]

### Description [[description: text]]

Runs validation across the workspace: broken links, duplicate IDs, invalid syntax. Results appear in the VS Code Problems panel.

## Run SQL Query [[ext_cmd_sql: ExtCommand]]

Executes a SQL query against workspace objects.

- command_id: qmdc.runSqlQuery
- keybinding: null
- when: null
- depends: [[#lsp:cmd_run_sql_query]]

### Description [[description: text]]

Opens an input box for entering a SQL query, executes it, and displays the results.

Available tables:

- `objects` — all objects with columns: `__id`, `__kind`, `__label`, `__file`, `__line`, `data` (JSON)
- `edges` — all references with columns: `source_id`, `source_field`, `target_id`, `edge_type`, `__workspace`

## Run Query from Block [[ext_cmd_query_block: ExtCommand]]

Executes a SQL query from a code block in the document.

- command_id: qmdc.runQueryFromBlock
- keybinding: null
- when: null
- depends: [[#lsp:cmd_run_sql_query]]

### Description [[description: text]]

Used for dynamic `[[query:...]]` blocks. Extracts the SQL from the code block and executes it against the workspace database.

## Open Preview [[ext_cmd_preview: ExtCommand]]

Opens a preview of the QMD.md document.

- command_id: qmdc.openPreview
- keybinding: null
- when: null
- depends: [[#lsp:hover]]

### Description [[description: text]]

Shows a rendered version of the document with resolved references, executed SQL query blocks, and Mermaid diagrams.

## Refresh Explorer [[ext_cmd_refresh: ExtCommand]]

Refreshes the QMDC Explorer view.

- command_id: qmdc.refreshExplorer
- keybinding: null
- when: view == qmdcObjects
- depends: [[#lsp:workspace_symbol]]

### Description [[description: text]]

Reloads the object tree in the QMDC Explorer panel.

## Group by Namespace [[ext_cmd_group_ns: ExtCommand]]

Groups objects in the Explorer by namespace.

- command_id: qmdc.groupByNamespace
- keybinding: null
- when: view == qmdcObjects

### Description [[description: text]]

Grouping mode: Workspace → Namespace → Objects.

## Group by File [[ext_cmd_group_file: ExtCommand]]

Groups objects in the Explorer by file.

- command_id: qmdc.groupByFile
- keybinding: null
- when: view == qmdcObjects

### Description [[description: text]]

Grouping mode: Workspace → Files → Objects.

## Smart Hierarchy [[ext_cmd_group_smart: ExtCommand]]

Groups objects in the Explorer by smart hierarchy.

- command_id: qmdc.groupBySmart
- keybinding: null
- when: view == qmdcObjects

### Description [[description: text]]

Grouping mode: shows the parent-child object hierarchy.

## Open Preview Beside [[ext_cmd_preview_beside: ExtCommand]]

Opens a preview of the QMD.md document in a split editor.

- command_id: qmdc.openPreviewBeside
- keybinding: null
- when: editorLangId == qmdmd

### Description [[description: text]]

Same as Open Preview but opens in a side-by-side split view.

## Copy Global ID [[ext_cmd_copy_global_id: ExtCommand]]

Copies the global ID of an object from the Explorer tree.

- command_id: qmdc.copyGlobalId
- keybinding: null
- when: view == qmdcObjects

### Description [[description: text]]

Context menu action on objects in the QMDC Explorer. Copies the `__global_id` (e.g., `workspace:namespace:id`) to the clipboard.

## Reveal In File Explorer [[ext_cmd_reveal_in_explorer: ExtCommand]]

Reveals the file of an object in the VS Code file explorer.

- command_id: qmdc.revealInExplorer
- keybinding: null
- when: view == qmdcObjects

### Description [[description: text]]

Context menu action on objects in the QMDC Explorer. Opens the file explorer and highlights the file containing the selected object.

## Preview Object File [[ext_cmd_preview_object_file: ExtCommand]]

Opens a preview of the file containing an object from the Explorer tree.

- command_id: qmdc.previewObjectFile
- keybinding: null
- when: view == qmdcObjects

### Description [[description: text]]

Context menu action on objects in the QMDC Explorer. Opens the QMDC preview for the file containing the selected object.
