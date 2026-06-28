# Task Workflow [[task_workflow: SOP]]

Standard Operating Procedure for executing tasks with checkpoints.

- version: 1.2
- commands: [do, approve, create, decline]

## Overview

The workflow manages the task lifecycle through a state machine with two checkpoints for human review: after triage and after execution.

**When to use:**

- "Do QMD-17" — execute the next step of a task
- "Approve QMD-17" — confirm a checkpoint and continue
- "Create task: description" — create a new task

**CRITICAL RULE:**

🚨 **CANNOT change task status if tests are failing!**

- Run `make test-fast` (or `make test`) after EVERY code change
- `make test-fast` — recommended option, runs tests in parallel (~2x faster)
- If tests fail — this is YOUR problem, fix them immediately
- NEVER move a task to `triage_review` or `done_review` with failing tests
- If tests broke on you — fix the tests
- Failing tests block any status transition

## Prerequisites [[prerequisites: Section]]

**Local setup:**

```bash
# Install all dependencies and build parsers
./setup.sh

# Or just the Rust parser (for qmdc)
cd qmdc-rs && cargo build && cd ..
```

**Requirements:** Python 3.12+, Node.js 18+, Rust 1.70+, uv

**MUST read before starting work:**

1. `docs/tracking/readme.qmd.md` — Feature/Finding/Result templates, folder structure, naming rules
2. `.agents/qmd-guide.qmd.md` — QMD syntax (if unfamiliar)

**Constraints:**

- You MUST read `docs/tracking/readme.qmd.md` before creating any files
- You MUST use templates from readme for Feature, Finding, Result objects
- You MUST NOT invent your own file formats or ID naming conventions
- **You MUST run `make test-fast` (or `make test`) before changing task status**
- **You MUST fix ALL failing tests before proceeding to next step**
- **NEVER change task status if tests are failing**

## Tools [[tools_section: Section]]

**CRITICAL:** Use `./qmdc` for working with QMD files.

**If `./qmdc` doesn't work:**

```bash
./setup.sh  # Will install everything and build qmdc
```

### Reading task status

```bash
# Find task and its status (Feature or Bug)
./qmdc query ./docs "SELECT __id, __kind, json_extract(data, '$.status') as status FROM objects WHERE __kind IN ('Feature', 'Bug') AND __id LIKE '%qmd17%'"
```

### File validation

```bash
# After creating/changing a QMD file — MUST validate
./qmdc parse -i docs/tracking/planned/QMD-17/QMD-17-task.qmd.md > /dev/null && echo "✅ OK" || echo "❌ FAIL"
```

### Finding tasks

```bash
# All incomplete tasks (Feature and Bug)
./qmdc query ./docs "SELECT __id, __kind, __label, json_extract(data, '$.status') as status FROM objects WHERE __kind IN ('Feature', 'Bug') AND json_extract(data, '$.status') != 'done'"

# Tasks awaiting review
./qmdc query ./docs "SELECT __id, __kind FROM objects WHERE __kind IN ('Feature', 'Bug') AND json_extract(data, '$.status') IN ('triage_review', 'done_review')"
```

## Parameters [[task_workflow_params: Parameters]]

Parameters are extracted from the operator's command.

- task_id: Task ID in the format QMD-XX (required for do/approve)
- command: do | approve | create (required)
- description: description of the new task (required for create)

## Steps

### 1. Parse Command [[step_parse: Step]]

Determine the command and find the task.

- order: 1
- input: operator's command

**Constraints:**

- You MUST extract command type (do/approve/create) from operator message
- You MUST use `./qmdc query` to find task and read status
- You MUST NOT read task files directly — use qmdc query
- You MUST NOT guess status or task location

**Actions:**

1. Parse command: "Do X" → do, "Approve X" → approve, "Create task" → create
2. Find task and status:

   ```bash
   ./qmdc query ./docs "SELECT __id, __file, json_extract(data, '$.status') as status FROM objects WHERE __kind = 'Feature' AND __id LIKE '%qmd{id}%'"
   ```

### 2. Triage [[step_triage: Step]]

Technical analysis of the task. Create findings with an implementation and testing plan.

- order: 2
- trigger_status: planned
- result_status: triage_review
- checkpoint: true
- location: planned/

**Constraints:**

- You MUST analyze task requirements thoroughly
- You MUST identify and document open questions FIRST (unclear requirements, ambiguous specs)
- You MUST create Finding objects in {ID}-findings.qmd.md
- **You MUST use unique IDs with task prefix — `[[qmd{N}_finding_name: Finding]]`**
- **NEVER use generic IDs like `[[finding_test_design]]` — always add task prefix**
- You MUST validate created files with `./qmdc parse -i {file}`
- You MUST identify affected files and functions
- You MUST document approach and test cases
- **You MUST include test_plan in findings (existing tests + new tests if needed)**
- **You MUST prefer data-driven tests over code tests**
- You MUST set status to `triage_review` after creating findings
- You MUST stop execution after setting triage_review
- You MUST NOT proceed to implementation
- You MUST NOT change status to in_progress

