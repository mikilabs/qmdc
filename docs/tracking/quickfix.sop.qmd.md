# Quick Fix [[quickfix: SOP]]

Simplified process for small bugs and simple features that don't need the full workflow.

- version: 1.1
- commands: [quickfix, qf]

## Overview

For bugs and small features that: affect a well-understood area, have an obvious implementation, and can be completed in one session. Everything else goes through the full workflow (`task_workflow`).

**When to use:**

- "Quickfix: Rust parser doesn't parse X" — small bug
- "qf: empty __label in test 052" — specific fix
- "qf: add map field type" — small, self-contained feature with clear spec

**When NOT to use (→ full workflow):**

- Large features that touch many subsystems
- Unclear requirements or design decisions needed
- Affects all 3 parsers in non-trivial ways
- Unclear whether it's a bug or by design

## Process

### 1. Reproduce [[qf_step_reproduce: Step]]

- order: 1

For bugs: find or write a minimal `.qmd.md` input, show expected vs actual result.
For features: write a minimal `.qmd.md` showing the desired syntax and expected JSON output.

```bash
# Check parser on a specific file
./qmdc parse -i path/to/test.qmd.md
```

If the bug doesn't reproduce — close. If unclear whether it's a bug or by design — escalate to full workflow. If the feature scope grows beyond "small" — escalate.

### 2. Write test [[qf_step_test: Step]]

- order: 2

Before fixing — capture the bug with a test. Options:

- **New microtest** — create `.qmd.md` + `.expected.json` in `tasks/QMD-4/artifacts/microtests/`. Number = next free. Expected must reflect correct behavior per spec.
- **Fix existing test** — if `.expected.json` is wrong per spec, update it.

```bash
# Confirm the test fails (bug is reproduced through the test)
make test-fast  # should fail on the new/updated test
```

**Constraints:**

- Test MUST fail before the fix — otherwise it's not a bug or the test is wrong
- Expected output MUST match the spec in `docs/format/`
- If the correct expected is unclear — re-read the spec or escalate

### 3. Fix [[qf_step_fix: Step]]

- order: 3

Apply the fix. If only one parser is affected — fix only that one. If the bug is common to all three — fix all three, verifying each separately.

**Constraints:**

- Bug fixes MUST NOT modify files in `docs/format/` — that's a spec change, needs full workflow
- Features MAY update spec in `docs/format/` alongside implementation
- If the fix requires a new error type — full workflow
- If the fix breaks >2 existing tests — stop and think

### 4. Verify [[qf_step_verify: Step]]

- order: 4

```bash
make test-fast
```

All tests must pass. If `.expected.json` needs updating — verify the new expected is correct per spec.

### 5. Describe [[qf_step_describe: Step]]

- order: 5

Commit message format:

```text
fix(<parser>): short description       # for bugs
feat(<scope>): short description       # for features

What was broken/added and why. Which tests are affected.
```

Examples:

- `fix(rust): sanitized_line.find(':') → find(": ") for mixed_field_keys`
- `fix(ts): duplicate children in table rebuild`
- `fix(py): empty __label with [[:Kind]] without Title`
- `feat(format): add map field type (str→str dict via [[field: map]])`

### 6. Record in tracking [[qf_step_track: Step]]

- order: 6

Create a task straight in `done/` — two files (task + result), optionally findings.

```bash
# Determine next ID
./qmdc query ./docs "SELECT __id FROM objects WHERE __kind IN ('Feature', 'Bug')" | grep -oE 'qmd[0-9]+' | grep -oE '[0-9]+' | sort -n | tail -1
# next = max + 1
```

Create `docs/tracking/done/QMD-{next}/QMD-{next}-task.qmd.md`:

For bugs:

```markdown
# QMD-{next}: Short bug description

## Short bug description [[qmd{next}: Bug]]

What was broken and how it manifested.

- status: done
- priority: low
- category: parser
```

For features:

```markdown
# QMD-{next}: Short feature description

## Short feature description [[qmd{next}: Feature]]

What was added and why.

- status: done
- priority: medium
- category: format
```

Create `docs/tracking/done/QMD-{next}/QMD-{next}-result.qmd.md`:

```markdown
# QMD-{next}: Result

## Result [[qmd{next}_result: Result]]

Short description of the fix/feature.

- feature: [[#qmd{next}]]
- completed: YYYY-MM-DD
- files_changed: [file1.rs, file2.ts]
- tests_changed: [NNN-test-name]
```

Optionally create `docs/tracking/done/QMD-{next}/QMD-{next}-findings.qmd.md` if there are design decisions worth recording.

Validate:

```bash
./qmdc parse -i docs/tracking/done/QMD-{next}/QMD-{next}-task.qmd.md > /dev/null
./qmdc parse -i docs/tracking/done/QMD-{next}/QMD-{next}-result.qmd.md > /dev/null
```

## Escalation to full workflow

If during a quickfix it turns out that:

- The scope is larger than expected
- Design decisions need discussion with the team
- The fix/feature breaks many tests
- Correct behavior is unclear
- Requirements are ambiguous

→ Stop, create a task through the full workflow (`task_workflow`).

## Examples

### Example 1: One line in one parser

```text
User: qf: Rust parser false positive on mixed_field_keys for URL with colon

Agent: Reproduced — `sanitized_line.find(':')` matches `http://`.
       Added microtest 178 with URL in field — test fails on Rust ✅
       Fix: `find(": ")` instead of `find(':')`.
       `make test-fast` ✅ — all tests pass including the new one.
       Created QMD-46 in done/ — Bug + Result.
       Commit: fix(rust): find(": ") instead of find(':') in mixed_field_keys
```

### Example 2: Small feature across all parsers

```text
User: qf: add map field type

Agent: Designed syntax: [[field: map]] + list of - key: value.
       Created microtests 179, 180, 181 — tests fail ✅
       Implemented in all 3 parsers (Python, TS, Rust).
       Updated spec: heading-syntax, objects-and-fields, primitives.
       `make test` ✅ — all parsers produce identical output.
       Created QMD-47 in done/ — Feature + Findings + Result.
       Commit: feat(format): add map field type (str→str dict via [[field: map]])
```

### Example 3: Escalation

```text
User: qf: parser doesn't create object for ## Title without [[id]]

Agent: Not a bug — per spec, heading without [[id]] = __TextBlock.
       If different behavior is needed — that's a spec change.
       Recommend creating a task through the full workflow.
```
