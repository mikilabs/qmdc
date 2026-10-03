# QMD-84: qmdc mcp could not compose explicit workspaces

## qmdc mcp -w [[qmd84_mcp_with: Feature]]

GitHub [#10](https://github.com/mikilabs/qmdc/issues/10), MCP part. 2.0.0 shipped `-w` for `query`,
`workspace parse` and `workspace validate` (QMD-72), but `qmdc mcp` only composed a container of
sibling workspaces, so criteria 8 (MCP sees the same composed graph) and 10 (`--force-root` rejects
`-w` paths outside it) were open.

- status: done
- priority: medium
- category: mcp
- related_task: [[#qmd72]]
