# Build a Small Workspace [[guide_workspace: HowTo]]

- goal: build a multi-file workspace with namespaces and cross-file references
- audience: developer
- prerequisites: [[#guide_first_file]]
- outcome: a validated multi-file workspace graph
- about: [[#workspace]], [[#object]], [[#reference]]
- next: [[#guide_query]]

## Content Generator [[guide_workspace_gen: ContentGenerator]]

- target: [[#guide_workspace.content]]
- about: [[#workspace]], [[#object]], [[#reference]]
- sources_hash: f39f157d605d3ef0

### Prompt [[guide_workspace_gen_prompt: text]]

Write a guide: "Build a multi-file QMDC workspace from scratch."

Steps:

1. Create a project folder with `readme.qmd.md` containing `[[myproject:__Workspace]]`
2. Create a subfolder `api/` with `readme.qmd.md` containing `[[api:__Namespace]]`
3. Add a file `api/endpoints.qmd.md` with 2-3 objects
4. Add cross-file references between objects
5. Validate: `qmdc workspace validate .`
6. Query: `qmdc query . "SELECT __id, __file FROM objects"`

Show the full file tree and contents at each step. Explain namespaces, cross-file references, and how the workspace stitches files together into one graph.

End with: "Your workspace is ready. For workspace syntax details, see the Workspaces page."

## Content [[content: text]]

This guide walks you through creating a multi-file QMDC workspace from scratch. By the end you'll have a project with namespaces, cross-file [[#reference]]s, and a single queryable graph spanning multiple files.

**Prerequisites:** complete [[#guide_first_file]] first so you're familiar with objects and fields.

---

## Step 1: Create the workspace root

Create a project folder with a single anchor file:

```text
myproject/
└── readme.qmd.md
```

**`myproject/readme.qmd.md`:**

```qmd
# My Project [[myproject: __Workspace]]

- description: A sample multi-file QMDC workspace
```

The `[[myproject: __Workspace]]` declaration marks this directory as a [[#workspace]] root. Every `.qmd.md` file inside — at any depth — becomes part of one graph.

---

## Step 2: Add a namespace

Namespaces partition your workspace into logical sections. Create a subfolder with its own `readme.qmd.md`:

```text
myproject/
├── readme.qmd.md
└── api/
    └── readme.qmd.md
```

**`myproject/api/readme.qmd.md`:**

```qmd
# API [[api: __Namespace]]

- description: Public API layer
```

`[[api: __Namespace]]` makes the `api/` folder a namespace. All [[#object]]s in files under this folder automatically receive `__namespace: api`.

---

## Step 3: Add objects in the namespace

Create a file with a few objects:

```text
myproject/
├── readme.qmd.md
└── api/
    ├── readme.qmd.md
    └── endpoints.qmd.md
```

**`myproject/api/endpoints.qmd.md`:**

```qmd
# API Endpoints

## List Users [[list_users: Endpoint]]

- method: GET
- path: /api/users
- returns: array

## Get User [[get_user: Endpoint]]

- method: GET
- path: /api/users/:id
- returns: object

## Create User [[create_user: Endpoint]]

- method: POST
- path: /api/users
- returns: object
```

Each heading with `[[id: Kind]]` defines an [[#object]]. The workspace automatically assigns `__file: api/endpoints.qmd.md` and `__namespace: api` to each.

---

## Step 4: Add cross-file references

Connect objects across files. Edit the root to reference the endpoints:

**`myproject/readme.qmd.md`** (updated):

```qmd
# My Project [[myproject: __Workspace]]

- description: A sample multi-file QMDC workspace

## Architecture [[architecture]]

- public_endpoints: [[[#api:list_users]], [[#api:get_user]], [[#api:create_user]]]
```

The syntax `[[#api:list_users]]` is a cross-namespace [[#reference]] — it targets `list_users` in the `api` namespace. Within the same namespace you'd write just `[[#list_users]]`.

Add a reference between objects in the same file too:

**`myproject/api/endpoints.qmd.md`** (updated — last object only):

```qmd
## Create User [[create_user: Endpoint]]

- method: POST
- path: /api/users
- depends: [[#get_user]]
- returns: object
```

Every reference produces a typed edge in the graph. Here `depends` becomes the `edge_type`.

---

## Step 5: Validate the workspace

```bash
qmdc workspace validate .
```

Validation checks that:

- There is exactly one `__Workspace` declaration and no nested workspaces
- All `[[#...]]` references resolve to existing objects
- No ID collisions exist within the same Kind

Fix any warnings before moving on.

---

## Step 6: Query the graph

```bash
qmdc query . "SELECT __id, __file FROM objects"
```

Expected output:

| __id | __file |
|------|--------|
| myproject | readme.qmd.md |
| architecture | readme.qmd.md |
| api | api/readme.qmd.md |
| list_users | api/endpoints.qmd.md |
| get_user | api/endpoints.qmd.md |
| create_user | api/endpoints.qmd.md |

Query the edges to see how objects connect:

```bash
qmdc query . "SELECT source_id, target_id, edge_type FROM edges"
```

| source_id | target_id | edge_type |
|-----------|-----------|-----------|
| architecture | list_users | public_endpoints |
| architecture | get_user | public_endpoints |
| architecture | create_user | public_endpoints |
| create_user | get_user | depends |

---

## How it all fits together

| Concept | Declared via | Purpose |
|---------|-------------|---------|
| **Workspace** | `[[id: __Workspace]]` in root `readme.qmd.md` | Top-level container — all files in the tree become one graph |
| **Namespace** | `[[id: __Namespace]]` in a subfolder's `readme.qmd.md` | Logical partition; objects inherit `__namespace` automatically |
| **Cross-file reference** | `[[#namespace:id]]` or `[[#namespace:Kind:id]]` | Typed edge between objects in different files/namespaces |

The workspace layer:

1. Parses each `.qmd.md` file independently
2. Indexes all objects by `namespace:Kind:id`
3. Resolves every `[[#...]]` reference and reports broken links
4. Assigns metadata (`__file`, `__namespace`, `__workspace`) to each object
5. Builds the edge graph with typed relationships

Your workspace is ready. For the complete workspace syntax rules, see the [[#workspace]] reference page. To learn what you can do with the graph, continue to [[#guide_query]].
