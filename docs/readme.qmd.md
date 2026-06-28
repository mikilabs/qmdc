# QMDC Documentation [[docs:__Workspace]]

- version: 2.0
- namespaces: [[#format]], [[#parsers]], [[#architecture]], [[#lsp]], [[#mcp]], [[#extension]], [[#semantic]], [[#testing]], [[#tutorials]], [[#guides]], [[#learn]], [[#ssg]], [[#tracking]], [[#releasing]]

## Metamodel [[metamodel: Metamodel]]

### Anchor Kinds [[anchor_kinds: text]]

| Kind             | Definition                                                          |
| ---------------- | ------------------------------------------------------------------- |
| SyntaxConcept    | A core concept of the QMD.md format (objects, fields, references, etc) |
| ValidationError  | A specific error type with code, severity, and message              |
| Parser           | A language-specific QMDC parser implementation                       |
| Command          | A CLI command exposed by qmdc                                       |
| Algorithm        | A specific algorithm used in parsing, search, or graph operations   |
| LSPFeature       | An LSP capability (completion, diagnostics, navigation, etc)        |
| DiagnosticRule   | A specific diagnostic check the LSP performs                        |
| CompletionContext | A trigger context for LSP completions                              |
| ExtCommand       | A VS Code extension command                                         |
| TestSuite        | A data-driven test suite with location and format                   |
| Idea             | A future improvement concept                                        |
| NarrativeDoc     | A document whose sections explain anchors via about links           |
| Tutorial         | A hands-on lesson that builds skill by doing (learn-by-doing)        |
| HowTo            | A practical recipe that accomplishes one real-world task            |
| Explanation      | A conceptual discussion that builds understanding (the "why")       |
| Category         | A grouping container that nests related objects for tree hierarchy  |
| McpTool          | A tool exposed by the qmdc MCP server (tools/call)                  |
| McpResource      | A resource exposed by the qmdc MCP server (qmdc:// URI)              |

### Link Roles [[link_roles: text]]

| Role           | Meaning                                                    |
| -------------- | ---------------------------------------------------------- |
| `about`        | This section describes or explains X                       |
| `depends`      | This anchor relies on another anchor at runtime            |
| `implements`   | This thing realizes/provides X (parser implements format)  |
| `tests`        | This test suite tests X                                    |
| `validates`    | This diagnostic rule checks for this error                 |
| `related_to`   | Loose conceptual association (for ideas, findings)         |
| `affects`      | This task/bug changes X                                    |

### Tree Nesting [[tree_nesting: text]]

**Standard:** group related objects under a container so the `smart` tree (and the Objects Explorer) shows a real hierarchy instead of a flat list.

- Each topic file opens with a level-1 `# Title [[id: Category]]` container object.
- The items in that file are written as deeper headings (`##`, `###`) under the container, so the parser sets their `__parent` and the smart tree nests them: `namespace → Category → item`.
- Item ids stay short; references like `[[#item]]` still resolve via `__local_id` (see [[#lsp:definition]]), so introducing a container never breaks existing links.

Without a container, every `##` object in a namespace has `__parent: null` and renders as a flat sibling list. The `lsp` and `mcp` namespaces both follow this standard.
