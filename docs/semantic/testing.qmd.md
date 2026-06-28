# Semantic Testing [[semantic_testing: NarrativeDoc]]

- about: [[#algo_chunking]], [[#algo_hybrid_search]], [[#algo_inferred_edges]], [[#algo_graph_walk]]

Test suite for the qmdc-semantic package.

## Test Categories

| Category | Marker | Files | Description |
|----------|--------|-------|-------------|
| Unit | `unit` | test_chunking.py, test_config.py, test_search.py | Fast, isolated logic tests |
| Integration | `integration` | test_storage.py | SQLite/sqlite-vec operations |
| E2E | `slow`, `e2e` | test_e2e.py | Full pipeline with Ollama |
| Quality | `slow`, `e2e` | test_search_quality.py, test_docs_quality.py | Data-driven search quality tests (mini-workspace + real docs) |

~60 tests collected. The E2E and quality tests are provider-gated — without a configured embedding provider they are deselected (≈41 run by default).

## Running Tests

```bash
# All tests (~75s)
uv run pytest tests/ -v

# Only fast tests (<1s)
uv run pytest tests/ -v -m "not slow"

# Only slow/E2E tests (~73s)
uv run pytest tests/ -v -m slow

# Specific test file
uv run pytest tests/test_search.py -v

# Quality tests with report
uv run pytest tests/test_search_quality.py -v -s
```

## Unit Tests

### test_chunking.py

- about: [[#algo_chunking]]

| Test | Description |
|------|-------------|
| test_compute_hash | SHA256 hash computation |
| test_collect_text_fields | Field extraction from objects |
| test_short_fields_combined_chunk | Combined chunk for small objects |
| test_long_fields_create_children | Parent/child chunks for large objects |
| test_min_text_length_filter | Filtering short chunks |
| test_empty_object | Handling empty objects |
| test_chunk_ids_use_global_id | Chunk ID format with global_id |
| test_large_child_chunk_is_split | Long child chunk split at boundaries |
| test_small_child_chunk_not_split | Small child chunk left intact |
| test_split_preserves_admin_commands_section | Section boundaries preserved on split |
| test_short_text_not_split | Short text never split |
| test_sections_detected_by_colon_headers | Section detection by `Header:` lines |
| test_paragraph_fallback | Paragraph split when no sections found |

### test_search.py

- about: [[#algo_hybrid_search]]

| Test | Description |
|------|-------------|
| test_dense_only | Fusion with only dense results |
| test_fts_only | Fusion with only FTS results |
| test_combined_results | Fusion combining both |
| test_id_query_weights | Dynamic weights for ID-like queries |
| test_empty_inputs | Handling empty inputs |
| test_id_patterns | _is_id_query detection patterns |
| test_non_id_queries | Non-ID query detection |
| test_mixed_queries | Mixed query detection |
| test_trigram_boosts_results | Trigram leg boosts substring matches |
| test_trigram_introduces_new_results | Trigram surfaces results FTS/dense miss |
| test_no_trigram_results | Graceful fusion when trigram is empty |

### test_config.py

Tests for configuration loading: defaults, loading from files, priority resolution.

## Integration Tests

### test_storage.py

- about: [[#algo_chunking]], [[#algo_inferred_edges]]

Tests SQLite and sqlite-vec operations.

| Test | Description |
|------|-------------|
| test_create_storage | Database initialization |
| test_save_and_get_chunk | Chunk CRUD operations |
| test_compute_diff_* | Diff computation (new, changed, deleted) |
| test_delete_chunks | Chunk deletion |
| test_fts_search | FTS5 full-text search |
| test_save_and_get_inferred_edges | Edge storage |
| test_get_neighbors | Graph neighbor queries |
| test_meta_get_set | Metadata storage |
| test_trigram_search | Trigram substring search |
| test_trigram_search_phrase | Trigram phrase/substring matching |
| test_chunks_trigram_table_exists | Trigram FTS table present |

## E2E Tests

- about: [[#algo_hybrid_search]], [[#algo_graph_walk]]

End-to-end tests validate the full pipeline.

Requirements: Ollama running at `localhost:11434`, embedding model available (e.g., `qwen3-embedding`), config at `tests/fixtures/mini-workspace/.qmdc-semantic/config.yaml`.

| Class | Tests | Description |
|-------|-------|-------------|
| TestIndexing | 3 | Workspace indexing, embeddings, FTS creation |
| TestSearch | 4 | Search results, relevance, ID queries, graph walk |
| TestRegression | 1 | Stability check against golden files |

Mini workspace fixture in `tests/fixtures/mini-workspace/` contains sample QMD.md documents, config with Ollama settings, and golden files for regression tests.

## Data-Driven Quality Tests

- about: [[#algo_hybrid_search]]

Tests search quality using YAML-defined queries with expected results and metrics.

Test data in `tests/fixtures/search_quality/`:

- `queries.yaml` — mini-workspace queries
- `queries_docs.yaml` — real docs workspace queries

**Test data format:**

```yaml
queries:
  - id: exact_api_service
    query: "API gateway service"
    description: "Find API gateway service"
    expected:
      must_contain:
        - api_gateway
      should_contain:
        - user_service
    metrics:
      precision_at_3: 0.3
      mrr: 0.3

global_thresholds:
  min_avg_mrr: 0.3
  min_avg_precision_at_5: 0.2
```

**Implemented metrics:**

| Metric | Description |
|--------|-------------|
| Precision@K | Fraction of top-K results that are relevant |
| Recall@K | Fraction of relevant items found in top-K |
| MRR | Mean Reciprocal Rank (1/rank of first relevant) |
| NDCG@K | Normalized Discounted Cumulative Gain |

**Current baseline:** Average MRR 0.833, Average P@5 0.378, Average NDCG@5 0.806.

## Test Coverage

| Module | Tests | Coverage |
|--------|-------|----------|
| test_chunking.py | 13 | hash, fields, chunks, global_id, section splitting |
| test_config.py | 4 | defaults, loading, priority |
| test_storage.py | 13 | CRUD, FTS, trigram, diff, edges, neighbors |
| test_search.py | 11 | hybrid_fusion, trigram fusion, `_is_id_query` |
| test_e2e.py | 8 | indexing, search, regression |
| test_search_quality.py | 7 | parametrized query tests (mini-workspace) |
| test_docs_quality.py | 4 | quality metrics on the real docs workspace |
