# QMD-66: Findings

## Guide sync mechanism [[qmd66_finding_guide_sync: Finding]]

The canonical guide source is `docs/guides/qmdc-guide.qmd.md`; `qmdc-rs/src/qmdc-guide.qmd.md` is a vendored copy embedded into the crate at COMPILE TIME (`include_str!` at `core/guide.rs:16`; the constant is served by MCP tool `qmdc_get_guide`, `mcp/tools.rs:353`, AND as MCP resource `qmdc://guide`, `mcp/resources.rs:81`). The Makefile provides `make guide-sync` (Makefile:601-608) to copy canonical → vendored. Because of `include_str!`, `make guide-sync` alone does NOT update what a running MCP server returns — a rebuild is required. An automated parity test already exists: `qmdc-rs/tests/mcp.rs:494/:547` asserts the served text equals the canonical source, so `cargo test` fails on drift.

- category: docs
- related_to: [[#qmd66_dot_notation_discrepancies]]

### Solution [[solution: text]]

Edit `docs/guides/qmdc-guide.qmd.md` only; run `make guide-sync`; rebuild qmdc-rs; never hand-edit the vendored copy.

### Test Plan [[test_plan: text]]

Existing: `qmdc workspace validate ./docs` must stay `[]` after edits; `./qmdc parse -i docs/guides/qmdc-guide.qmd.md` must pass; parity test `tests/mcp.rs:494/:547` gates canonical/vendored drift automatically. New tests: none needed — doc-only. Verify: `make test-fast` (includes the parity test).

## Goals A1-A3: fiction removal scope [[qmd66_finding_fiction_scope: Finding]]

Exact locations of the unimplemented syntax. Guide (canonical copy): line ~412 "For an array of results use `*`: `[[#*object.array[field=value]]]`" in the Field References section, and line ~1067 `- columns: [[[#users.columns[name=email]]]]` in the Database Schema example. Spec: `docs/format/references.qmd.md` lines ~211-212 (0-results / >1-results filter semantics) and line ~244 (`- references: [[#users.columns[name=id]]]` example). No `[key=value]` or `*` handling exists in any parser (`core/resolve.rs`, `workspace.rs`, `parser_modules/references.rs`, `db/mod.rs`; same absence in py/ts). Zero test coverage in all implementations.

- category: docs
- related_to: [[#qmd66_dot_notation_discrepancies]]

### Affected files [[affected_files: text]]

- `docs/guides/qmdc-guide.qmd.md` — lines ~412 (wildcard sentence) and ~1067 (filter example)
- `docs/format/references.qmd.md` — lines ~211-212 (filter result semantics) and ~244 (`[name=id]` example)

### Solution [[solution: text]]

Delete the wildcard sentence; replace the filter examples with real table-row addressing via auto-generated IDs (`parent.field.field_<n>`, e.g. `[[#users.columns.columns_0]]` — behavior per `parser.rs:148-155`, asserted in `tests/parser/032-table.expected.json:7-8`). In `references.qmd.md` drop the filter-semantics bullets and the `[name=id]` example; state explicitly that filter/wildcard forms are not part of the format.

### Test Plan [[test_plan: text]]

Existing: `tests/parser/032-table.expected.json` already asserts the auto-ID scheme the new examples will show. New tests: none — removing fiction, not changing behavior. Verify: grep the docs tree for `[name=` and `[[#*` → only historical mentions in tracking/ remain.

## Goal B4: port ambiguous_field_reference to shared validator [[qmd66_finding_b4_shared_validator: Finding]]

All three implementations already emit `ambiguous_field_reference` in the CLI workspace-validate path (`qmdc-rs/src/workspace.rs:991-1046`, `qmdc-py/qmdc/workspace.py:1078-1121`, `qmdc-ts/src/workspace.ts:1022-1057`). The gap is rs-only: the shared validator `core/ops/validate.rs` (`collect_reference_issues`, used by LSP diagnostics and MCP `qmdc_validate_references`) resolves via `ObjectIndex::resolve` (`core/resolve.rs:227-242`), where the hierarchical-id branch returns `Resolved` before the field interpretation is ever considered.

Diagnostic code: QMDC003 and QMDC004 are TAKEN at the LSP layer (duplicate-id `lsp/server.rs:1241`, workspace-in-wrong-file `lsp/server.rs:1281`; fixtures `tests/lsp/microtests/diagnostics/002-duplicate-id/expected.json:10` pin them), and `docs/lsp/diagnostics.qmd.md:25-37` reserves QMDC003-QMDC008. The new diagnostic gets **QMDC009** (first genuinely free per the docs registry).

The `Resolution` match in `collect_reference_issues` (`core/ops/validate.rs:144-155`) is exhaustive (no `_ =>` wildcard), so adding the variant is compile-checked — every consumer is forced to handle it.

- category: parser
- related_to: [[#qmd66_dot_notation_discrepancies]]

### Affected files [[affected_files: text]]

- `qmdc-rs/src/core/resolve.rs` — `ObjectIndex::resolve`: detect the dotted-ref case where BOTH the whole string matches an `__id` AND the last-dot split resolves as a field; return a new `Resolution::AmbiguousFieldRef` variant (carry candidates)
- `qmdc-rs/src/core/ops/validate.rs` — map the new variant to a QMDC009 diagnostic
- `qmdc-rs/src/lsp/server.rs` — `compute_diagnostics` consumes the same resolution; ensure the new variant surfaces (fixture `tests/lsp/microtests/diagnostics/` update)
- `qmdc-rs/src/mcp/tools.rs:212` — tool description text documents "QMDC001/QMDC002"; extend with QMDC009
- `docs/lsp/diagnostics.qmd.md` — register QMDC009 in the code table

### Solution [[solution: text]]

Mirror the CLI logic including the exemption: NOT ambiguous when the field value is exactly the parser-generated `[[#<full dotted id>]]` parent→child link (`workspace.rs:1009-1012`). Note the exemption only concerns the case where the WHOLE dotted ref is a real object `__id` (resolved_objects.len()==1 branch) — plain field refs like `[[#auth_svc.config.timeout]]` (not an object id) never enter this branch and cannot false-positive. Message and candidates format copied from the CLI error for consistency. py/ts need no change (no LSP/MCP there); the CLI paths in all three implementations stay as-is.

### Test Plan [[test_plan: text]]

Existing: `tests/workspace/ambiguous-field-reference/` covers the CLI path in all three implementations — unchanged; `tests/workspace/hierarchical-ids/` must stay green (guards against exemption false-positives). New: one LSP microtest `tests/lsp/microtests/diagnostics/` (data-driven input.qmd.md + expected.json) reproducing the same collision (object `[[team.score]]` + field `score` on `team`) and expecting a QMDC009 diagnostic; the parser-generated-link exemption case expecting NO diagnostic; plus `mcp-request.json`/`mcp-expected.json` in the same fixture dir so MCP `qmdc_validate_references` is covered data-driven (no manual step). Verify: `make test-fast`.

## Goal B5: field-ref branch missing in resolve_object [[qmd66_finding_b5_resolve_object: Finding]]

`ObjectIndex::resolve_object` (`core/resolve.rs:146-190`) lacks the `split_field_ref` branch that `resolve` (validation) has (`core/resolve.rs:238-242`). Consumers: `core/ops/locate.rs`, `core/ops/references.rs`, `core/ops/describe.rs` (has its own `split_dot_path` workaround at `describe.rs:42-47/133-147`), `core/ops/rename_plan.rs`, `lsp/handlers/references.rs`. Consequence: `[[#team.config.env]]` validates OK but find_references misses the referrer and rename does not rewrite it (`rename_plan.rs:110-119` — `resolve_object` returns `None` → `applies=false`) → silent reference breakage on rename.

- category: parser
- related_to: [[#qmd66_dot_notation_discrepancies]]

### Affected files [[affected_files: text]]

- `qmdc-rs/src/core/resolve.rs` — add field-ref branch to `resolve_object`: after the hierarchical-full-id check (matching `resolve()`'s precedence, hierarchical at :233 → field-ref at :238), try `split_field_ref`; if the prefix resolves by exact `__id` and has the field, return the PREFIX object
- `qmdc-rs/src/core/ops/rename_plan.rs` — verify rewrite: renaming `team` must rewrite the prefix inside `[[#team.config.env]]`; the bounded-token rewriter (`rewrite_reference_raw`, lines 290-310) already allows `.` as a following boundary, so prefix rewrite should work once resolution succeeds
- `qmdc-rs/src/core/ops/describe.rs` — ⚠️ REORDER REQUIRED, do NOT drop `split_dot_path`: describe currently calls `resolve_object` first (`describe.rs:38`); once that succeeds for field paths it would return the full object card of the prefix instead of the field descriptor `{object_id, field, value, type}` — a behavior regression of `qmdc describe`/`qmdc_describe_object`/MCP resource `qmdc://object/{id}` (`mcp/resources.rs:103`). Try the field-descriptor path (`split_dot_path` → `describe_field`, `describe.rs:42-47`) BEFORE falling back to whole-object resolution

### Solution [[solution: text]]

Return the prefix object from `resolve_object` for field refs — matches user intent (the ref points AT that object's field) and makes find_references/rename treat the referrer as pointing to the object. Caller audit (exhaustive): `locate.rs:31` — reports owning object's location for a field path, operator approved; `references.rs:44/:67` and `lsp/handlers/references.rs:71/:127` — identity comparison, the intended fix; `rename_plan.rs:111` — the intended fix; `describe.rs:38` — MUST be reordered first (see affected files), otherwise field-path describe regresses.

### Test Plan [[test_plan: text]]

Existing: `tests/lsp/microtests/rename/004-cascading` covers object dot-path cascade (does NOT cover field refs); `tests/workspace/hierarchical-ids/services.qmd.md:26` has a valid field ref; existing describe tests gate the describe reorder. New (data-driven, rs LSP microtests): (1) `rename/` fixture — workspace with `[[#team.config.timeout]]` field ref, rename `team` → expect the ref rewritten to `[[#squad.config.timeout]]`; (2) `references/` fixture (or find_references microtest) — field ref counted among references to `team.config`'s owner; (3) describe fixture (or existing test extension) asserting field-path describe still returns the field descriptor after the reorder. Verify: `make test-fast`; existing rename tests must stay green (bounded-token safety test `rename_plan.rs:356-367`).

## Goal F14: canonical error list from code [[qmd66_finding_f14_error_list: Finding]]

Error types actually emitted by qmdc-rs (grep over emitted string literals): broken_link, duplicate_id, ambiguous_reference, broken_parent, ambiguous_field_reference, nested_workspace, structured_in_textblock, multiple_definitions, ordered_list_in_array, nested_subitems, explicit_system_type, mixed_field_keys, dangling_field, invalid_map_entry, invalid_map_content, workspace_in_wrong_file, invalid_id_character (17 total). Discrepancies: the guide's table lists `type_mismatch` which is emitted NOWHERE in any implementation (one more piece of fiction, same as A1/A2); the guide omits dangling_field, invalid_map_entry, invalid_map_content, workspace_in_wrong_file, invalid_id_character; `docs/format/validation-errors.qmd.md` does not define nested_subitems/mixed_field_keys; `docs/parsers/commands.qmd.md` omits broken_parent/ambiguous_field_reference.

- category: docs
- related_to: [[#qmd66_dot_notation_discrepancies]]

### Affected files [[affected_files: text]]

- `docs/guides/qmdc-guide.qmd.md` — error table (~lines 697-712)
- `docs/format/validation-errors.qmd.md` — remove type_mismatch (~106-110), add 3 missing definitions
- `docs/parsers/commands.qmd.md` — error list (~212-217)

### Solution [[solution: text]]

The 17-type code-derived list is canonical. Update all three documents to it: guide error table (drop type_mismatch, add the 5 missing), `validation-errors.qmd.md` (REMOVE the `type_mismatch` definition at lines ~106-110 — it defines fiction; ADD definitions for nested_subitems, mixed_field_keys AND invalid_id_character, all three absent), `commands.qmd.md` (add broken_parent, ambiguous_field_reference). Note in validation-errors.qmd.md which errors come from parse vs workspace-validate stage. Marginal: `workspace.rs:1140` has a defensive `unwrap_or("parsing_error")` fallback type — leave undocumented (unreachable in practice, not a real 18th type).

### Test Plan [[test_plan: text]]

Existing: none applicable (docs). New: none. Verify: for each of the 17 types, confirm one emitting site in code; for each documented type, confirm it is in the 17; parse + workspace validate the edited docs.

## Goals C6/D7/E8-E13: guide content plan [[qmd66_finding_guide_content: Finding]]

Where each addition lands in the canonical guide. C6 (real resolution semantics) → rewrite the "Field References" section: exact whole-string `__id` match, no per-segment walking, unlimited depth, field ref = last-dot split with single trailing segment, `__` fields excluded, `[[#ns:obj.field]]` unsupported. D7 → rewrite "Reference Philosophy": severity is `error`, but reference errors are non-fatal (graph still loads; unresolved ref stays a string) unlike syntax errors; SAME rewording in `docs/format/references.qmd.md` lines ~205 ("all reference issues are warnings"), ~214 ("Validation produces warnings"), ~216 (strict-mode sentence — drop or reword, warnings no longer exist as a category). E8 (dot-ID declarations) → new subsection in Syntax after "Nested Objects", cross-linked from the broken_parent row. E9 (ID formation + `__Workspace`/`__Namespace` flat exception) → extend "Nested Objects"/"Object Arrays". E10 (`__local_id` + fallback order) → extend "System Types"/reference overview, per `docs/architecture/algorithms.qmd.md:229-255`. E11 → error-table row for invalid_id_character. E12 (rename cascade) → subsection under Reference (decided — it is reference semantics, not CLI usage): refs to descendants rewritten, bounded-token safety. E13 — constraint: no LSP content (completion triggers, go-to-def) anywhere in the guide.

- category: docs
- related_to: [[#qmd66_dot_notation_discrepancies]]

### Affected files [[affected_files: text]]

- `docs/guides/qmdc-guide.qmd.md` — C6, D7, E8-E12 (then `make guide-sync` + rebuild)
- `docs/format/references.qmd.md` — D7 rewording at lines ~205/214/216 (in addition to the A1 filter-semantics removal tracked in [[#qmd66_finding_fiction_scope]])

### Triage review record [[triage_review_record: text]]

Triage was reviewed on 2026-07-07 by three independent review passes (fact-check vs code, goal-coverage vs SOP, adversarial design review of B4/B5). Corrections applied: QMDC003→QMDC009 (003/004 taken at LSP layer, docs reserve up to 008); describe reorder requirement added to B5 (prefix-object return would regress field-path describe — split_dot_path must run first, not be dropped); D7 scope extended to references.qmd.md warnings-wording (lines ~205/214/216); F14 extended (type_mismatch also defined in validation-errors.qmd.md — remove; invalid_id_character also missing there); guide-sync finding corrected (include_str! → rebuild required; parity test exists at tests/mcp.rs:494/:547); E12 placement decided (Reference section); B4 MCP verification upgraded to data-driven mcp-request/mcp-expected fixtures; Resolution match in shared validator confirmed exhaustive (no wildcard arms — new variant is compile-checked). Exemption false-positive risk checked: plain field refs never enter the len==1 branch, tests/workspace/hierarchical-ids/ guards it.

### Resolved questions [[resolved_questions: text]]

Operator decisions (2026-07-07):

1. B5 return semantics — APPROVED: `resolve_object` returns the PREFIX object for field-path targets; `qmdc locate` reporting the object's location for a field-path is acceptable (matches describe's current behavior).
2. `type_mismatch` — APPROVED: remove from the guide as fiction (part of goal F14); do not implement.
3. Kind-dot form `[[#Kind.id]]` — DROPPED: do not document, no follow-up task, leave code behavior as is.

### Test Plan [[test_plan: text]]

Existing: guide examples live in fenced blocks (`example` modifier) so `qmdc workspace validate ./docs` stays the gate. New: none for doc goals. Verify: parse guide, validate ./docs, `make guide-sync`, diff copies empty.

## Goals G15-G18: id scoping and nameless arrays [[qmd66_finding_id_scoping: Finding]]

Integrated from a separately-drafted task (AI-DLC metamodel experience, 2026-07-08) after fact-checking against code and tests. Verified claims: (1) same local id under different parents is legal and produces distinct dot-path ids — `tests/parser/202-dot-id-no-collision.qmd.md` (same file) and `tests/workspace/hierarchical-ids-no-dup/_expected.json` (`errors: []`, objects `auth_svc.config` + `payment_svc.config`); (2) duplicate detection indexes the FULL hierarchical `__id` (`workspace.rs:580-707`), so the guide's `duplicate_id` row ("same Kind:Id in one namespace") is misleading for nested children. Corrections applied to the original proposal during integration: its suggested wording "same id at the same level in the same namespace" is also imprecise — the real rule is "same full hierarchical `__id`" (cross-file always errors; same-file only with different kinds); and its C4 referenced a "generic field IDs" section that does not exist in the canonical guide (external-steering-copy-only) — G18 adds the principle as new content, external copy out of scope (re-syncs from canonical afterwards).

- category: docs
- related_to: [[#qmd66_dot_notation_discrepancies]]

### Affected files [[affected_files: text]]

- `docs/guides/qmdc-guide.qmd.md` — "Nested Objects" (G15 example + duplicate_id cross-ref), error table `duplicate_id` row (G16, coordinate with F14), "Object Arrays" (G17), "What Good QMD.md Looks Like" principles (G18)

### Solution [[solution: text]]

G15: add scoped-reuse paragraph + before/after example (verbose `run_desc`/`task_desc` → repeated `description` under each parent) near "Nested Objects". G16: reword the `duplicate_id` row to "two objects with the same full hierarchical `__id`" and note parent-scoped children never collide; one cross-ref line in Nested Objects. G17: add `[Kind]`-array recommendation with table example to "Object Arrays", plus the trade-off note: auto row ids (`field_<n>`) are positional — if items WILL be referenced individually, give them names; those names scope like G15 (last dot-path segment, e.g. `svc_a.rules.timeout_rule`), so simple local names (`timeout_rule`), never parent-prefixed (`svc_a_timeout_rule`). Verified empirically: two `[[timeout_rule]]` under different parents' `[Kind]` arrays with refs to both dot-paths → `workspace validate` returns `[]`. G18: one principle line in the `good_qmdc` principles: identity comes from Kind + position in the hierarchy, not from a bespoke globally-unique name. All edits canonical-guide-only, flowing through the same guide-sync/rebuild as the other doc goals ([[#qmd66_finding_guide_sync]]).

### Test Plan [[test_plan: text]]

Existing: `tests/parser/202-dot-id-no-collision.qmd.md` and `tests/workspace/hierarchical-ids-no-dup/` already pin the behavior being documented — no behavior change. New: none (docs only). Verify: parse guide, `qmdc workspace validate ./docs` stays `[]`, `make guide-sync` + rebuild, parity test green. Acceptance (from the original proposal): following the guide from scratch on a small metamodel yields `description`/`constraints`/array-rules, not `*_desc`/`*_constraints`/hand-named rules.
