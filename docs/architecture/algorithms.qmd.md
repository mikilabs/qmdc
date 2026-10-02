# Algorithms

Key algorithms of the QMDC system, independent of any specific parser implementation.

## Parsing Algorithm [[parsing_algorithm: Algorithm]]

- source_file: parser.py / parser.ts / parser.rs
- complexity: O(n) where n = number of lines
- used_in: [[#parsers:cmd_parse]]

### Description [[description: text]]

Converts QMD.md into JSON objects.

**Input:** Markdown string + parsing options (seed for ID auto-generation, output format).

**Output:** Array of JSON objects with system and user fields.

**High-level algorithm:**

```text
1. Split markdown into lines
2. Initialize:
   - objects_map: Map<id, object>
   - object_stack: Stack<(id, level)> for hierarchy tracking
   - current_object: currently processed object

3. For each line:
   a. If heading (##, ###, etc.):
      - Save previous current_object
      - Create new object:
        * Extract ID from [[id]] or generate from label
        * Extract Kind from [[id: Kind]]
        * Set level by number of #
        * Set line = line number
      - Update object_stack to determine __parent:
        * Pop objects with level >= current.level
        * If stack not empty → set __parent = stack.top
        * Push (id, level) onto stack

   b. If list (- key: value):
      - Parse key: value
      - Add to current_object.fields
      - If value contains a reference → add to references

   c. If text (not heading, not list):
      - Process as text field or TextBlock

4. Save last current_object

5. Build final JSON objects:
   - Add system fields (__id, __kind, __level, __line, __parent)
   - Add user fields
   - Add __references (if format = Full)

6. Return array of objects
```

**Key characteristics:**

- **ID auto-generation** — if no ID specified, generated from label (lowercase, spaces → _)
- **Hierarchy** — object stack tracks nesting by heading levels
- **References** — patterns like `[[#id]]`, `[[#namespace:id]]`, `[[#workspace:namespace:id]]` are extracted
- **Lossless** — field order, object order, and reference positions are preserved

## Rebuild Algorithm [[rebuild_algorithm: Algorithm]]

- source_file: parser.py / parser.ts / rebuild.rs
- complexity: O(n) where n = number of objects
- used_in: [[#parsers:cmd_rebuild]]

### Description [[description: text]]

Restores QMD.md from JSON objects (inverse of parsing_algorithm).

**Core principle: raw slice in, raw slice out.** Parse stores `__comments.content` and multiline text fields as raw markdown slices. Rebuild outputs them as-is, without interpreting the content.

**High-level algorithm:**

```text
1. Classify objects:
   - Find __Document (if any) — determines content order
   - Find __Workspace / __Namespace (if any) — root objects
   - Find __TextBlock objects
   - Remaining — business objects

2. Determine output order:
   a. If __Document exists:
      - Iterate over __Document.content[]
      - For each reference, find object and output it
   b. If __Workspace or __Namespace (without __Document):
      - Output root object (heading + fields)
      - Output child objects (by __parent references)
   c. If only business objects:
      - Output objects in array order

3. Output a single object:
   a. Heading: level # symbols + __label + [[__id]] or [[__id: __kind]]
   b. Comments after __self: raw markdown as-is
   c. Fields in insertion order:
      - Check __syntax for output format selection
      - Simple field: "- key: value"
      - yaml_array: "- key: [a, b, c]"
      - markdown_list: "### Label [[field: array]]\n- item1\n- item2"
      - multiline_text: "### Label [[field: text]]\n\ncontent"
      - yaml_object / headers / table: corresponding syntax
   d. Nested objects: output inline via recursion
```

## Workspace Indexing [[workspace_indexing: Algorithm]]

- source_file: workspace.py / workspace.ts / workspace.rs
- complexity: O(n × m) where n = number of files, m = average file size
- used_in: [[#parsers:cmd_workspace_parse]]

### Description [[description: text]]

Builds an index of a multi-file workspace.

**Algorithm:**

```text
1. Find workspace root:
   - Locate readme.qmd.md with [[id:__Workspace]]
   - If not found → error "Not a workspace"

2. Scan files:
   - Recursively find all *.qmd.md files
   - Apply .qmdcignore rules (if present)

3. Parse each file:
   - Call parsing_algorithm
   - Add metadata to each object:
     * __file: relative path from workspace root
     * __workspace: workspace object ID
     * __namespace: namespace ID (if file is in a subfolder with __Namespace)

4. Determine namespace:
   - If file is in a subfolder whose readme.qmd.md contains [[id:__Namespace]]
   - Then __namespace = that namespace object's ID
   - Otherwise __namespace = null (object at workspace root)

5. Validate:
   - Check ID uniqueness within namespace
   - Collect parsing errors

6. Build indexes:
   - by_id: Map<id, object> for direct lookup
   - by_ns_id: Map<(namespace, id), object> for scoped lookup
   - by_kind_id: Map<(kind, id), object[]> for Kind-qualified lookup
   - by_local_id: Map<local_id, object[]> for __local_id fallback resolution

7. Return: objects, files, errors, indexes
```

**.qmdcignore** follows git's `.gitignore` rules exactly. The three parsers carry the same port
of git's own matcher (`ignore.rs`, `ignore.py`, `ignore.ts`), and
`tests/ignore/gitignore-matrix.json` pins it against git's answers.

- Blank lines and lines starting with `#` are skipped, and `\#` is a literal `#`. Trailing spaces
  are dropped unless escaped with `\`; leading spaces are part of the pattern.
- `*` matches anything but `/`, `?` one byte but `/`, and `[...]` one byte from a set: `[!...]`
  or `[^...]` negates it, and ranges and classes such as `[[:digit:]]` work.
- `**` crosses `/` as a whole path segment: `**/x` matches `x` at any depth, `x/**` everything
  inside `x`, and `a/**/b` any number of directories between, none included. The line's literal
  prefix is matched first and dropped, so `te**/c.qmd.md` crosses too — `te` is literal, and the
  `**` then opens what is left — while `t*s/su**/c.qmd.md` has a wildcard before it, nothing is
  dropped, and that `**` is a plain `*`. Followed by anything other than `/` or the end of the
  line it is always a plain `*`, as in `tracking/**artifacts/*`.
- A line with no slash, or only a trailing one, matches a name at any depth. A slash at the
  start or in the middle anchors the line to the directory of the ignore file.
- A trailing `/` makes the line match directories only.
- A line starting with `!` brings back what an earlier line excluded; the last matching line
  wins.
- A line that matches a directory hides everything below it, and nothing below an excluded
  directory can be brought back.
- Matching is case-sensitive on every OS.

Each workspace reads the `.qmdcignore` in its own root. The `.qmdcignore` of a container, or of
a `--with` path that is not itself a workspace, only steers which workspaces are found. A
`.qmdcignore` in any other subdirectory is not read.

## SQLite Mapping [[sqlite_mapping: Algorithm]]

- source_file: db.py / db.ts / db/mod.rs
- complexity: O(n) where n = number of objects
- used_in: [[#parsers:cmd_query]]

### Description [[description: text]]

Translates a workspace into a SQLite database for SQL queries.

**Why SQLite mapping:**

1. **Powerful queries** — SQL enables complex filtering, grouping, JOINs
2. **Relationship analysis** — the edges table enables graph analysis
3. **Fast search** — indexes on __kind,__namespace, __id
4. **Standard interface** — SQL is a universal query language

**Schema:**

Table `objects`: `__workspace`, `__namespace`, `__id`, `__global_id` (generated: `workspace:namespace:id`), `__kind`, `__label`, `__file`, `__parent`, `__line`, `__level`, `data` (JSON). Primary key: `(__workspace, __namespace, __id)`.

Table `edges`: `source_id`, `source_field`, `target_id`, `edge_type`, `__workspace`. Unique constraint on `(source_id, source_field, target_id, edge_type)`. Foreign keys to `objects(__global_id)`.

**Indexes:** `idx_objects_kind`, `idx_objects_namespace`, `idx_objects_parent`, `idx_objects_workspace`, `idx_edges_source`, `idx_edges_target`.

**Loading algorithm:**

```text
1. Create in-memory SQLite database
2. Create schema (tables + indexes)
3. For each object:
   a. Extract system fields → INSERT into objects
   b. Serialize user fields to JSON → data column
   c. Extract references from all fields:
      - Find `[[#id]]`, `[[#namespace:id]]`, `[[#workspace:namespace:id]]` patterns
      - For text fields: check preamble (all-or-nothing rule)
      - INSERT into edges (ON CONFLICT DO NOTHING)
4. Validate: check all target_ids exist in objects
```

## Reference Resolution [[reference_resolution: Algorithm]]

- source_file: lsp/server.rs
- complexity: O(1) with index, O(n) without
- used_in: [[#parsers:rust_parser]]

### Description [[description: text]]

Resolves references of the form `[[#id]]` to objects.

**Reference formats:**

A reference target is a right-aligned suffix of `workspace:namespace:id` (the
`__global_id` grammar), with an optional `.field` suffix on the id. There is no Kind
segment.

1. Local reference: `[[#id]]` — own namespace first, then any namespace of the same workspace
2. Different namespace: `[[#namespace:id]]`
3. Cross-workspace: `[[#workspace:namespace:id]]`
4. Cross-workspace, namespace elided: `[[#workspace::id]]` — any namespace of that workspace

**Algorithm:**

```text
1. Parse reference:
   - Extract content from [[...]]
   - Remove leading # if present
   - Split by : into parts
   - Determine: workspace_id, namespace_id, kind, id

2. Determine context:
   - Current workspace (from object's __workspace)
   - Current namespace (from object's __namespace)

3. Search for object:
   a. If workspace_id specified → search in that workspace
   b. Otherwise → search in current workspace
   c. If namespace_id specified → search in that namespace
   d. Otherwise → search current namespace, then workspace root
   e. If kind specified → filter by __kind
   f. Search by __id → if found, return object
   g. If not found by __id → search by __local_id:
      - Find all objects where __local_id matches the reference id
      - If exactly one match → resolve to that object (success)
      - If multiple matches → ambiguous_reference error
      - If no match → broken_link error

4. Validate:
   - Check object existence
   - Check uniqueness (if kind not specified)
   - Return result or error
```

**Resolution priority:**

1. Exact match on `__id` (primary lookup)
2. Fallback match on `__local_id` (when `__id` lookup fails)
3. If `__local_id` match is unambiguous (exactly one object) → resolve successfully
4. If multiple objects share the same `__local_id` → `ambiguous_reference` error

**Optimization:** index maps for O(1) lookup — `Map<id, object>`, `Map<(namespace, id), object>`, `Map<(kind, id), object[]>`, `Map<local_id, object[]>` (the `by_local_id` index).
