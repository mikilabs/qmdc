# QMD-63: MCP workspace resolution fails when given the project root

## qmdc MCP tools fail to find a workspace that lives in a subdirectory [[qmd63: Bug]]

Calling a `qmdc` MCP tool with the **project/repo root** as `path` fails to
resolve the workspace, even though a valid QMD workspace exists in a
subdirectory (`docs/`). The error surfaces as a `.git`-boundary failure.

- status: done
- priority: medium
- category: parser
- requires_changes: []
- findings: []
- result: null

### Reproduction [[qmd63_repro: text]]

```text
Tool: qmdc_describe_metamodel
Args: {"path": "<repo-root>"}

MCP Tool Error Response:
{"success":false,"error":{"code":"not-resolved",
 "message":"no workspace root found (hit .git boundary at
  '<repo-root>')"}}
```

### Observed behaviour [[qmd63_observed: text]]

The resolver in `qmdc-rs/src/core/index_seam.rs` walks **upward** from the
given `path`, looking for a `readme.qmd.md` that declares `__Workspace` or
`__Namespace`. At each level it also checks for `.git` and treats it as a hard
stop boundary.

When `path` is the repo root:

1. The repo root has only `README.md` (not `readme.qmd.md`) and no workspace
   marker, so no root is found at that level.
2. `.git` exists at the repo root, so the walk stops immediately and returns
   `not-resolved`.

The actual QMD workspace lives **below** the repo root (`docs/`, e.g.
`docs/tracking/readme.qmd.md` declares `[[tracking: __Namespace]]`). Because
the resolver only searches upward, it never discovers a workspace that sits in
a subdirectory of the supplied path.

### Environment notes [[qmd63_env: text]]

- This checkout is a **git worktree**: `.git` at the repo root is a regular
  *file* (a gitlink, ~68 bytes), not a directory. `current.join(".git").exists()`
  is true for the file as well, so the boundary check behaves the same as a
  normal clone — the worktree is likely not the root cause, but should be
  confirmed during triage.
- `./qmdc query ./docs "..."` works fine (294 objects), confirming the
  workspace is valid and lives in `docs/`.

### Impact [[qmd63_impact: text]]

The MCP `path` argument is documented as "Any file or directory inside the
target QMD workspace (locates the workspace root)". An agent's natural first
guess is the project/repo root, which fails with a confusing `.git`-boundary
error and gives no hint that a workspace exists in a subdirectory. This blocks
the qmdc MCP tools entirely for this repo layout unless the caller already
knows to pass `.../docs`.

### Desired direction [[qmd63_direction: text]]

The resolver should search **downward first, then upward** from the given
`path`, and it should be **shared with the LSP**, not an MCP-specific resolver.
The LSP already resolves this repo correctly today (downward `.qmdcignore`-aware
discovery of `__Workspace` roots + a prefix-based owner lookup that assumes
nesting is disallowed). The fix is to consolidate that logic into one
transport-agnostic core primitive and have MCP (and ideally the LSP) call it,
rather than maintaining a third resolver.

Rationale for down-first: an agent caller usually points at somewhere inside the
project (the repo root, or a content subdir), so the workspace most often sits
*at or below* that path. Searching the subtree first matches the common case;
the upward walk becomes the special case (caller is already deep inside a
workspace and points below its root).

Failing with "nothing found" while a valid workspace sits in a subdirectory
(`docs/`) is the core problem — the tool sees nothing and just says "nope".
Downward-first resolution closes that gap so passing the repo root "just works".
If neither direction finds a workspace, the error message must be actionable
(point the caller at how to locate the workspace).

### Multiple workspaces → ask, don't guess [[qmd63_ambiguity: text]]

If the downward search finds **more than one distinct workspace**, the tool
must NOT auto-pick. This is an **error**, returned via the standard
`{success:false, error:...}` envelope with a new `ambiguous` code, carrying the
list of found workspace paths inside the error (`candidates`) so the caller can
re-call with the one they want. Workspaces cannot be nested (that is an invalid
config), so multiple hits are always disjoint siblings — never a tie-break.
Keeping it in the error envelope avoids introducing a second success-shaped
response format — a tool call either succeeds with a normal result or fails
with the standard error. A single unambiguous match resolves directly with no
prompt.

(Context: the qmdc core and the VS Code extension via LSP already resolve fine
in their own entry points; this gap is specific to the MCP metamodel/tool path
where the caller hands in an arbitrary `path`.)

### Open questions (for triage) [[qmd63_open_questions: text]]

1. How to bound the downward search: max depth, and which directories to skip
   (honour `.qmdcignore`, skip `.git`, `node_modules`, `target`, etc.).
2. Exact error code for the ambiguous case — reuse `not-resolved` or add an
   `ambiguous` code — and how the candidate list is shaped inside the error.
3. Confirm the order: downward search first, upward walk as fallback. Since
   nested workspaces are illegal, a path already inside a workspace yields zero
   down-hits and falls through to the up-walk — so existing behaviour is
   preserved (confirm with a test).
4. Final error message when neither direction finds anything — what wording
   is actionable for an agent caller.
5. Confirm the git-worktree `.git`-as-file detail is irrelevant to the failure
   (vs. a plain clone).

## Checklist

- [ ] Understood the task
- [ ] Studied the code (`qmdc-rs/src/core/index_seam.rs` resolver)
- [ ] Created a plan and prototypes in `artifacts/`
- [ ] Tested the solution
- [ ] Moved the code into the project
- [ ] Created Result.md and Findings.md
