# Golden Files for Regression Tests

This directory contains baseline results for regression testing.

## Purpose

When we optimize algorithms (e.g., FTS5 vs in-memory BM25, vec0 KNN vs brute-force),
we need to verify that the quality doesn't degrade. Golden files store expected
results that new implementations must match or exceed.

## Files

- `hybrid_search_results.json` - Baseline from original test_hybrid.py
- `inferred_edges_results.json` - Baseline from original test_inferred_expansion.py

## Generating Baselines

To regenerate baselines from the original artifact scripts:

```bash
cd docs/tracking/active/QMD-41/artifacts/
python test_hybrid.py > golden/hybrid_search_results.json
python test_inferred_expansion.py > golden/inferred_edges_results.json
```

## Acceptance Criteria

| Test | Metric | Threshold |
|------|--------|-----------|
| Hybrid search | precision@10 overlap | >= 90% |
| Inferred edges | precision@50 overlap | >= 95% |

## Updating Baselines

Only update baselines when:

1. The algorithm is intentionally changed
2. The change is documented
3. Quality metrics are verified manually
