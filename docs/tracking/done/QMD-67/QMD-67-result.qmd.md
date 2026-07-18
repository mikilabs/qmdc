# QMD-67: Result

## Namespace-scoped duplicate detection [[qmd67_result: Result]]

Duplicate detection now groups by `(namespace, full __id)` in all three validators,
so the same top-level id in different namespaces is no longer a false `duplicate_id`.
This aligns the validator with the DB identity `(__workspace, __namespace, __id)` and
with namespace-aware reference resolution. The shared bare-id index used by reference
resolution / `__local_id` fallback / cross-namespace hints was left untouched; the new
grouping is a separate map. Same-file and genuine same-namespace duplicate detection,
plus system/parser-error exclusions, are preserved.

- feature: [[#qmd67_ns_dup]]

### Files Changed [[files_changed: array]]

- qmdc-rs/src/workspace.rs
- qmdc-py/qmdc/workspace.py
- qmdc-ts/src/workspace.ts
- tests/workspace/duplicate-id-nested-namespace-scoping/readme.qmd.md
- tests/workspace/duplicate-id-nested-namespace-scoping/outer/readme.qmd.md
- tests/workspace/duplicate-id-nested-namespace-scoping/outer/inner_a/readme.qmd.md
- tests/workspace/duplicate-id-nested-namespace-scoping/outer/inner_b/readme.qmd.md
- tests/workspace/duplicate-id-nested-namespace-scoping/_expected.json
- Makefile
- docs/lsp/diagnostics.qmd.md

### Summary [[summary: text]]

- Rust: added a separate `ObjectsByNsId` map keyed by `(namespace, full __id)` for
  duplicate detection; introduced `DupLocation`/`ObjectsByNsId` type aliases to satisfy
  clippy `type_complexity`.
- Python: added `objects_by_ns_id` grouping keyed by `(namespace, obj_id)`.
- TypeScript: added a nested `Map<namespace, Map<id, locations>>` (collision-safe
  composite key) for duplicate grouping.
- Tests: existing `duplicate-id-namespace-scoping` (flat) turns green; added
  `duplicate-id-nested-namespace-scoping` for nested namespaces; genuine duplicates
  remain covered by `validation-parser-consistency`.
- Docs: corrected the `duplicate_id` LSP diagnostic wording to the full hierarchical
  `__id` (Kind removed from duplicate identity).
- Build: fixed a `make -j` uv race in the Makefile — `py-build` now
  `uv sync --extra dev` (single authoritative writer) and `py-format` depends on it, so
  parallel `uv run` calls are pure reads. `make test-fast` passed 5/5 consecutive runs.
