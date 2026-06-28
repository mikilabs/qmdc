# Discovery [[mcp_discovery: Category]]

Tools for finding and inspecting objects in a workspace — the read-only "where is X / what is X" surface.

## Locate Object [[tool_locate_object: McpTool]]

Find WHERE an object is defined: its file, line, id, kind, and namespace.

- tool_name: qmdc_locate_object
- status: implemented
- args: path, ref

### Description [[description: text]]

Resolves `ref` to a single object and returns its location. `ref` accepts a bare id (`users`), a namespaced id (`storage:users`), a kind-qualified id, or a field dot-path (`users.email`); a leading `#` is optional. To read the object's fields instead of its location, use [[#tool_describe_object]].

## Describe Object [[tool_describe_object: McpTool]]

Describe WHAT an object is: its full card (label, id, kind, namespace, file, all fields).

- tool_name: qmdc_describe_object
- status: implemented
- args: path, ref

### Description [[description: text]]

Returns every field of the resolved object. When `ref` is a field dot-path like `users.email`, it returns that single field's value and type instead of the whole card. For just the location, use [[#tool_locate_object]].

## Search Objects [[tool_search_objects: McpTool]]

Fuzzy lookup by id or name (case-insensitive substring).

- tool_name: qmdc_search_objects
- status: implemented
- args: path, query

### Description [[description: text]]

Returns objects whose id or name contains `query`, each with id, name, kind, file, line, and namespace. Use it when you don't know the exact id. Results are bounded (see [[#mcp_bounded]]).

## Outline File [[tool_outline_file: McpTool]]

Outline a single file as a nested object tree.

- tool_name: qmdc_outline_file
- status: implemented
- args: path, file

### Description [[description: text]]

Returns the objects in one file (`file` is a workspace-relative path) as a tree of `{id, kind, name, line, children}`, nested and ordered exactly like [[#tool_get_tree]] (children-first, alphabetical). It is the MCP twin of the LSP [[#lsp:document_symbol]] feature. For the whole workspace, use [[#tool_get_tree]].

## Get Tree [[tool_get_tree: McpTool]]

The workspace structure as a tree.

- tool_name: qmdc_get_tree
- status: implemented
- args: path, mode

### Description [[description: text]]

Returns the workspace as a nested tree. `mode` selects the grouping: `namespace` (namespace → kind → object, the default), `file` (group by files), or `smart` (a single parent/child hierarchy by `__parent`). The `smart` mode is the one the tree-nesting standard targets — containers like this Category nest their items. Output is bounded by the NFR-2 file-count cap.

## Describe Metamodel [[tool_describe_metamodel: McpTool]]

Discover the workspace vocabulary: kinds, per-kind field names, and edge-type counts.

- tool_name: qmdc_describe_metamodel
- status: implemented
- args: path, namespace

### Description [[description: text]]

Returns which object kinds exist, the count and observed fields per kind, and edge-type counts. Call it first to learn the schema before writing [[#tool_query_sql]] or walking the graph with [[#tool_traverse_graph]]. An optional `namespace` limits the summary.

## Get Guide [[tool_get_guide: McpTool]]

Return the build-embedded QMDC agent guide.

- tool_name: qmdc_get_guide
- status: implemented

### Description [[description: text]]

Serves the QMD.md format guide (object/reference/field/namespace syntax) as a text block [[#qmdc_guide]]. It is the tools-only twin of the [[#res_guide]] resource — identical, byte-for-byte, build-embedded content — so a client that bridges only MCP tools (not resources) can still fetch the guide. No `path`, no index, cannot fail.
