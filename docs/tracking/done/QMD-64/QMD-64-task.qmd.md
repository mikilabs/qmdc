# QMD-64: LSP diagnostics report false QMDC001 for refs to externally-changed files

## LSP validation goes stale when files change outside the editor [[qmd64: Bug]]

The LSP validator reports `Object 'X' not found` (QMDC001) for a `[[#X]]`
reference even though the object exists and everything else (hover, the card,
definition, CLI `validate`) resolves it fine. It clears after
`Restart Language Server`. This is a **stale workspace-index** bug, not a
resolution-logic bug.

- status: done
- priority: medium
- category: lsp
- related_task: [[#qmd63]]
- requires_changes: []
- findings: []
- result: null

### Reproduction [[qmd64_repro: text]]

1. With the workspace open in the editor, create or move a `*.qmd.md` file
   **outside** the editor (e.g. a tool writes it, a folder is `mv`'d — as
   happened while working QMD-63: the task file was written to disk and the
   folder moved `planned/ → active/`).
2. In another already-open file, add a reference to an object defined in that
   externally-changed file (e.g. `feature: [[#qmd63]]`).
3. The LSP underlines the reference with QMDC001 "Object 'qmd63' not found".
4. Hover / card / go-to-definition on the same reference resolve correctly, and
   CLI `qmdc workspace validate ./docs` reports 0 errors.
5. `Restart Language Server` → full rescan → the false error disappears.

### Root cause (diagnosed) [[qmd64_cause: text]]

- `Backend::compute_diagnostics` (`qmdc-rs/src/lsp/server.rs`) resolves
  references against the **cached** workspace index `ws.objects`, obtained via
  `find_workspace_for_file`. That index is only refreshed on the editor's
  `did_open`/`did_save` (and initial scan). Files created/moved on disk outside
  the editor are not re-scanned, so newly-added objects are missing from
  `ws.objects` → false QMDC001 for cross-file references to them.
- Hover works because `find_object_in_workspace_with_namespace` checks the
  **open document** first and resolves live, so it does not depend on the stale
  index the same way.
- CLI `validate` always parses from disk, so it never sees the stale state
  (this is why `make test-fast` is green while the editor shows red).

### Why existing tests miss it [[qmd64_gap: text]]

The LSP/CLI parity tests cover reference **resolution** (given an index, do LSP
and CLI agree). They do **not** cover **cache invalidation on external file
changes** — there is no test that creates/moves a file on disk (outside
`did_open`/`did_save`) and asserts diagnostics in an already-open referencing
file refresh. Related: `qmdc-rs/tests/lsp_stale_diagnostics.rs`,
`lsp_did_save_sync.rs`.

### Open questions (for triage) [[qmd64_open_questions: text]]

1. Should the LSP handle `workspace/didChangeWatchedFiles` (register file
   watchers for `**/*.qmd.md`) to rescan on external create/delete/move, or is
   the extension expected to send those notifications?
2. Or should diagnostics resolution fall back to a live re-scan / disk read when
   an id is missing from the cached index (cheaper to implement, but papers over
   the stale index for other features)?
3. Scope: is a broad rescan-on-change acceptable given the NFR-2 reparse bound,
   or does invalidation need to be incremental?
4. Confirm this is independent of QMD-63 (it is: CLI validate clean; the
   QMD-63 resolver change does not affect single-workspace file→workspace
   mapping).

## Checklist

- [x] Understood the task
- [x] Studied the code (`compute_diagnostics`, workspace index refresh, fs watch)
- [x] Created a plan and prototypes in `artifacts/`
- [x] Tested the solution
- [x] Moved the code into the project
- [x] Created Result.md and Findings.md
