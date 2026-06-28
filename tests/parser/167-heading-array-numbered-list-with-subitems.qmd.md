## Task [[task1: Task]]

- category: implementation

### Open Questions [[open_questions: array]]

1. **Backward compat:** Support old field `objects`?
   - **Answer:** Replace immediately, internal API

2. **Empty groups:** Include empty array `kindGroups: []`?
   - **Answer:** Yes, for consistency

3. **Sorting:** Keep current sorting logic?
   - **Answer:** Yes, already works correctly

### Steps [[steps: array]]

1. Change code in `get_tree`:
   - Group objects by `kind`
   - Create `kindGroups`
   - Replace `objects` with `kindGroups`

2. Update expected files:
   - `workspace-mixed/namespace.expected.json`
   - `workspace-no-ns/namespace.expected.json`

3. Run tests:
   - `make test`
   - Check structure matches

