# Tracking [[tracking:__Namespace]]

Tracking tasks, technical findings, and development results for the QMD project.

## Overview

The `tracking` namespace contains folders with tasks. Each task has three files:

- **{ID}-task.qmd.md** — task description (Feature object)
- **{ID}-findings.qmd.md** — technical findings (Finding objects)
- **{ID}-result.qmd.md** — execution result (Result object)

## Roadmap: Where We Are [[roadmap: Dashboard]]

Dashboard for tracking project progress: current tasks, statistics, and active work.

- title: Roadmap: Where We Are
- description: Dashboard for tracking project progress: current tasks, statistics, and active work
- sections: Task Statistics, What We're Doing Now

### Task Statistics [[tasks_stats: text]]

```table
scope: workspace
sql: |
  SELECT
    json_extract(data, '$.status') as status,
    COUNT(*) as count,
    CASE json_extract(data, '$.status')
      WHEN 'done' THEN '✅'
      WHEN 'in_progress' THEN '🚧'
      WHEN 'planned' THEN '📅'
      WHEN 'triage_review' THEN '👀'
      WHEN 'done_review' THEN '✅'
      ELSE '❓'
    END as icon
  FROM objects
  WHERE __kind IN ('Feature', 'Bug')
    AND __namespace = 'tracking'
  GROUP BY status
  ORDER BY
    CASE status
      WHEN 'in_progress' THEN 1
      WHEN 'planned' THEN 2
      WHEN 'triage_review' THEN 3
      WHEN 'done_review' THEN 4
      WHEN 'done' THEN 5
    END
```

### What We Are Doing Now [[what_we_do_now: text]]

```table
scope: workspace
sql: |
  SELECT
    f.__id as task,
    f.__label as name,
    json_extract(f.data, '$.priority') as priority,
    json_extract(f.data, '$.status') as status,
    json_extract(f.data, '$.category') as category,
    CASE json_extract(f.data, '$.priority')
      WHEN 'critical' THEN '🔴'
      WHEN 'high' THEN '🟠'
      WHEN 'medium' THEN '🟡'
      WHEN 'low' THEN '🟢'
      ELSE '⚪'
    END as priority_icon
  FROM objects f
  WHERE f.__kind IN ('Feature', 'Bug')
    AND f.__namespace = 'tracking'
    AND json_extract(f.data, '$.status') IN ('planned', 'in_progress', 'triage_review', 'done_review')
  ORDER BY
    CASE json_extract(f.data, '$.status')
      WHEN 'in_progress' THEN 1
      WHEN 'done_review' THEN 2
      WHEN 'triage_review' THEN 3
      WHEN 'planned' THEN 4
    END,
    CASE json_extract(f.data, '$.priority')
      WHEN 'critical' THEN 1
      WHEN 'high' THEN 2
      WHEN 'medium' THEN 3
      WHEN 'low' THEN 4
    END,
    f.__id
```

### All Features [[query_all_features: Query]]

- sql: SELECT __id,__label, __kind, json_extract(data, '$.status') as status, json_extract(data, '$.priority') as priority FROM objects WHERE__kind IN ('Feature', 'Bug') AND __namespace = 'tracking' ORDER BY status, CASE json_extract(data, '$.priority') WHEN 'critical' THEN 1 WHEN 'high' THEN 2 WHEN 'medium' THEN 3 WHEN 'low' THEN 4 END,__id

### Features by Status [[query_features_by_status: Query]]

- sql: SELECT json_extract(data, '$.status') as status, COUNT(*) as count FROM objects WHERE __kind IN ('Feature', 'Bug') AND__namespace = 'tracking' GROUP BY status

### Findings by Category [[query_findings_by_category: Query]]

- sql: SELECT json_extract(data, '$.category') as category, COUNT(*) as count FROM objects WHERE __kind = 'Finding' AND__namespace = 'tracking' GROUP BY category

### Done Features [[query_done_features: Query]]

- sql: SELECT __id,__kind, __label, json_extract(data, '$.priority') as priority, substr(json_extract(data, '$.description'), 1, 80) as description FROM objects WHERE__kind IN ('Feature', 'Bug') AND __namespace = 'tracking' AND json_extract(data, '$.status') = 'done' ORDER BY CASE json_extract(data, '$.priority') WHEN 'critical' THEN 1 WHEN 'high' THEN 2 WHEN 'medium' THEN 3 WHEN 'low' THEN 4 END,__id

### Active Features [[query_active_features: Query]]

- sql: SELECT __id,__kind, __label, json_extract(data, '$.priority') as priority, substr(json_extract(data, '$.description'), 1, 80) as description FROM objects WHERE__kind IN ('Feature', 'Bug') AND __namespace = 'tracking' AND json_extract(data, '$.status') = 'in_progress' ORDER BY CASE json_extract(data, '$.priority') WHEN 'critical' THEN 1 WHEN 'high' THEN 2 WHEN 'medium' THEN 3 WHEN 'low' THEN 4 END,__id

### Planned Features [[query_planned_features: Query]]

- sql: SELECT __id,__kind, __label, json_extract(data, '$.priority') as priority, substr(json_extract(data, '$.description'), 1, 80) as description FROM objects WHERE__kind IN ('Feature', 'Bug') AND __namespace = 'tracking' AND json_extract(data, '$.status') = 'planned' ORDER BY CASE json_extract(data, '$.priority') WHEN 'critical' THEN 1 WHEN 'high' THEN 2 WHEN 'medium' THEN 3 WHEN 'low' THEN 4 END,__id

### Triage Review Features [[query_triage_review_features: Query]]

