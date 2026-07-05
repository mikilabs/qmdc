# QMD-65: Findings

Triage of the Windows-only LSP path/EOL divergence blocking the release gate.

## Two independent defects [[qmd65_finding_split: Finding]]

The 7 Windows microtest failures split into two root causes.

- category: lsp
- related_to: [[#qmd65]]
- solution: normalise emitted relative paths to `/`; force LF for `.qmd.md` (and optionally strip `\r` in hover display)

### A. Backslash separators in emitted relative paths [[qmd65_sep: text]]

`__file` is stored as a workspace-relative path computed with
`Path::strip_prefix(..).to_string_lossy()`, which yields OS separators — `\` on
Windows. Two computation sites:

- `qmdc-rs/src/lsp/workspace.rs` ~104 — `relative_path` in the incremental
  index update.
- `qmdc-rs/src/lsp/server.rs` ~447 — `__file` adjust in `scan_workspace_folder`.

The hover `📁 {file}` line (`handlers/hover.rs:144`) reads `__file` directly, so
it inherits the `\`. QMD's canonical logical separator is `/` everywhere else.

**`__file` is consumed by logic, not just display.** `completion/026-workspace-path`
is not a cosmetic mismatch: it expects `orders`/`users` (from `db/models.qmd.md`)
but Windows returns `main` — a *different* object set. Completion path-matches
candidates by `__file`; with `\` separators the match against the `/`-form fails
and it falls back to the wrong file. So the fix normalises at the **`__file`
source** (so both matching and rendering see `/`).

**Correction found via Wine (see [[#qmd65_finding_tests]]):** the
`definition`/`references` `uri` mismatches are NOT a production bug. Those results
carry a real `file://` URL (always `/`); the **test harness**
`uri_to_relative_path` (tests/lsp.rs) rebuilt the relative path with
`Path::display()`, which renders `\` on Windows. So that half is a harness fix
(normalise the harness output to `/`), while hover `📁` + completion/026 are the
genuine production `__file` fixes.

**Fix:** normalise at the two `__file` computation sites with
`s.replace(std::path::MAIN_SEPARATOR, "/")`. This is a **no-op on Unix**
(`MAIN_SEPARATOR == '/'`), so macOS/Linux output is unchanged, and it fixes
Windows. Fixing at the `__file` source (rather than each emit site) is the
single-source-of-truth point — `uri`/hover/etc. all derive from `__file`.

### B. CRLF in hover content [[qmd65_crlf: text]]

`hover` renders field values (e.g. the `task_statuses` table) that contain
`\r\n` on Windows. Source: `.gitattributes` has no EOL rule (only the `*.db`
LFS line), so with `core.autocrlf=true` (Windows default) `.qmd.md` files check
out as CRLF; `read_to_string` keeps `\r\n`; the parser stores it in the field
value; hover shows `\r\n` while fixtures expect `\n`.

**Fix (preferred):** add `.gitattributes` rule `*.qmd.md text eol=lf` (and the
`.expected.json`/fixture inputs as needed). CI does a fresh checkout, so this
deterministically gives LF on Windows → parser reads LF → hover matches.

**Do NOT** normalise line endings on file *read before parsing*: LSP positions
are mapped against the editor's buffer (which may be CRLF), so stripping `\r`
pre-parse would shift ranges. If extra robustness is wanted, strip `\r` only in
the **hover display string** (display-only, offset-safe) — see open questions.

## Affected / not-affected surfaces [[qmd65_finding_scope: Finding]]

What the fix touches and what to double-check.

- category: lsp
- related_to: [[#qmd65]]
- solution: confirm MCP `__file` output and CLI paths share the normalisation need or are already `/`

### Notes [[qmd65_scope_notes: text]]

- The 7 failing cases are all `definition`/`hover`/`references`/`completion`
  results that emit a file location: `completion/026-workspace-path`,
  `hover/012-deep-nested`, `definition/009-workspace-path`,
  `definition/013-nested-workspace-path`, `definition/014-deep-nested-spec`,
  `definition/015-cross-namespace`, `references/007-deep-nested`.
- MCP tools emit `__file` from the index too — check whether they need the same
  `/`-normalisation (parse_workspace's `__file` on Windows). If so, normalise at
  the shared parse layer instead of only the LSP sites. (Open question 3.)
- Independent of QMD-63: resolution direction/depth is unrelated to path-string
  rendering; these fail only on Windows and pass on macOS/Linux.

## Test plan [[qmd65_finding_tests: Finding]]

Verification without a Windows machine.

- category: testing
- related_to: [[#qmd65]]
- solution: rely on the existing data-driven `test_all_lsp_microtests` on windows-latest CI; keep macOS green locally

### How to verify [[qmd65_tests_verify: text]]

- **Local Wine validation (done).** No Windows machine, so cross-compiled the
  LSP test to `x86_64-pc-windows-gnu` (mingw-w64) and ran it under Wine 11
  (Rosetta 2) with `CARGO_TARGET_X86_64_PC_WINDOWS_GNU_RUNNER=wine`. Before the
  fix, Wine reproduced the separator failures (`\` in paths); after the fix
  `test_all_lsp_microtests` passes under Wine. Wine validates **Fix A**
  (separators) — a windows-target binary uses `\` regardless of host FS.
- **Wine does NOT validate CRLF (Fix B).** Wine reads the host's LF files, so
  CRLF never reproduces there; `.gitattributes` (`*.qmd.md text eol=lf`) is
  confirmed only by `windows-latest` CI (real git checkout with autocrlf).
- Locally (macOS) `make test-fast` stays green — separator normalise is a no-op
  on Unix, `.gitattributes` doesn't change already-LF files.
- No new Rust test code: the existing `test_all_lsp_microtests` fixtures are the
  oracle (run under Wine locally + Windows in CI).
