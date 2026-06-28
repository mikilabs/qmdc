"""Data-driven search quality tests.

Tests search quality using queries.yaml with expected results and metrics.
Run with: pytest tests/test_search_quality.py -v -m slow
"""

import math
import shutil
from pathlib import Path

import pytest
import yaml

# Mark all tests as slow/e2e
pytestmark = [pytest.mark.slow, pytest.mark.e2e]


# ============================================================================
# Metrics
# ============================================================================


def precision_at_k(retrieved: list[str], relevant: set[str], k: int) -> float:
    """Precision@K: fraction of top-K results that are relevant."""
    if k <= 0:
        return 0.0
    top_k = retrieved[:k]
    if not top_k:
        return 0.0
    relevant_in_top_k = sum(1 for r in top_k if r in relevant)
    return relevant_in_top_k / len(top_k)


def recall_at_k(retrieved: list[str], relevant: set[str], k: int) -> float:
    """Recall@K: fraction of relevant items found in top-K."""
    if not relevant:
        return 1.0  # No relevant items = perfect recall
    top_k = set(retrieved[:k])
    found = len(top_k & relevant)
    return found / len(relevant)


def mrr(retrieved: list[str], relevant: set[str]) -> float:
    """Mean Reciprocal Rank: 1/rank of first relevant result."""
    for i, item in enumerate(retrieved, start=1):
        if item in relevant:
            return 1.0 / i
    return 0.0


def ndcg_at_k(retrieved: list[str], relevant: set[str], k: int) -> float:
    """Normalized Discounted Cumulative Gain at K."""
    if not relevant or k <= 0:
        return 0.0

    def dcg(items: list[str]) -> float:
        score = 0.0
        for i, item in enumerate(items[:k], start=1):
            rel = 1.0 if item in relevant else 0.0
            score += rel / math.log2(i + 1)
        return score

    # Ideal DCG: all relevant items at top
    ideal_items = list(relevant)[:k]
    ideal_dcg = dcg(ideal_items)

    if ideal_dcg == 0:
        return 0.0

    return dcg(retrieved) / ideal_dcg


def compute_all_metrics(retrieved: list[str], relevant: set[str]) -> dict[str, float]:
    """Compute all metrics for a single query."""
    return {
        "precision_at_1": precision_at_k(retrieved, relevant, 1),
        "precision_at_3": precision_at_k(retrieved, relevant, 3),
        "precision_at_5": precision_at_k(retrieved, relevant, 5),
        "precision_at_10": precision_at_k(retrieved, relevant, 10),
        "recall_at_5": recall_at_k(retrieved, relevant, 5),
        "recall_at_10": recall_at_k(retrieved, relevant, 10),
        "mrr": mrr(retrieved, relevant),
        "ndcg_at_5": ndcg_at_k(retrieved, relevant, 5),
        "ndcg_at_10": ndcg_at_k(retrieved, relevant, 10),
    }


# ============================================================================
# Fixtures
# ============================================================================


@pytest.fixture(scope="module")
def quality_workspace(tmp_path_factory):
    """Create indexed workspace for quality tests."""
    from qmdc_semantic.chunking import extract_chunks
    from qmdc_semantic.config import load_config
    from qmdc_semantic.embedding import get_provider
    from qmdc_semantic.storage import Storage

    tmp_path = tmp_path_factory.mktemp("quality")
    fixtures_dir = Path(__file__).parent / "fixtures" / "mini-workspace"
    workspace = tmp_path / "workspace"
    shutil.copytree(fixtures_dir, workspace)

    config = load_config(workspace)
    storage = Storage(workspace)

    chunks = extract_chunks(workspace, config.chunking)
    provider = get_provider(config.embedding)
    embeddings = provider.embed([c["text"] for c in chunks])

    for chunk, embedding in zip(chunks, embeddings, strict=True):
        chunk["embedding"] = embedding
        chunk["model_id"] = f"{config.embedding.provider}:{config.embedding.model}"

    storage.save_chunks(chunks)

    return workspace, storage, config


@pytest.fixture(scope="module")
def queries_data():
    """Load queries.yaml test data."""
    queries_file = Path(__file__).parent / "fixtures" / "search_quality" / "queries.yaml"
    with open(queries_file) as f:
        return yaml.safe_load(f)


def normalize_object_id(oid: str) -> str:
    """Normalize object_id by extracting just the __id part.

    Handles formats:
    - "workspace:namespace:id" -> "id"
    - "workspace::id" -> "id"
    - "::id" -> "id"
    - "id" -> "id"
    """
    parts = [p for p in oid.split(":") if p]
    return parts[-1] if parts else oid


@pytest.fixture(scope="module")
def all_results(quality_workspace, queries_data):
    """Run all queries and collect results."""
    from qmdc_semantic.search import semantic_search

    workspace, storage, config = quality_workspace
    queries = queries_data["queries"]

    results = {}
    for q in queries:
        search_results = semantic_search(
            storage=storage,
            query=q["query"],
            config=config,
            top_k=20,
            depth=0,
        )
        # Normalize object_ids (remove :: prefix)
        retrieved = [normalize_object_id(r["object_id"]) for r in search_results]

        # Combine must_contain and should_contain for relevance
        expected = q.get("expected", {})
        relevant = set(expected.get("must_contain", []) + expected.get("should_contain", []))

        results[q["id"]] = {
            "query": q["query"],
            "description": q.get("description", ""),
            "retrieved": retrieved,
            "relevant": relevant,
            "must_contain": set(expected.get("must_contain", [])),
            "metrics_thresholds": q.get("metrics", {}),
            "metrics": compute_all_metrics(retrieved, relevant),
        }

    return results


