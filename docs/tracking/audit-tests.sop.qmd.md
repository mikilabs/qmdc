# Microtest Audit [[microtest_audit: SOP]]

Standard Operating Procedure for checking microtests against the format specification.

- version: 1.0
- commands: [audit-tests]

## Overview

Periodic check of microtests in `tasks/QMD-4/artifacts/microtests/` against the spec in `docs/format/`. Run after adding new tests or changing the spec.

**When to use:**

- "Audit tests" — check all new/changed tests
- "Audit tests 170-180" — check a specific range
- After adding new microtests
- After changing the spec in `docs/format/`

## Prerequisites [[audit_prereqs: Section]]

Read before first run:

1. `docs/format/` — all 12 spec files
2. Previous audit results — queried in step 2

## Parameters [[audit_params: Parameters]]

- range: test range (optional, defaults to all new/changed)
- full: true for full audit of all 169+ tests (default false — new only)

## Steps

### 1. Determine scope [[step_scope: Step]]

Determine which tests to check.

- order: 1

**Actions:**

If `full: true` — all tests. If `range` specified — tests in range. Otherwise — find new/changed:

```bash
# Tests not covered in batch reports
ls tasks/QMD-4/artifacts/microtests/*.qmd.md | wc -l
# Compare with last known count (169 at time of QMD-45)
```

### 2. Read spec [[step_read_spec: Step]]

Read the spec and previous audit decisions.

- order: 2

**Constraints:**

- You MUST read ALL files in `docs/format/` if this is your first audit
- You MUST query previous audit decisions:

```bash
./qmdc query ./docs "SELECT __id, __label, json_extract(data, '$.solution') as solution FROM objects WHERE __kind = 'TestAuditResult'"
```

### 3. Audit tests [[step_audit: Step]]

For each test in scope: read `.qmd.md` + `.expected.json`, check against spec.

- order: 3

**Constraints:**

- You MUST read both `.qmd.md` and `.expected.json` for each test
- You MUST NOT modify any files
- You MUST check against established decisions from step 2

**For each test, verify:**

1. `__id` — correct extraction/auto-generation
2. `__label` — present only when Title exists, correct value
3. `__kind` — `__Object` when no Kind, correct Kind when specified, system types correct
4. `__types`, `__syntax` — match actual content
5. `__comments` — correct anchors (`after` field), content is raw markdown slice
6. `__parent`, `__parent_field`, `__container` — correct relationships
7. References `[[#id]]` — correctly identified
8. Arrays — correct syntax annotation (`yaml_array`, `markdown_list`, `comma_refs`, etc.)
9. `__TextBlock` vs object — heading without `[[id]]` and without fields → `__TextBlock`; with fields → auto-ID object
10. Parsing errors — correct error type and line number
11. etc

**Output per test:**

- ✅ NNN-name — OK
- ❌ NNN-name — DESCRIPTION (which spec rule is violated)
- ⚠️ NNN-name — AMBIGUITY (spec doesn't cover this case)

### 4. Write report [[audit_step_report: Step]]

Write results.

- order: 4

**Actions:**

1. Write report to where the user asked (append new batch or update existing)
2. If new issues found — update `docs/tracking/planned/QMD-45/artifacts/summary.md`
3. If new spec gaps found — create `TestAuditResult` objects in the relevant task findings file
4. Report in chat: number of tests checked, issues found, new findings if any
