# Workspace A [[qmd72_meta_a: __Workspace]]

The shape `tests/mcp/qmd63-ambiguous` used to refuse: a non-workspace container holding two
sibling workspaces with DISTINCT ids. QMD-72 composes it, so this fixture keeps the composed
success pinned for a second MCP tool — not just `qmdc_validate_references` — because the seam
is shared and a per-tool divergence would be invisible from one fixture.

## Alpha [[alpha: Thing]]

- value: 1