# ============================================================================
# Tests
# ============================================================================


class TestSearchQualityMetrics:
    """Test individual query metrics against thresholds."""

    def test_must_contain_objects(self, all_results):
        """Test that must_contain objects appear in results."""
        failures = []

        for qid, data in all_results.items():
            must = data["must_contain"]
            retrieved = set(data["retrieved"][:10])  # Check top 10

            missing = must - retrieved
            if missing:
                failures.append(f"{qid}: missing {missing} in top 10. Got: {data['retrieved'][:5]}")

        if failures:
            pytest.fail("\n".join(failures))

    def test_precision_thresholds(self, all_results):
        """Test precision@K meets thresholds."""
        failures = []

        for qid, data in all_results.items():
            thresholds = data["metrics_thresholds"]
            metrics = data["metrics"]

            for k in [1, 3, 5, 10]:
                key = f"precision_at_{k}"
                if key in thresholds:
                    expected = thresholds[key]
                    actual = metrics[key]
                    if actual < expected:
                        failures.append(f"{qid}: {key} = {actual:.2f} < {expected:.2f}")

        if failures:
            pytest.fail("\n".join(failures))

    def test_recall_thresholds(self, all_results):
        """Test recall@K meets thresholds."""
        failures = []

        for qid, data in all_results.items():
            thresholds = data["metrics_thresholds"]
            metrics = data["metrics"]

            for k in [5, 10]:
                key = f"recall_at_{k}"
                if key in thresholds:
                    expected = thresholds[key]
                    actual = metrics[key]
                    if actual < expected:
                        failures.append(f"{qid}: {key} = {actual:.2f} < {expected:.2f}")

        if failures:
            pytest.fail("\n".join(failures))

    def test_mrr_thresholds(self, all_results):
        """Test MRR meets thresholds."""
        failures = []

        for qid, data in all_results.items():
            thresholds = data["metrics_thresholds"]
            metrics = data["metrics"]

            if "mrr" in thresholds:
                expected = thresholds["mrr"]
                actual = metrics["mrr"]
                if actual < expected:
                    failures.append(f"{qid}: MRR = {actual:.2f} < {expected:.2f}")

        if failures:
            pytest.fail("\n".join(failures))


class TestGlobalQuality:
    """Test aggregate quality metrics."""

    def test_average_mrr(self, all_results, queries_data):
        """Test average MRR across all queries."""
        thresholds = queries_data.get("global_thresholds", {})
        min_mrr = thresholds.get("min_avg_mrr", 0.0)

        # Only count queries with relevant items
        mrr_values = [data["metrics"]["mrr"] for data in all_results.values() if data["relevant"]]

        if mrr_values:
            avg_mrr = sum(mrr_values) / len(mrr_values)
            assert avg_mrr >= min_mrr, f"Average MRR {avg_mrr:.2f} < {min_mrr:.2f}"

    def test_average_precision(self, all_results, queries_data):
        """Test average precision@5 across all queries."""
        thresholds = queries_data.get("global_thresholds", {})
        min_precision = thresholds.get("min_avg_precision_at_5", 0.0)

        precision_values = [
            data["metrics"]["precision_at_5"] for data in all_results.values() if data["relevant"]
        ]

        if precision_values:
            avg_precision = sum(precision_values) / len(precision_values)
            assert avg_precision >= min_precision, (
                f"Average Precision@5 {avg_precision:.2f} < {min_precision:.2f}"
            )


class TestMetricsReport:
    """Generate quality report (always passes, for visibility)."""

    def test_print_metrics_report(self, all_results):
        """Print detailed metrics report."""
        print("\n" + "=" * 70)
        print("SEARCH QUALITY REPORT")
        print("=" * 70)

        # Per-query metrics
        for qid, data in all_results.items():
            print(f"\n{qid}: {data['description']}")
            print(f"  Query: '{data['query']}'")
            print(f"  Top 5: {data['retrieved'][:5]}")

            m = data["metrics"]
            print(
                f"  P@1={m['precision_at_1']:.2f}  P@5={m['precision_at_5']:.2f}  "
                f"R@10={m['recall_at_10']:.2f}  MRR={m['mrr']:.2f}  "
                f"NDCG@5={m['ndcg_at_5']:.2f}"
            )

        # Aggregate
        print("\n" + "-" * 70)
        print("AGGREGATE METRICS")

        all_with_relevant = [d for d in all_results.values() if d["relevant"]]
        if all_with_relevant:
            avg_mrr = sum(d["metrics"]["mrr"] for d in all_with_relevant) / len(all_with_relevant)
            avg_p5 = sum(d["metrics"]["precision_at_5"] for d in all_with_relevant) / len(
                all_with_relevant
            )
            avg_ndcg = sum(d["metrics"]["ndcg_at_5"] for d in all_with_relevant) / len(
                all_with_relevant
            )

            print(f"  Queries with relevant items: {len(all_with_relevant)}")
            print(f"  Average MRR: {avg_mrr:.3f}")
            print(f"  Average P@5: {avg_p5:.3f}")
            print(f"  Average NDCG@5: {avg_ndcg:.3f}")

        print("=" * 70)
