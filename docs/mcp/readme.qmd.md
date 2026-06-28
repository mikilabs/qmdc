# MCP Server [[mcp:__Namespace]]

- categories: [[#mcp_discovery]], [[#mcp_graph]], [[#mcp_query]], [[#mcp_refactoring]], [[#mcp_resources]], [[#mcp_server]]
- depends: [[#parsers:rust_parser]], [[#lsp]]

Model Context Protocol server for QMDC. Exposes the same workspace intelligence that powers the [[#lsp]] — reference resolution, tree, search, SQL, graph walks, rename — to AI agents over JSON-RPC 2.0 on stdio.

Run it with `qmdc mcp`. The server reuses the transport-agnostic `core` layer shared with the LSP, so an MCP tool and the matching editor feature return the same answers.

The surface is **14 tools** (`tools/call`) plus **4 resources** (`resources/read`). Tools are grouped into categories: [[#mcp_discovery]], [[#mcp_graph]], [[#mcp_query]], and [[#mcp_refactoring]]. Cross-cutting behavior (workspace anchoring, security, read-only SQL, bounded output, error envelope) is described in [[#mcp_server]].

## Workspace Anchoring [[mcp_path_anchor: text]]

Every index-backed tool takes a `path` argument: any file or directory inside the target QMDC workspace. The server walks upward from `path` to find the nearest enclosing workspace root (a `readme.qmd.md` declaring `__Workspace`/`__Namespace`), then indexes that whole workspace. The operation covers the entire workspace, not just the file at `path`.

## Tool Naming [[mcp_tool_naming: text]]

All tools share the `qmdc_` prefix so they never collide with other MCP servers a client has mounted. Names front-load the verb and object: `qmdc_locate_object`, `qmdc_find_references`, `qmdc_query_sql`, and so on.
