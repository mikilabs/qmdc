# Workspace A [[qmd63_dup_id: __Workspace]]

- description: QMD-63 — first of two sibling workspaces under the container

QMD-72 narrowed what `ambiguous` means. A container of sibling workspaces is now COMPOSED,
because the CLI has always composed one and neither candidate alone can see the other's
objects. The surviving ambiguous case is the one a composed graph cannot represent: two
members declaring the SAME workspace id, so every id in one collides with the other's and no
reference could resolve to a single object. That is why both readmes here declare
`qmd63_dup_id`.

## Alpha [[alpha: Thing]]

- value: 1
