# MCP Server [[mcp:__Namespace]]

- categories: [[#mcp_discovery]], [[#mcp_graph]], [[#mcp_query]], [[#mcp_refactoring]], [[#mcp_resources]], [[#mcp_server]]
- depends: [[#parsers:rust_parser]], [[#lsp]]

Model Context Protocol server for QMDC. Exposes the same workspace intelligence that powers the [[#lsp]] — reference resolution, tree, search, SQL, graph walks, rename — to AI agents over JSON-RPC 2.0 on stdio.

Run it with `qmdc mcp`. The server reuses the transport-agnostic `core` layer shared with the LSP, so an MCP tool and the matching editor feature return the same answers.

The surface is **14 tools** (`tools/call`) plus **4 resources** (`resources/read`). Tools are grouped into categories: [[#mcp_discovery]], [[#mcp_graph]], [[#mcp_query]], and [[#mcp_refactoring]]. Cross-cutting behavior (workspace anchoring, security, read-only SQL, bounded output, error envelope) is described in [[#mcp_server]].

## Workspace Anchoring [[mcp_path_anchor: text]]

Every index-backed tool takes a `path` argument: any file or directory inside — or containing — the target QMDC workspace. The server resolves the workspace root **down first, then up**: if `path` itself is a workspace it is used; otherwise the server scans **downward** for a workspace root (a `readme.qmd.md` declaring `__Workspace`), and only if none is found does it walk **upward** to the nearest enclosing one. It then indexes that whole workspace — the operation covers the entire workspace, not just the file at `path`.

The downward scan is bounded to **5 directory levels** (a fixed, non-configurable default) and honours `.qmdcignore`, so pointing a tool at a large repository root stays cheap. If the scan finds **more than one** workspace below `path`, they are **composed into a single graph**, so a reference from one to another resolves exactly as the CLI's `--with` form resolves it — indexing one of them alone could not see the others' objects, so picking a single candidate is not a remedy. The scan returns an `ambiguous` error (listing the candidate roots and the depth searched) only when two of those workspaces declare the **same** workspace id, which no composed graph can represent. A marker deeper than 5 levels is not discovered by the downward scan — point `path` closer to it.

## Tool Naming [[mcp_tool_naming: text]]

All tools share the `qmdc_` prefix so they never collide with other MCP servers a client has mounted. Names front-load the verb and object: `qmdc_locate_object`, `qmdc_find_references`, `qmdc_query_sql`, and so on.
