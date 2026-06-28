# Semantic Storage Schema [[semantic_storage: NarrativeDoc]]

- about: [[#algo_chunking]], [[#algo_hybrid_search]], [[#algo_inferred_edges]]

SQLite database schema for QMDC Semantic. Database is stored at `.qmdc-semantic/embeddings.db` in the workspace.

**Git LFS recommendation:**

```bash
git lfs track ".qmdc-semantic/*.db"
```

## chunks Table

- about: [[#algo_chunking]]

Stores chunk metadata and text.

```sql
CREATE TABLE chunks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    chunk_id TEXT UNIQUE NOT NULL,  -- __global_id format
    object_id TEXT NOT NULL,        -- Parent object __global_id
    object_kind TEXT,               -- Object kind (Feature, etc.)
    chunk_type TEXT,                -- parent | child | combined
    source_file TEXT,               -- Source .qmd.md file
    text TEXT,                      -- Chunk text content
    text_hash TEXT,                 -- SHA256 hash (for diff)
    model_id TEXT,                  -- provider:model identifier
    parent_chunk_id TEXT,           -- For child chunks
    embedded_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_chunks_chunk_id ON chunks(chunk_id);
CREATE INDEX idx_chunks_object_id ON chunks(object_id);
CREATE INDEX idx_chunks_text_hash ON chunks(text_hash);
CREATE INDEX idx_chunks_model_id ON chunks(model_id);
```

## vec_chunks Table

- about: [[#algo_hybrid_search]]

Vector embeddings via sqlite-vec. Multiple tables for different dimensions: 768, 1024, 1536, 3072, 4096.

```sql
CREATE VIRTUAL TABLE vec_chunks_768 USING vec0(
    chunk_id TEXT PRIMARY KEY,
    embedding float[768] distance_metric=cosine
);
```

## chunks_fts Table

- about: [[#algo_hybrid_search]]

FTS5 full-text search index. Triggers keep FTS5 in sync with the chunks table on insert, delete, and update. Indexes `chunk_id` and `object_id` alongside `text` so ID-style queries match.

```sql
CREATE VIRTUAL TABLE chunks_fts USING fts5(
    chunk_id,
    object_id,
    text,
    content=chunks,
    content_rowid=id
);
```

## chunks_trigram Table

- about: [[#algo_hybrid_search]]

FTS5 trigram index for substring matching (e.g. finds `333` inside `me333`). Kept in sync by the same triggers as `chunks_fts`.

```sql
CREATE VIRTUAL TABLE chunks_trigram USING fts5(
    chunk_id,
    text,
    content=chunks,
    content_rowid=id,
    tokenize='trigram'
);
```

## edges Table

- about: [[#algo_graph_walk]]

Explicit edges from qmdc_parser (references between objects).

```sql
CREATE TABLE edges (
    source_id TEXT NOT NULL,
    target_id TEXT NOT NULL,
    source_field TEXT,
    PRIMARY KEY (source_id, target_id, source_field)
);

CREATE INDEX idx_edges_source ON edges(source_id);
CREATE INDEX idx_edges_target ON edges(target_id);
```

## inferred_edges Table

- about: [[#algo_inferred_edges]]

Semantic similarity edges between objects.

```sql
CREATE TABLE inferred_edges (
    source_id TEXT NOT NULL,        -- __global_id format
    target_id TEXT NOT NULL,        -- __global_id format
    similarity REAL,                -- Cosine similarity 0-1
    PRIMARY KEY (source_id, target_id)
);

CREATE INDEX idx_inferred_source ON inferred_edges(source_id);
CREATE INDEX idx_inferred_target ON inferred_edges(target_id);
```

## meta Table

Key-value metadata storage. Keys include `schema_version` for database migrations (current version: 5). Migrations from v4 are incremental and non-destructive (they preserve existing embeddings); pre-v4 databases are rebuilt and require a re-index.

```sql
CREATE TABLE meta (
    key TEXT PRIMARY KEY,
    value TEXT
);
```

## ID Format

All IDs use `__global_id` format: `workspace:namespace:id`.

Examples:

- `docs::my_feature` — object in docs workspace, no namespace
- `tasks:QMD-41:qmd41_finding1` — object in tasks/QMD-41 namespace

## Incremental Updates

The `text_hash` column enables efficient incremental indexing:

1. Extract chunks from workspace
2. Compute SHA256 hash for each chunk
3. Compare with stored hashes
4. Only embed new/changed chunks
5. Delete removed chunks

## Common Queries

**KNN search:**

```sql
SELECT chunk_id, distance
FROM vec_chunks_768
WHERE embedding MATCH ? AND k = 10
ORDER BY distance;
```

**FTS5 search:**

```sql
SELECT c.chunk_id, bm25(chunks_fts) as score
FROM chunks_fts f
JOIN chunks c ON f.rowid = c.id
WHERE chunks_fts MATCH 'query'
ORDER BY score;
```

**Get neighbors (explicit + inferred, both directions):**

```sql
SELECT target_id AS neighbor, 1.0 AS weight, 'explicit' AS type
FROM edges WHERE source_id = ?
UNION
SELECT source_id AS neighbor, 1.0 AS weight, 'explicit' AS type
FROM edges WHERE target_id = ?
UNION
SELECT target_id AS neighbor, similarity AS weight, 'inferred' AS type
FROM inferred_edges WHERE source_id = ?
UNION
SELECT source_id AS neighbor, similarity AS weight, 'inferred' AS type
FROM inferred_edges WHERE target_id = ?;
```