**Actions:**

1. Read task description from Feature object
2. **Identify open questions** — unclear requirements, missing details, ambiguous specs
3. Analyze codebase to identify affected areas
4. Create Finding objects with:
   - **ID format — `[[qmd{N}_finding_name: Finding]]` (e.g., `[[qmd11_finding2: Finding]]`)**
   - **open_questions** (if any) — list of questions needing clarification
   - affected_files: concrete file paths
   - affected_functions: function names
   - solution: implementation approach
   - **test_plan**: how to test (see below)
5. **Test plan must include:**
   - Existing tests — which tests already cover this (or "none")
   - New tests needed — test type (parser microtests, workspace tests, lsp microtests, sql tests), location, test cases
   - How to verify — brief description of validation process
6. Validate: `./qmdc parse -i {ID}-findings.qmd.md > /dev/null`
7. **Run tests: `make test-fast` — ALL tests MUST pass before proceeding**
8. Update Feature status: `planned` → `triage_review`
9. Validate: `./qmdc parse -i {ID}-task.qmd.md > /dev/null`
10. STOP and report: "Triage complete. Awaiting approval to start work."
    - If open questions exist — list them and ask for clarification

### 3. Approve Triage [[step_approve_triage: Step]]

Checkpoint: operator confirms triage.

- order: 3
- trigger_status: triage_review
- trigger_command: approve
- result_status: in_progress
- checkpoint: false
- location_change: planned/ → active/

**Constraints:**

- You MUST only execute on explicit "approve" command
- You MUST move task folder from planned/ to active/
- You MUST update status to in_progress
- You MAY start implementation immediately after

**Actions:**

1. Move folder: `mv docs/tracking/planned/QMD-{id} docs/tracking/active/`
2. Update Feature status: `triage_review` → `in_progress`
3. Report: "Task activated. Starting work."

### 4. Implementation [[step_implementation: Step]]

Execute the task according to findings.

- order: 4
- trigger_status: in_progress
- result_status: done_review
- checkpoint: true
- location: active/

**Constraints:**

- You MUST follow the plan from Finding objects
- You MUST implement ALL goals listed in the task — partial completion is NOT done
- You MUST implement changes according to findings
- You MUST create Result object in {ID}-result.qmd.md
- You MUST validate created files with `./qmdc parse -i {file}`
- You MUST run a code review on uncommitted files before setting done_review (see Code Review Gate below)
- You MUST set status to `done_review` only after ALL goals are complete AND code review passes
- You MUST stop execution after setting done_review
- You MUST NOT move task to done/
- You MUST NOT set status to done

**Test Constraints:**

- PRIORITY: data-driven tests over code tests (`.sql` + `.expected.json`, not `.rs`/`.ts` test code)
- Data-driven tests MUST pass in all parser implementations (Python, TypeScript, Rust) — except LSP-specific tests
- You MUST NOT write new test code without first checking if data-driven test exists
- You MUST prefer adding `.sql` + `.expected.json` files over writing Rust/TypeScript test code
- You MUST read existing tests in the project before proposing new ones
- You MUST explain why existing tests are insufficient before proposing new tests
- You MUST get user approval before creating new test files
- You MUST NOT blindly update expected files — verify the new values are correct

**Actions:**

1. Read findings for implementation plan
2. Implement changes in codebase
3. **Run tests after EACH change: `make test-fast`**
4. **If ANY test fails:**
   a. **STOP immediately — do NOT proceed**
   b. **Fix the failing test or revert your changes**
   c. **Re-run `make test-fast` until ALL tests pass**
   d. **NEVER change task status with failing tests**