- sql: SELECT __id,__kind, __label, json_extract(data, '$.priority') as priority, substr(json_extract(data, '$.description'), 1, 80) as description FROM objects WHERE__kind IN ('Feature', 'Bug') AND __namespace = 'tracking' AND json_extract(data, '$.status') = 'triage_review' ORDER BY CASE json_extract(data, '$.priority') WHEN 'critical' THEN 1 WHEN 'high' THEN 2 WHEN 'medium' THEN 3 WHEN 'low' THEN 4 END,__id

### Done Review Features [[query_done_review_features: Query]]

- sql: SELECT __id,__kind, __label, json_extract(data, '$.priority') as priority, substr(json_extract(data, '$.description'), 1, 80) as description FROM objects WHERE__kind IN ('Feature', 'Bug') AND __namespace = 'tracking' AND json_extract(data, '$.status') = 'done_review' ORDER BY CASE json_extract(data, '$.priority') WHEN 'critical' THEN 1 WHEN 'high' THEN 2 WHEN 'medium' THEN 3 WHEN 'low' THEN 4 END,__id

## Task Structure

```text
docs/tracking/
├── readme.qmd.md
├── done/                           # Completed tasks
│   ├── QMD-2/
│   │   ├── QMD-2-task.qmd.md
│   │   ├── QMD-2-findings.qmd.md
│   │   ├── QMD-2-result.qmd.md
│   │   └── artifacts/
│   └── QMD-4/
├── active/                         # Active tasks
│   ├── QMD-5/
│   ├── QMD-6/
│   │   ├── QMD-6-task.qmd.md
│   │   ├── QMD-6-findings.qmd.md
│   │   ├── QMD-6-result.qmd.md
│   │   ├── artifacts/
│   │   └── subtasks/               # Subtasks (for large tasks)
│   │       ├── tier1-completion.qmd.md
│   │       ├── tier1-hover.qmd.md
│   │       └── tier2-references.qmd.md
│   ├── QMD-7/
│   └── UNIFIED-DOCS/
└── planned/                        # Planned tasks
    └── QMD-8/
```

**Subtasks:** For large tasks, a `subtasks/` folder is created with separate Feature objects linked via `parent_task: [[#main_task_id]]`.

## Workflow

### 1. Creating a Task

Create a folder `docs/tracking/planned/{ID}/` with three files:

**{ID}-task.qmd.md** — Feature object with the task description:

```markdown
# {ID}: Task Name

## Task [[task_id: Feature]]

Task description.

- status: planned
- priority: medium
- requires_changes: []

## Checklist

- [ ] Understood the task
- [ ] Studied the code
- [ ] Created a plan and prototypes in `artifacts/`
- [ ] Tested the solution
- [ ] Moved the code into the project
- [ ] Created Result.md and Findings.md
```

**{ID}-findings.qmd.md** — empty file (filled in during work):

```markdown
# {ID}: Findings

Technical findings will be added during work.
```

**{ID}-result.qmd.md** — empty file (filled in after completion):

```markdown
# {ID}: Result

The result will be added after task completion.
```

**artifacts/** — folder for prototypes, tests, examples.

### 2. Working on a Task

When starting work:

- Move the folder from `planned/` to `active/`
- Update the Feature status to `in_progress`

During work:

- Create prototypes and tests in `artifacts/`
- Document findings in `{ID}-findings.qmd.md` as Finding objects
- For large tasks, create `subtasks/` with subtask files

### 3. Completing a Task

After completion:

- Create a Result object in `{ID}-result.qmd.md`
- Link the Result to the Feature via the `feature` field
- Link Finding objects to the Feature via the `related_to` field
- Update the Feature status to `done`
- Move the folder from `active/` to `done/`

## Templates

### Feature

```markdown
## Task Name [[task_id: Feature]]

Task description.

- status: planned
- priority: medium
- requires_changes: []
- findings: []
- result: null
```

### Finding

```markdown example
## Finding Name [[finding_id: Finding]]

Description of the technical finding.

- category: parser
- related_to: [[#task_id]]
- solution: Description of the solution
```

### Result

```markdown example
## Task Result [[result_id: Result]]

Description of the task execution result.

- feature: [[#task_id]]
- files_changed: [file1.ts, file2.ts]
- tests_added: [test1.ts]
```

## SQL Queries (examples)

```sql
-- All planned features
SELECT __id, status, priority FROM objects 
WHERE __kind = 'Feature' AND status = 'planned'

-- What a feature requires
SELECT target_id FROM edges 
WHERE source_id = 'task_id' AND source_field = 'requires_changes'

-- Findings by category
SELECT __id FROM objects 
WHERE __kind = 'Finding' AND json_extract(data, '$.category') = 'parser'

-- Progress by status
SELECT json_extract(data, '$.status') as status, COUNT(*) as count 
FROM objects WHERE __kind = 'Feature' 
GROUP BY status
```

## Feature Template (standard fields)

For consistency, all Feature objects should have the following fields:

**Required:**

- `status` — planned | triage_review | in_progress | done_review | done
- `priority` — low | medium | high | critical  
- `category` — parser | lsp | extension | format | testing | docs

**Statuses:**

- `planned` — task created, awaiting triage
- `triage_review` — triage complete, awaiting operator approval
- `in_progress` — task in progress
- `done_review` — work complete, awaiting operator approval
- `done` — task completed and archived

**Optional (for connectivity):**

- `affects` — references to affected components/modules
- `requires_changes` — references to objects that need to be changed
- `related_task` — references to related tasks

**Example:**

```markdown example
## Task Name [[task_id: Feature]]

Task description.

- status: active
- priority: high
- category: lsp
- affects: [[#lsp:definition]], [[#lsp:hover]]
- requires_changes: [[#architecture:lsp_server]]
- related_task: [[#qmd6_lsp]]
```
