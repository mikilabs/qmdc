# QMD-65: Result

## Windows path/EOL normalisation for LSP output [[qmd65_result: Result]]

The LSP no longer emits OS-native backslashes on Windows: relative `__file`
paths are normalised to `/` at the source, the test harness renders relative
paths with `/`, and `.qmd.md` files are pinned to LF. This clears the 7
`windows-latest` `test_all_lsp_microtests` failures that were gating the release.
Validated locally by cross-compiling to `x86_64-pc-windows-gnu` and running the
LSP test suite under Wine (separators), plus macOS `make test-fast` green; CRLF
is confirmed by Windows CI.

- feature: [[#qmd65]]
- files_changed: [qmdc-rs/src/workspace.rs, qmdc-rs/src/lsp/server.rs, qmdc-rs/src/lsp/workspace.rs, qmdc-rs/tests/lsp.rs, .gitattributes]
- tests_added: [qmdc-rs/src/workspace.rs::qmd65_tests]

### What changed [[qmd65_result_changes: text]]

- `workspace.rs`: added `path_to_slash(&Path) -> String` and its testable core
  `sep_to_slash(&str, char)`. Production replaces **only the platform separator**
  (`MAIN_SEPARATOR`), so it is a genuine no-op on Unix and converts `\` → `/` on
  Windows. Deliberately NOT an unconditional `\` → `/` replace: on Unix a
  backslash is a valid filename character (only `/` and NUL are forbidden), so
  `weird\name.qmd.md` is a single component that must be preserved. Applied at
  every relative `__file`/`files`/`error.file` source in the shared parser
  (`scan_workspace`, `parse_all_workspaces`) — so CLI/MCP `__file` output is `/`
  on Windows too (the triage open question).
- `lsp/server.rs` + `lsp/workspace.rs`: the two LSP `__file` computation sites
  now use `path_to_slash`. This fixes the hover `📁 {file}` line and the
  completion path-matching (`completion/026`: it matched candidates by `__file`,
  so `\` made it resolve the wrong file → `main` instead of `orders`/`users`).
- `workspace.rs` `#[cfg(test)] mod qmd65_tests`: unit tests for the fix. The
  Windows branch is driven on any host by calling `sep_to_slash(_, '\\')`
  directly (a no-op-on-Unix wrapper is otherwise untestable on macOS); a
  companion test asserts a literal `\` in a Unix filename is preserved.
- `tests/lsp.rs`: `uri_to_relative_path` normalised to `/` (`display()` rendered
  `\` on Windows). This was a **harness** bug — production `definition`/
  `references` results carry real `file://` URLs (already `/`); only the test's
  relative-path comparison was wrong.
- `.gitattributes`: `*.qmd.md text eol=lf` — stops `core.autocrlf` from checking
  out CRLF on Windows, which had leaked `\r\n` into hover content (`hover/012`).

### Verification [[qmd65_result_verify: text]]

- **Wine (local, no Windows machine):** cross-compiled the LSP test to
  `x86_64-pc-windows-gnu` (mingw-w64) and ran it under Wine 11 via
  `CARGO_TARGET_X86_64_PC_WINDOWS_GNU_RUNNER=wine`. Pre-fix Wine reproduced the
  `\` failures; post-fix `test_all_lsp_microtests` passes. Wine covers the
  separator fix (windows-target binary uses `\` regardless of host).
- **macOS:** `make test-fast` green (3200 cases, 0 failures, incl. the 3 new
  `qmd65_tests` unit tests) — the separator normalise is a no-op on Unix.
- **CRLF (Fix B):** not reproducible under Wine (host files are LF); confirmed
  only by `windows-latest` CI after push.

### Note [[qmd65_result_note: text]]

Independent of QMD-63 (resolution direction/depth) — this is pure path/EOL
rendering. Local Wine toolchain: `brew install --cask wine-stable` (Intel build
with Rosetta 2), `x86_64-w64-mingw32` from `mingw-w64`, `rustup target add
x86_64-pc-windows-gnu`, and copy `libwinpthread-1.dll` next to the test exe.

### Deliberately out of scope [[qmd65_result_scope: text]]

Scope is **Rust-only** — the only parser gated on Windows CI (the `binary` job
runs on `windows-latest`; the `python`/`typescript` jobs are `ubuntu-latest`
only). `qmdc-py` and `qmdc-ts` still emit `\` in `__file`/`files`/`error.file`
on Windows — a real but **latent** bug that no test currently exercises (there
is no `windows-latest` leg for those suites), so it does not gate this release.
Fixing the py/ts parsers **and** adding a `windows-latest` cross-parser CI leg
(so the divergence becomes a genuine red-green, not an untested preventive edit)
is tracked as a separate follow-up. The residual live-editor-buffer CRLF in
hover (a Windows user editing a CRLF buffer) is likewise deferred there;
`.gitattributes` covers the disk-checkout/CI case that this task needed.