5. If tests needed:
   a. Read existing test files in the affected area
   b. Explain to user why new tests are needed (or why existing don't fit)
   c. Wait for user approval before creating test files
6. Create Result object with:
   - files_changed: list of modified files
   - summary: brief description of changes
7. Validate: `./qmdc parse -i {ID}-result.qmd.md > /dev/null`
8. **Final test run: `make test-fast` — ALL tests MUST pass**
9. **Code Review Gate:**
   a. Run `/code-review` in a clean subagent on all uncommitted files
   b. If the review finds BLOCKING/CRITICAL issues — fix them before proceeding
   c. Re-run code review until no criticals remain
   d. Only then proceed to set done_review
10. Update Feature status: `in_progress` → `done_review`
11. Validate: `./qmdc parse -i {ID}-task.qmd.md > /dev/null`
12. STOP and report: "Work complete. Awaiting approval to finalize."

### 5. Approve Result [[step_approve_result: Step]]

Checkpoint: operator confirms the result.

- order: 5
- trigger_status: done_review
- trigger_command: approve
- result_status: done
- checkpoint: false
- location_change: active/ → done/

**Constraints:**

- You MUST only execute on explicit "approve" command
- You MUST move task folder from active/ to done/
- You MUST update status to done

**Actions:**

1. Move folder: `mv docs/tracking/active/QMD-{id} docs/tracking/done/`
2. Update Feature status: `done_review` → `done`
3. Report: "Task completed and archived."

### 6. Create Task [[step_create: Step]]

Create a new task.

- order: 6
- trigger_command: create
- result_status: planned

**Constraints:**

- You MUST determine next Task ID by scanning all folders
- You MUST create folder structure in planned/
- You MUST create all three files — task, findings, result
- You MUST validate all created files with `./qmdc parse`
- You MUST set initial status to planned

**Actions:**

1. Find max ID:

   🚨 **WARNING! Search BOTH Feature AND Bug — otherwise you'll create a duplicate ID!**

   ```bash
   ./qmdc query ./docs "SELECT __id FROM objects WHERE __kind IN ('Feature', 'Bug')" | grep -oE 'qmd[0-9]+' | grep -oE '[0-9]+' | sort -n | tail -1
   ```

2. Next ID = max + 1 (or 1 if empty)
3. Create folder: `docs/tracking/planned/QMD-{next}/`
4. Create files from templates
5. Validate all files:

   ```bash
   ./qmdc parse -i docs/tracking/planned/QMD-{next}/QMD-{next}-task.qmd.md > /dev/null
   ./qmdc parse -i docs/tracking/planned/QMD-{next}/QMD-{next}-findings.qmd.md > /dev/null
   ./qmdc parse -i docs/tracking/planned/QMD-{next}/QMD-{next}-result.qmd.md > /dev/null
   ```

6. Report: "Created task QMD-{next}"

## State Machine [[task_state_machine: StateMachine]]

Visualization of status transitions.

- states: [planned, triage_review, in_progress, done_review, done, declined]
- checkpoints: [triage_review, done_review]

```text
planned ──[triage]──► triage_review ──[approve]──► in_progress
                          │                            │
                       CHECKPOINT                   [work]
                      (wait approve)                   │
                                                       ▼
done ◄──[approve]── done_review ◄─────────────────────┘
             │
          CHECKPOINT
         (wait approve)

   (any state) ──[decline]──► declined   (terminal; folder → declined/)
```

### 7. Decline Task [[step_decline: Step]]

Retire a task we will not do — genuinely dead, obsolete, or already solved a
different way. Terminal, like `done`, but for work that was never executed.

- order: 7
- trigger_command: decline
- result_status: declined
- location_change: planned|active/ → declined/

**Constraints:**

- You MUST only execute on an explicit "decline" command
- You MUST set `status: declined` and record a one-line `decline_reason` on the Feature/Bug
- You MUST move the task folder to `docs/tracking/declined/`
- You MUST NOT delete the task — declining preserves the record and its reasoning

**Actions:**

1. Set status to `declined` and add `decline_reason: <why>` to the Feature/Bug object
2. Move folder: `mv docs/tracking/{planned,active}/QMD-{id} docs/tracking/declined/`
3. Validate: `./qmdc parse -i {ID}-task.qmd.md > /dev/null`
4. Report: "Task QMD-{id} declined and archived in declined/."

## Troubleshooting [[task_troubleshooting: Section]]

### Task not found

- Check all three folders: planned/, active/, done/
- Verify Task ID format: QMD-{number}

### Wrong status for command

- "do" on triage_review → respond "Awaiting triage approval"
- "do" on done_review → respond "Awaiting result approval"
- "approve" on planned → respond "Triage is needed first"

### Checkpoint violated

If agent proceeded past checkpoint without approval:

- Revert status change
- Report violation to operator
- Wait for explicit approval

## Examples [[task_examples: Section]]

### Example 1: Full workflow

```text
Operator: Create task: Add comment support to the parser
Agent: Created task QMD-18 in docs/tracking/planned/QMD-18/

Operator: Do QMD-18
Agent: [performs triage, creates findings]
       Triage complete. Findings:
       - Affected files: parser/lexer.ts, parser/ast.ts
       - Approach: add COMMENT token, update grammar
       Awaiting approval to start work.

Operator: Approve QMD-18
Agent: Task activated. Starting work.
       [performs implementation]
       Work complete. Changed files: lexer.ts, ast.ts, tests/comments.test.ts
       Awaiting approval to finalize.

Operator: Approve QMD-18
Agent: Task QMD-18 completed and archived in done/.
```

### Example 2: Checkpoint enforcement

```text
Operator: Do QMD-18
Agent: Task status: triage_review
       Awaiting triage approval. Use "Approve QMD-18" to continue.
```
