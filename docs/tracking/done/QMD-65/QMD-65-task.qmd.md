# QMD-65: LSP emits OS-native path separators (and CRLF) on Windows

## LSP output uses `\` and `\r\n` on Windows, breaking cross-platform microtests [[qmd65: Bug]]

On Windows, the LSP renders file locations with backslash separators (`\`) and
carries CRLF (`\r\n`) in text content, so `definition` / `hover` / `references`
/ `completion` results diverge from the `/`-and-`\n` shapes the fixtures (and
every other platform) expect. This fails `test_all_lsp_microtests` on
`windows-latest` and, via the release CI gate (`needs: test`), **blocks the
release**.

- status: done
- priority: high
- category: lsp
- related_task: [[#qmd63]]
- requires_changes: []
- findings: [[[#qmd65_finding_split]], [[#qmd65_finding_scope]], [[#qmd65_finding_tests]]]
- result: [[#qmd65_result]]

### Symptom (Windows CI) [[qmd65_symptom: text]]

`binary (windows-latest)` → `test_all_lsp_microtests` FAILED: 110 passed, 7
failed. All 7 are path/nesting shapes:

- `completion/026-workspace-path`
- `hover/012-deep-nested`
- `definition/009-workspace-path`
- `definition/013-nested-workspace-path`
- `definition/014-deep-nested-spec`
- `definition/015-cross-namespace`
- `references/007-deep-nested`

macOS and Linux binary jobs pass; local `make test-fast` (macOS) is green.

### Evidence [[qmd65_evidence: text]]

From the failing job diff:

- definition/references `uri`: expected `workspace/db/models.qmd.md`, actual
  `workspace/db\models.qmd.md` — only the deeper component is a backslash, so
  the URI is a stored `/` base joined with an OS-separator relative path.
- hover `📁` line: expected `architecture/domain/readme.qmd.md`, actual
  `architecture\domain\readme.qmd.md`.
- hover content: expected `\n` between table rows, actual `\r\n` (CRLF from
  reading the file on Windows / git `autocrlf`).

### Root cause (hypothesis) [[qmd65_cause: text]]

- **Separators:** relative paths for `uri`/`file`/the hover `📁` line are built
  with `std::path::Path` join / `strip_prefix`, which yields `\` on Windows.
  They are never normalised to forward slashes before being emitted as LSP
  strings. (QMD uses `/` as the canonical logical separator everywhere else.)
- **Line endings:** file content read for hover is not normalised to LF, so
  CRLF files surface `\r\n` in the rendered content.

This is a **pre-existing cross-platform bug**, independent of QMD-63 (which
changed resolution direction/depth, not path rendering).

### Fix direction [[qmd65_fix: text]]

- Normalise emitted path strings to `/` at the LSP output boundary (a single
  helper applied where `uri`/`file`/`📁` relative paths are produced), rather
  than sprinkling replacements. Prefer fixing at the point relative paths are
  computed so both LSP and any shared formatter benefit.
- Normalise text content line endings to `\n` (strip `\r`) when building hover
  content (or when reading file content for display).
- Keep behaviour identical on macOS/Linux (they already emit `/` and `\n`).

### Constraints — no Windows machine [[qmd65_constraints: text]]

- **No local Windows access.** The fix is written to be OS-independent and
  validated by the existing **data-driven** `test_all_lsp_microtests` (which
  encode the `/`+`\n` expectations) run on `windows-latest` in CI. Local
  (macOS) `make test-fast` must stay green; the Windows result is confirmed on
  the next CI run after push.
- Prefer a normalisation that is a no-op on Unix (where separator is already
  `/`), so macOS/Linux fixtures are unaffected.

### Open questions (for triage) [[qmd65_open_questions: text]]

1. Normalise at the lowest shared point (where relative paths are computed) vs.
   at each LSP emit site — pick the single-source-of-truth location.
2. CRLF: strip `\r` only in hover-display content, or normalise on file read
   more broadly (risk of affecting offsets/positions — must not shift LSP
   ranges).
3. Are the MCP tools affected too (they emit `__file` from the index)? Check
   whether MCP path output needs the same normalisation or is already `/`.

## Checklist

- [x] Understood the task
- [x] Studied the code (LSP uri/file/hover path emission, content read)
- [x] Created a plan and prototypes in `artifacts/`
- [x] Tested the solution
- [x] Moved the code into the project
- [x] Created Result.md and Findings.md
