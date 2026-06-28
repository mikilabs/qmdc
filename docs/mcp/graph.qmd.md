# Graph [[mcp_graph: Category]]

Tools that follow the reference graph — reverse lookups, walks, and path-finding over typed edges.

## Find References [[tool_find_references: McpTool]]

Reverse lookup: every object that references a given object.

- tool_name: qmdc_find_references
- status: implemented
- args: path, id

### Description [[description: text]]

Returns each referring object's file, line, id, and kind. Matching is by **resolved identity** — namespaced (`ns:id`), hierarchical (`a.b`), and `__local_id` references all resolve to the same target — not naive text matching. It shares this logic with the LSP [[#lsp:references]] feature, so both agree on membership. Results are bounded (see [[#mcp_bounded]]).

## Traverse Graph [[tool_traverse_graph: McpTool]]

Walk the reference graph from a start object.

- tool_name: qmdc_traverse_graph
- status: implemented
- args: path, start_id, direction, depth, edge_type

### Description [[description: text]]

Returns the objects and typed edges reachable from `start_id` within `depth` hops (1–50, default 3). `direction` is `outgoing` (default), `incoming`, or `both`; `edge_type` optionally restricts to references whose field name matches. The walk is cycle-safe (visited set) and depth-bounded. For the link between two specific objects, use [[#tool_find_path]].

## Find Path [[tool_find_path: McpTool]]

Find a connecting path between two objects.

- tool_name: qmdc_find_path
- status: implemented
- args: path, from_id, to_id, edge_type

### Description [[description: text]]

Returns the ordered chain of objects and edges that connects `from_id` to `to_id` through their references, or a clearly-marked no-path result. An optional `edge_type` restricts which references may be followed.

## Validate References [[tool_validate_references: McpTool]]

Check the workspace (or one file) for broken or ambiguous references.

- tool_name: qmdc_validate_references
- status: implemented
- args: path, file

### Description [[description: text]]

Returns diagnostics, each with file, line, code (`QMDC001` = not-found, `QMDC002` = ambiguous), and a message (the did-you-mean suggestion is folded into the message). An optional `file` limits the check to one workspace-relative path. It is the tool form of the LSP [[#lsp:diagnostics]] feature; results are bounded (see [[#mcp_bounded]]).
