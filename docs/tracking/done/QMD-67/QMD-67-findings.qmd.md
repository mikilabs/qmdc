# QMD-67: Findings

## Namespace-scoped duplicate detection is inconsistent across validators [[qmd67_finding_namespace_scope: Finding]]

Workspace object identity is already namespace-scoped in persistence and reference
resolution, but duplicate validation groups candidates only by bare `__id`. The same
false positive exists in Rust, Python, and TypeScript.

- category: parser
- related_to: [[#qmd67_ns_dup]]

### Operator Decisions [[operator_decisions: text]]

- TypeScript is explicitly in scope and must be fixed together with Rust and Python.
- Do not add Kind-specific behavior or regression variants. Kind semantics are planned
  for removal separately and must not expand QMD-67.

### Affected Files [[affected_files: array]]

- qmdc-rs/src/workspace.rs
- qmdc-py/qmdc/workspace.py
- qmdc-ts/src/workspace.ts
- tests/workspace/duplicate-id-namespace-scoping/
- tests/workspace/duplicate-id-nested-namespace-scoping/
- tests/workspace/validation-parser-consistency/_expected.json

### Affected Functions [[affected_functions: array]]

- qmdc_rs::workspace::parse_workspace
- qmdc.workspace.validate_workspace
- validateWorkspace

### Solution [[solution: text]]

Build a separate duplicate-validation grouping keyed by `(namespace, full __id)` in
all three validators. Do not add Kind to the uniqueness key or introduce new
Kind-specific behavior. Do not replace the existing bare-ID candidate index used by
reference resolution: local-first and explicit cross-namespace reference semantics
depend on seeing all namespace candidates. Preserve exclusions for parser errors and
auto-generated system objects, and preserve parser ownership of same-file duplicate
diagnostics.

### Why This Bug Survived [[qmd67_why_survived: text]]

- about: [[#qmd67_ns_dup]]

Duplicate detection was the only place that keyed identity on the bare `__id`. The
SQLite schema already uses `PRIMARY KEY (__workspace, __namespace, __id)` with a
generated unique `__global_id`, and reference resolution already filters candidates by
namespace — so `r1:foo` and `r2:foo` were always distinct rows and distinct reference
targets, yet the validator alone called them duplicates. The gap went unnoticed
because no fixture and none of the 15 namespaces in the dogfooded `docs/` workspace
ever place the same bare top-level id in two namespaces: everything is disambiguated
by hierarchy (dot-ids) or unique names. Namespace-aware machinery (`__local_id`,
`by_local_id`) was layered in later while the duplicate check kept the namespace field
in its tuple unused.

### Implementation Guardrails [[qmd67_guardrails: text]]

- about: [[#qmd67_ns_dup]]

1. Add a SEPARATE map for duplicate detection keyed by `(namespace, full __id)`. Do
   NOT re-key the shared `objects_by_id` — reference resolution, `__local_id`
   fallback, cross-namespace "Did you mean" hints, and `ambiguous_reference` all read
   the bare-id index and filter by namespace in-loop.
2. Key on the full hierarchical `__id`, never the bare leaf, or `parent_a.config` and
   `parent_b.config` would collapse.
3. Root/no-namespace objects use empty-string namespace; two root files with the same
   id must still collide (`validation-parser-consistency`).
4. `__workspace` is intentionally absent from the key; this is safe only because
   validation runs per-workspace. Do not pool objects across workspaces into one
   duplicate pass.
5. Preserve current exclusions: `__ParsingError` (Rust), `__Document`/`__TextBlock`,
   and the parser-owned same-file branch. Leave the same-file/different-kind branch
   byte-for-byte — no Kind changes here.
6. TypeScript must use a collision-safe composite key (nested map or encoded tuple),
   not a naive delimiter string.

### Test Plan [[test_plan: text]]

**Existing tests:** the shared, currently untracked
`duplicate-id-namespace-scoping` fixture already reproduces the flat sibling case and
is auto-discovered by Rust, Python, and TypeScript conformance runners.
`validation-parser-consistency` protects genuine root-namespace duplicates;
`hierarchical-ids-no-dup` protects repeated leaf IDs under distinct parents; existing
local-ID/cross-namespace fixtures protect reference resolution.

**New coverage needed:** do not modify existing fixtures. Keep
`duplicate-id-namespace-scoping` unchanged as the flat sibling regression. No existing
fixture directly covers repeated top-level IDs in nested namespaces, so add one
minimal shared fixture at `duplicate-id-nested-namespace-scoping`. Do not add a new
same-namespace collision fixture: `validation-parser-consistency` already covers real
duplicates. Do not add Kind variants or implementation-specific test code.

**Verification:** parse this Finding, run `make test-fast`, and during implementation
run the shared Rust/Python/TypeScript workspace conformance suites plus targeted
namespace/reference and duplicate fixtures. Confirm valid cross-namespace reuse has
no `duplicate_id`, while same-namespace duplicates retain existing diagnostics.

**Triage reproduction result:** `make test-fast` fails only because the new
`duplicate-id-namespace-scoping` regression expects no errors before the fix. Python
reports one failing fixture assertion; Rust reports the same fixture through its
workspace unit and conformance wrappers. Every observed failure contains the expected
`duplicate_id` for `foo` in `ns_a/readme.qmd.md` and `ns_b/readme.qmd.md`; unrelated
tests pass. This is the documented Bug-triage exception, not an implementation result.
