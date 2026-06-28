# Workspace [[workspace: SyntaxConcept]]

- depends: [[#object]], [[#reference]]

## Description [[description: text]]

A workspace is a directory of QMD.md files that can reference each other. Workspaces provide multi-file structure with automatic object indexing and reference validation.

## Syntax [[syntax: text]]

Workspace structure:

```text
my-project/
├── readme.qmd.md              # Workspace root (__Workspace)
├── users.qmd.md
├── storage/
│   ├── readme.qmd.md          # Namespace "storage" (__Namespace)
│   ├── tables.qmd.md
│   └── indexes.qmd.md
└── api/
    ├── readme.qmd.md          # Namespace "api" (__Namespace)
    └── endpoints.qmd.md
```

Workspace is defined in root `readme.qmd.md` via `[[id: __Workspace]]`. Namespace is defined in subfolder `readme.qmd.md` via `[[id: __Namespace]]`. All files in a folder inherit `__workspace` and `__namespace` from their anchor files.

## Cross File References [[cross_file_refs: text]]

The parser automatically:

1. Finds all objects in all files
2. Indexes them by `namespace:Kind:id`
3. Validates all references
4. Reports broken links

Cross-namespace reference format: `[[#namespace:id]]` or `[[#namespace:Kind:id]]`.
Cross-workspace reference format: `[[#workspace:namespace:Kind:id]]`.

## Object Metadata [[object_metadata: text]]

When parsing a workspace, each object gets:

- `__file` — relative file path
- `__line` — line number
- `__workspace` — reference to `__Workspace` object
- `__namespace` — reference to `__Namespace` object (or null for root)

## Rules [[rules: text]]

- Workspace is defined by `__Workspace` kind in root `readme.qmd.md`
- Namespace is defined by `__Namespace` kind in subfolder `readme.qmd.md`
- All files inherit workspace and namespace from nearest anchor file
- If no anchor found: `__workspace: "default"`, `__namespace` not set
- Nested workspaces are forbidden (`nested_workspace` error)
- Workspace and namespace don't affect local references within a single file
