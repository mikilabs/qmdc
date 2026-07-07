# QMD-66: Fix dot-notation discrepancies between guide, code, tests, and docs

## Fix dot-notation discrepancies [[qmd66_dot_notation_discrepancies: Bug]]

A full audit (code + tests + docs, 2026-07-07) of dot-notation support found that the agent guide (`qmdc-rs/src/qmdc-guide.qmd.md`, mirrored at `docs/guides/qmdc-guide.qmd.md`) documents syntax that does not exist in any implementation, omits implemented features, and that two resolver code paths silently diverge. The guide is what agents read to produce syntax, so unbacked claims directly cause broken links in user workspaces.

Operator decisions are recorded per goal — they are requirements, not open questions.

- status: planned
- priority: high
- category: format
- affects: [[#guides:qmdc_guide]]

### Goals [[goals: [Goal]]]

#### A1: Remove array-filter reference syntax from docs [[qmd66_goal_a1]]

Remove `[[#users.columns[name=email]]]` from the guide (including the Database Schema example) and from `docs/format/references.qmd.md` (filter result semantics, lines ~211-212, and the `[name=id]` example). No `[key=value]` parsing exists in any implementation; such refs resolve to nothing and produce broken_link. Operator: plans that leaked into docs, not implemented, possibly never will be — delete.

- group: A_remove_fiction
- done: false

#### A2: Remove wildcard syntax from the guide [[qmd66_goal_a2]]

Remove `[[#*object.array[field=value]]]`. It exists only in the guide — no spec, no code, no tests.

- group: A_remove_fiction
- done: false

#### A3: Show the real table-row addressing mechanism [[qmd66_goal_a3]]

Replace the removed examples with auto-generated hierarchical IDs `parent.field.field_<n>` (e.g. `[[#users.columns.columns_0]]`) — the only real way to reference a table row.

- group: A_remove_fiction
- done: false

#### B4: Emit ambiguous_field_reference from the shared validator [[qmd66_goal_b4]]

Today it is only emitted by CLI `qmdc workspace validate` (`workspace.rs:991-1046`). The shared validator `core/ops/validate.rs` (used by LSP diagnostics and MCP `qmdc_validate_references`) never emits it — `core/resolve.rs:232-242` resolves the object silently. Add the check to the shared path so all consumers agree. Operator: "нужно добавить в код".

- group: B_code_fixes
- done: false

#### B5: Fix field-path refs in rename and find_references [[qmd66_goal_b5]]

`resolve_object` (`core/resolve.rs:146-190`) lacks the field-ref branch that `resolve` (`core/resolve.rs:227-242`) has. Consequence: a field-path ref like `[[#team.config.env]]` validates OK but is invisible to find_references and is NOT rewritten by rename → silent reference breakage on rename. Add field-ref support to `resolve_object` (or the rename/find_references paths). Operator: "бага, надо добавить в код".

- group: B_code_fixes
- done: false

#### C6: Document actual dot-path resolution semantics [[qmd66_goal_c6]]

The guide is the syntax source for agents. State the real rule: a dotted ref resolves by exact whole-string `__id` match; intermediate segments are not walked or validated; depth unlimited. Field ref = split on the LAST dot only, exactly one trailing field segment, prefix must be a full object `__id`, system `__` fields excluded, namespace-qualified field refs (`[[#ns:obj.field]]`) not supported.

- group: C_semantics
- done: false

#### D7: Fix the severity vs fatality conflation [[qmd66_goal_d7]]

Reference errors are errors by severity, but non-fatal: unlike syntax/parse errors they do not prevent the graph from being built — the object still loads and the unresolved reference stays a plain string. The guide's "Reference Philosophy" section conflates the two dimensions by calling them "warnings". Rewrite it to state both explicitly: `severity: error` (matching `validation-errors.qmd.md` and the guide's own JSON example), AND non-blocking for graph loading (which is the true point the section was making). Align `docs/format/references.qmd.md` wording the same way.

- group: D_severity
- done: false

#### E8: Document dot-ID declarations [[qmd66_goal_e8]]

`## Alice [[team.members.alice]]` top-level heading syntax, parent auto-linking (`__parent` injection), parent lookup by prefix-up-to-last-dot, cross-file and cross-namespace resolution, and how this produces `broken_parent`.

- group: E_add_to_guide
- done: false

#### E9: Document hierarchical ID formation rules [[qmd66_goal_e9]]

Single child → `parent.child`; array element → `parent.field.child`; children of `__Workspace`/`__Namespace` keep flat IDs.

- group: E_add_to_guide
- done: false

#### E10: Document __local_id fallback resolution [[qmd66_goal_e10]]

Resolution order: exact `__id` → `__local_id` fallback → ambiguous_reference on collision. The guide never mentions `__local_id` at all.

- group: E_add_to_guide
- done: false

#### E11: Add invalid_id_character to the guide error table [[qmd66_goal_e11]]

A dot in a NESTED heading ID is a parse error; dot-IDs are legal only on top-level headings.

- group: E_add_to_guide
- done: false

#### E12: Document rename cascade semantics [[qmd66_goal_e12]]

Renaming `team` also rewrites `[[#team.config]]`, `[[#team.members.alice]]`; bounded-token rewrite keeps `[[#teamwork]]` safe.

- group: E_add_to_guide
- done: false

#### E13: Keep LSP behavior out of the guide [[qmd66_goal_e13]]

Do NOT add LSP-specific behavior (dot-trigger completion etc.) to the guide — the guide describes the format only. Operator: "LSP вообще не должно тут быть". Verify no LSP content slips in while adding E8-E12.

- group: E_add_to_guide
- done: false

#### F14: Reconcile the three diverging error lists [[qmd66_goal_f14]]

Guide error table vs `docs/format/validation-errors.qmd.md` vs `docs/parsers/commands.qmd.md`. Guide lists `nested_subitems`/`mixed_field_keys` that validation-errors.qmd.md does not define; validation-errors.qmd.md defines `dangling_field`/`invalid_map_entry`/`invalid_map_content`/`workspace_in_wrong_file` that the guide omits; commands.qmd.md omits `broken_parent`/`ambiguous_field_reference` entirely. One canonical list, all three documents consistent with code.

- group: F_error_tables
- done: false

### Notes [[qmd66_notes: text]]

- Both guide copies (`qmdc-rs/src/qmdc-guide.qmd.md` and `docs/guides/qmdc-guide.qmd.md`) are currently byte-identical — keep them in sync when editing.
- Kind-dot form `[[#Kind.id]]` resolves by bare last segment with Kind ignored (`core/resolve.rs:28-35`), unlike `[[#Kind:id]]`. Decide during triage whether to fix or explicitly not document.
- `ambiguous_field_reference` exemption: not raised when the conflicting field value is exactly the parser-generated `[[#parent.child]]` link (`workspace.rs:1009-1012`) — keep this behavior when porting the check to the shared validator.
- Audit evidence: code `core/resolve.rs`, `workspace.rs:517-577/991-1046`, `parser.rs:63-104/1586-1628`, `core/ops/rename_plan.rs:77-135`; tests `tests/parser/186-208`, `tests/workspace/dot-id-*`, `tests/workspace/hierarchical-ids*`, `tests/workspace/ambiguous-field-reference/`; zero test coverage exists for filters/wildcard in any implementation.

## Checklist

- [ ] Understood the task
- [ ] Studied the code
- [ ] Created a plan and prototypes in `artifacts/`
- [ ] Tested the solution
- [ ] Moved the code into the project
- [ ] Created Result.md and Findings.md
