# Task Review [[task_review: SOP]]

Standard Operating Procedure for reviewing task completion before approval.

- version: 1.0
- commands: [review]

## Overview

This SOP checks task execution quality and identifies issues. **Does not fix anything** — only analyzes and outputs a list of problems.

**When to use:**

- "Review QMD-31" — review a task before approval
- "Check QMD-31" — same thing

**What it outputs:**

- List of issues, bugs, discrepancies with findings
- Scenarios not covered by tests
- Potential edge cases

**What it does NOT do:**

- Does not fix code
- Does not update task files
- Does not change status
- Does not write "everything is fine"
- Does not create documents — the report is output ONLY to chat

## Parameters [[task_review_params: Parameters]]

- task_id: Task ID in the format QMD-XX (required)

## Steps

### 1. Collect Task Data [[step_collect: Step]]

Collect all data about the task.

- order: 1

**Actions:**

1. Find the task:

   ```bash
   ./qmdc query ./docs "SELECT __id, __file, json_extract(data, '$.status') as status FROM objects WHERE __kind IN ('Feature', 'Bug') AND __id LIKE '%qmd{id}%'"
   ```

2. Read task files:
   - `{ID}-task.qmd.md` — description and requirements
   - `{ID}-findings.qmd.md` — implementation and testing plan
   - `{ID}-result.qmd.md` — description of what was done

3. Get changes from git:

   ```bash
   git status
   git diff --name-only HEAD
   git diff <changed_files>
   ```

### 2. Analyze Implementation [[step_analyze: Step]]

Compare the implementation against findings.

- order: 2

**Constraints:**

- You MUST check EVERY item in findings against actual implementation
- You MUST read changed files and understand the code
- You MUST identify any deviations from the plan
- You MUST NOT suggest fixes or improvements

**Checklist:**

1. **affected_files** — have all files from findings been changed?
2. **affected_functions** — have all functions been touched?
3. **solution** — has the solution been implemented as described?
4. **test_plan** — have all planned tests been written?

### 3. Analyze Tests [[step_tests: Step]]

Check test quality.

- order: 3

**Constraints:**

- You MUST verify tests actually test what they claim
- You MUST identify untested scenarios
- You MUST check test assertions are meaningful (not just prints)
- You MUST NOT write or fix tests

**Checklist:**

1. **Tests from findings** — have all been written?
2. **Assertions** — is there an assert for each check (not just println)?
3. **Edge cases** — are edge cases covered?
4. **Tests pass** — run `make test`

### 4. Find Code Issues [[step_issues: Step]]

Find potential bugs in the code.

- order: 4

**Constraints:**

- You MUST look for common bug patterns
- You MUST check for inconsistencies in the codebase
- You MUST identify security or performance concerns
- You MUST NOT fix anything

**Bug patterns to check:**

1. **Case sensitivity** — string comparisons without normalization
2. **Error handling** — unhandled errors, unwrap() without checks
3. **Race conditions** — async code without synchronization
4. **Resource leaks** — unclosed resources, memory leaks
5. **Hardcoded values** — magic numbers, hardcoded paths
6. **Missing validation** — missing input validation
7. **Incomplete implementation** — TODO, FIXME, unfinished code

### 5. Generate Report [[review_step_report: Step]]

Generate an issues report.

- order: 5

**Constraints:**

- You MUST output report directly in chat (NOT create any files)
- You MUST output ONLY problems found
- You MUST NOT include "everything is good" statements
- You MUST be specific about each issue (file, line, description)
- You MUST prioritize issues by severity

**Output format (in chat):**

```markdown
## 🔴 Critical bugs
(bugs that break functionality)

## 🟠 Discrepancies with findings
(what was planned but not implemented or implemented differently)

## 🟡 Scenarios not covered by tests
(what should be tested but is not)

## 🔵 Potential issues
(code smells, performance concerns, edge cases)
```

**If no issues found:**
If no issues are found, output only:

```text
✅ No issues found. Ready for approval.
```

## Example [[task_review_example: Section]]

```text
Operator: Review QMD-31

Agent: 
## 🔴 Critical bugs

1. **Case sensitivity in scan_workspace_folder** (server.rs:231)
   - `folder_path.join("readme.qmd.md")` — hardcoded lowercase
   - On Linux, the file `README.qmd.md` won't be found
   - Workspace will get its ID from the folder instead of from readme

## 🟠 Discrepancies with findings

(none)

## 🟡 Scenarios not covered by tests

1. **Case-insensitive readme** — no test for `README.qmd.md`
2. **Multiple file creation** — no test for batch CREATED events

## 🔵 Potential issues

1. **Performance** — full WalkDir scan on each create/delete
2. **Deleted incremental logic removed** — now delete = full rescan
```
