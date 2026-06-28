# Tree Order Casefold [[tree_order_casefold:__Workspace]]

- namespaces: [[#learn]], [[#lsp]]

Regression workspace for case-insensitive tree ordering. Sibling labels must sort
case-insensitively: "Learn" before "LSP Server" (SQLite BINARY collation wrongly
orders the all-caps `LSP` first), and likewise "Lexer" before "LSP Hover".
