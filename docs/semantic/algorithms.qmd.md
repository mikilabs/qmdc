# Semantic Algorithms [[semantic_algorithms]]

Algorithms used in the QMDC Semantic search pipeline.

## Chunking [[algo_chunking: Algorithm]]

Hierarchical chunking algorithm for QMD.md objects.

- source_file: qmdc-semantic/qmdc_semantic/chunking.py
- complexity: O(n) where n = number of objects
- used_in: [[#semantic_commands.cmd_sem_index]]

### Description [[description: text]]

Splits QMD.md objects into chunks suitable for embedding and retrieval.

**Algorithm:**

1. Compute `__global_id` for each object (format: `workspace:namespace:id`)
2. Collect text fields (excluding `__*` metadata fields)
3. Build header with kind, label, ID variants for FTS matching
4. Split fields by length threshold:
   - **Long fields**: >= `long_field_threshold` (default: 50 chars)
   - **Short fields**: < `long_field_threshold`
5. If long fields exist:
   - Each long field → separate **child chunk** with namespace context
   - A child chunk longer than `max_chunk_size` (default: 3000 chars) is split at section/paragraph boundaries into multiple child chunks
   - Create **parent chunk** with header + short fields summary
6. If only short fields:
   - Create single **combined chunk** with all content
7. Filter: minimum `min_text_length` (default: 30 chars)

**ID variants for FTS:** For better ID search, chunk text includes variants — `qmd41` → `qmd41 QMD-41 QMDC 41`. This allows FTS to match both `QMD-41` and `qmd41` queries.

**Chunk types:**

| Type | Description | Content |
|------|-------------|---------|
| `parent` | Summary of object | kind, label, ID variants, short fields |
| `child` | Long field content | namespace context + field_name: field_value |
| `combined` | Small object | kind, label, ID variants, all fields |

**Comments integration:** Chunking includes `__comments` content. Parser `__comments` fields are merged into chunk text based on their `after` anchor — each comment block is appended to the field it follows. Comments without an anchor are added to the object header.

## Hybrid Search [[algo_hybrid_search: Algorithm]]

Combines keyword and semantic search with dynamic weight fusion.

- source_file: qmdc-semantic/qmdc_semantic/search.py
- complexity: O(k log n) where k = top-K, n = total chunks
- used_in: [[#semantic_commands.cmd_sem_search]]

### Description [[description: text]]

**Algorithm:**

1. **Embed query**: Get vector embedding via provider (Ollama/OpenRouter)
2. **Dense search**: KNN via sqlite-vec → top K×2 chunks by cosine distance
3. **FTS5 search**: Keyword search via SQLite FTS5 → top K×2 chunks by BM25 (the query is normalized so `QMD-17` matches both `qmd17` and `QMD 17`)
4. **Trigram search**: Substring match via the FTS5 trigram table → top K×2 chunks (finds e.g. `333` inside `me333`)
5. **Dynamic fusion**: Combine the three legs with query-type-aware weights
6. **Group by object**: Aggregate chunk scores per object with count boost
7. **Graph walk**: Expand via explicit/inferred edges (seeds = K/2)
8. **Rerank**: Score graph walk results by query similarity (0.8× discount)
9. **Return top-K objects**

**Dynamic weights** adapt based on query type detection:

| Query Type | Dense | FTS | Trigram | Detection |
|------------|-------|-----|---------|-----------|
| ID query | 0.3 | 0.7 | 0.4 | Pattern `[A-Za-z]+-?\d+`, ≤2 words |
| Semantic | 0.7 | 0.3 | 0.2 | Everything else |

Examples: `QMD-41` → ID query (FTS weight 0.7); `how to test LSP` → Semantic (Dense weight 0.7).

**Score normalization:**

```python
# Dense: distance → similarity (0-1, higher = better)
similarity = 1.0 - (distance - min_dist) / dist_range

# FTS: BM25 (more negative = better) → normalized (0-1, min-max)
norm_fts = (abs(score) - min_fts) / fts_range

# Trigram: BM25 → normalized by max-abs (a single substring hit still boosts)
norm_tri = abs(score) / max_tri

# Final fusion
final = dense_weight * similarity + fts_weight * norm_fts + trigram_weight * norm_tri
```

**Group by object** — objects with multiple matching chunks get boosted:

```python
count_boost = 1 + 0.1 * log(1 + chunk_count)  # Diminishing returns
avg_factor = 0.5 + 0.5 * (avg_score / max_score)  # Consistency bonus
object_score = max_score * count_boost * avg_factor
```

## Inferred Edges [[algo_inferred_edges: Algorithm]]

Computes semantic similarity edges between objects.

- source_file: qmdc-semantic/qmdc_semantic/inferred.py
- complexity: O(n × k × log n) via sqlite-vec KNN
- used_in: [[#semantic_commands.cmd_sem_index]]

### Description [[description: text]]

**Algorithm:**

1. For each object, get representative chunk embedding
2. KNN search to find top-K similar chunks (default K=50)
3. Aggregate by object pair (avoid duplicates)
4. Filter by similarity threshold (default: 0.7)
5. Store edges in `inferred_edges` table (exclude self-references)

Inferred edges are used in graph walk to expand search results beyond explicit `[[#ref]]` links from the QMDC parser.

## Graph Walk [[algo_graph_walk: Algorithm]]

Expands search results through graph traversal with reranking.

- source_file: qmdc-semantic/qmdc_semantic/search.py
- complexity: O(seeds × branching^depth)
- used_in: [[#semantic_commands.cmd_sem_search]]

### Description [[description: text]]

**Algorithm:**

1. Get top K/2 objects from hybrid search as **seeds**
2. BFS traversal to depth D (default: 2):
   - Follow **explicit edges**: `[[#ref]]` links from parser
   - Follow **inferred edges**: Semantic similarity > threshold
3. Rerank discovered objects:
   - Initial results: keep hybrid fusion scores
   - Graph walk results: KNN similarity × 0.8 discount (prefer direct matches)
4. Sort by score, return top-K

**Edge sources:**

| Type | Source | Weight | Table |
|------|--------|--------|-------|
| Explicit | `[[#ref]]` in QMD.md | 1.0 | `edges` |
| Inferred | Semantic similarity | 0.7–1.0 | `inferred_edges` |

**Reranking** — objects found via graph walk (not in initial results) are scored as `similarity * 0.8`. The discount ensures direct search matches rank higher than objects found only through graph traversal.

**Parameters:**

| Parameter | Default | Description |
|-----------|---------|-------------|
| seeds | top-K/2 | Number of seed objects for graph walk |
| depth | 2 | Max BFS traversal depth |
| discount | 0.8 | Score multiplier for graph walk results |
