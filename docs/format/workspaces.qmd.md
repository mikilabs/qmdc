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
2. Indexes them by `workspace:namespace:id` (its `__global_id`)
3. Validates all references
4. Reports broken links

Cross-namespace reference format: `[[#namespace:id]]`.
Cross-workspace reference format: `[[#workspace:namespace:id]]`, or
`[[#workspace::id]]` to elide the namespace and match any namespace of that workspace.
A cross-workspace reference MUST name its workspace: a bare `[[#id]]` never reaches into a
sibling workspace.

## Object Metadata [[object_metadata: text]]

When parsing a workspace, each object gets:

- `__file` — the file's path relative to the base of the result: the workspace root for a single
  workspace, the given directory for a directory holding several, and the workspace's own id for
  workspaces composed with `-w`, which need not share a directory. The result's `workspaces` list
  maps each workspace to its location on disk (see the `workspace parse` output)
- `__line` — line number
- `__workspace` — reference to `__Workspace` object
- `__namespace` — reference to `__Namespace` object (or null for root)

## Rules [[rules: text]]

- Workspace is defined by `__Workspace` kind in root `readme.qmd.md`
- Namespace is defined by `__Namespace` kind in subfolder `readme.qmd.md`
- All files inherit workspace and namespace from nearest anchor file
- If no anchor found: `__workspace: "default"`, `__namespace` not set
- A workspace inside another one is not part of it: a parse of the outer workspace alone reports it as `nested_workspace`, because its files are missing from the result; composing both (`-w outer -w outer/inner`) reports nothing
- Workspace and namespace don't affect local references within a single file
