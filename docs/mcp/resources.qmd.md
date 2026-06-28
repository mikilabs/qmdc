# Resources [[mcp_resources: Category]]

The four `qmdc://` resources served via `resources/read`. Resources are the read-only, addressable counterpart to the tools; dynamic ones take the same `path` workspace anchor.

## Guide [[res_guide: McpResource]]

The QMDC agent guide.

- uri: qmdc://guide
- mimeType: text/markdown
- status: implemented

### Description [[description: text]]

Static, build-embedded guide to the QMD.md format [[#qmdc_guide]]. Served byte-for-byte from the binary (no runtime file read). The [[#tool_get_guide]] tool exposes the identical content for tools-only clients.

## Tree [[res_tree: McpResource]]

The workspace tree (smart mode).

- uri: qmdc://tree
- mimeType: application/json
- status: implemented

### Description [[description: text]]

Dynamic smart-mode tree of the workspace at `path` — the resource form of [[#tool_get_tree]] with `mode: smart`.

## Object [[res_object: McpResource]]

A single object card by id.

- uri: qmdc://object/{id}
- mimeType: application/json
- status: implemented

### Description [[description: text]]

Dynamic object description — the resource form of [[#tool_describe_object]]. The `{id}` segment is percent-decoded, so ids containing `:` or spaces can be requested as `qmdc://object/ns%3Ausers`.

## Diagnostics [[res_diagnostics: McpResource]]

Workspace validation diagnostics.

- uri: qmdc://diagnostics
- mimeType: application/json
- status: implemented

### Description [[description: text]]

Dynamic broken-link / ambiguous-reference diagnostics for the workspace at `path` — the resource form of the [[#lsp:diagnostics]] feature and the `qmdc_validate_references` tool.
