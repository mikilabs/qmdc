"""Search quality tests on the real docs workspace.

This tests search quality on real data to identify issues.
Run with: pytest tests/test_docs_quality.py -v -s

Requires:
- docs workspace already indexed (run `qmdc-semantic index` first)
- Ollama running with embedding model
"""

import math
from pathlib import Path

import pytest
import yaml

# Mark as slow - requires indexed workspace
pytestmark = [pytest.mark.slow, pytest.mark.e2e]

# Path to the docs workspace (repo root / docs)
# tests/test_docs_quality.py -> qmdc-semantic/tests -> qmdc-semantic -> repo root -> docs
DOCS_PATH = Path(__file__).parent.parent.parent / "docs"
QUERIES_FILE = Path(__file__).parent / "fixtures" / "search_quality" / "queries_docs.yaml"


# ============================================================================
# Metrics
# ============================================================================


def precision_at_k(retrieved: list[str], relevant: set[str], k: int) -> float:
    if k <= 0:
        return 0.0
    top_k = retrieved[:k]
    if not top_k:
        return 0.0
    return sum(1 for r in top_k if r in relevant) / len(top_k)


def recall_at_k(retrieved: list[str], relevant: set[str], k: int) -> float:
    if not relevant:
        return 1.0
    top_k = set(retrieved[:k])
    return len(top_k & relevant) / len(relevant)


def mrr(retrieved: list[str], relevant: set[str]) -> float:
    for i, item in enumerate(retrieved, start=1):
        if item in relevant:
            return 1.0 / i
    return 0.0


def ndcg_at_k(retrieved: list[str], relevant: set[str], k: int) -> float:
    if not relevant or k <= 0:
        return 0.0

    def dcg(items: list[str]) -> float:
        score = 0.0
        for i, item in enumerate(items[:k], start=1):
            rel = 1.0 if item in relevant else 0.0
            score += rel / math.log2(i + 1)
        return score

    ideal_items = list(relevant)[:k]
    ideal_dcg = dcg(ideal_items)
    return dcg(retrieved) / ideal_dcg if ideal_dcg > 0 else 0.0


def compute_all_metrics(retrieved: list[str], relevant: set[str]) -> dict[str, float]:
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


def normalize_object_id(oid: str) -> str:
    """Normalize object_id by extracting just the __id part.

    Handles formats:
    - "workspace:namespace:id" -> "id"
    - "workspace::id" -> "id"
    - "::id" -> "id"
    - "id" -> "id"
    """
    # Split by : and take the last non-empty part
    parts = [p for p in oid.split(":") if p]
    return parts[-1] if parts else oid


# ============================================================================
# Fixtures
# ============================================================================


@pytest.fixture(scope="module")
def docs_storage():
    """Get storage for docs workspace (must be already indexed)."""
    from qmdc_semantic.config import load_config
    from qmdc_semantic.storage import Storage

    if not DOCS_PATH.exists():
        pytest.skip(f"Docs workspace not found: {DOCS_PATH}")

    storage = Storage(DOCS_PATH)
    config = load_config(DOCS_PATH)

    cursor = storage.conn.cursor()
    cursor.execute("SELECT COUNT(*) FROM chunks")
    count = cursor.fetchone()[0]

    if count == 0:
        pytest.skip("Docs workspace not indexed. Run: qmdc-semantic index ../docs")

    return storage, config


@pytest.fixture(scope="module")
def queries_data():
    """Load queries_docs.yaml test data."""
    if not QUERIES_FILE.exists():
        pytest.skip(f"Queries file not found: {QUERIES_FILE}")

    with open(QUERIES_FILE) as f:
        return yaml.safe_load(f)


@pytest.fixture(scope="module")
def all_results(docs_storage, queries_data):
    """Run all queries and collect results."""
    from qmdc_semantic.search import semantic_search

    storage, config = docs_storage
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
        retrieved = [normalize_object_id(r["object_id"]) for r in search_results]

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


def _get_query_ids():
    """Get list of query IDs for parametrization."""
    if not QUERIES_FILE.exists():
        return []
    with open(QUERIES_FILE) as f:
        data = yaml.safe_load(f)
    return [q["id"] for q in data.get("queries", [])]


def _get_expected_failures():
    """Get set of expected failure IDs."""
    if not QUERIES_FILE.exists():
        return set()
    with open(QUERIES_FILE) as f:
        data = yaml.safe_load(f)
    return set(data.get("expected_failures", []))


# ============================================================================
# Parametrized Tests - Each query is a separate test
# ============================================================================


class TestQueryQuality:
    """Test each query individually - fails if thresholds not met."""

    @pytest.mark.parametrize("query_id", _get_query_ids())
    def test_query(self, query_id, all_results, queries_data):
        """Test individual query meets its thresholds."""
        if query_id not in all_results:
            pytest.skip(f"Query {query_id} not in results")

        data = all_results[query_id]
        m = data["metrics"]
        thresholds = data["metrics_thresholds"]

        failures = []

        # Check metric thresholds
        for key, threshold in thresholds.items():
            actual = m.get(key, 0)
            if actual < threshold:
                failures.append(f"{key}: {actual:.2f} < {threshold:.2f}")

        # Check must_contain
        must = data["must_contain"]
        retrieved_set = set(data["retrieved"][:10])
        missing = must - retrieved_set
        if missing:
            failures.append(f"missing in top 10: {missing}")

        # Print debug info
        print(f"\n{query_id}: '{data['query']}'")
        print(f"  Top 5: {data['retrieved'][:5]}")
        print(f"  Expected: {data['relevant']}")
        print(
            f"  P@1={m['precision_at_1']:.2f}  P@5={m['precision_at_5']:.2f}  "
            f"R@10={m['recall_at_10']:.2f}  MRR={m['mrr']:.2f}"
        )

        if failures:
            for f in failures:
                print(f"  ⚠ {f}")

            # Check if this is an expected failure
            expected_failures = set(queries_data.get("expected_failures", []))
            if query_id in expected_failures:
                pytest.xfail(f"Expected failure: {'; '.join(failures)}")
            else:
                pytest.fail(f"Query failed: {'; '.join(failures)}")


# ============================================================================
# Aggregate Tests
# ============================================================================


class TestGlobalThresholds:
    """Test aggregate metrics meet global thresholds."""

    def test_average_mrr(self, all_results, queries_data):
        """Average MRR should meet threshold."""
        all_with_relevant = [d for d in all_results.values() if d["relevant"]]
        if not all_with_relevant:
            pytest.skip("No queries with expected results")

        avg_mrr = sum(d["metrics"]["mrr"] for d in all_with_relevant) / len(all_with_relevant)
        threshold = queries_data.get("global_thresholds", {}).get("min_avg_mrr", 0)

        print(f"\nAverage MRR: {avg_mrr:.3f} (threshold: {threshold:.2f})")
        assert avg_mrr >= threshold, f"Average MRR {avg_mrr:.3f} < {threshold:.2f}"

    def test_average_precision(self, all_results, queries_data):
        """Average P@5 should meet threshold."""
        all_with_relevant = [d for d in all_results.values() if d["relevant"]]
        if not all_with_relevant:
            pytest.skip("No queries with expected results")

        avg_p5 = sum(d["metrics"]["precision_at_5"] for d in all_with_relevant) / len(
            all_with_relevant
        )
        threshold = queries_data.get("global_thresholds", {}).get("min_avg_precision_at_5", 0)

        print(f"\nAverage P@5: {avg_p5:.3f} (threshold: {threshold:.2f})")
        assert avg_p5 >= threshold, f"Average P@5 {avg_p5:.3f} < {threshold:.2f}"


# ============================================================================
# Report (informational, always passes)
# ============================================================================


class TestDocsQualityReport:
    """Generate quality report - informational only."""

    def test_print_summary(self, all_results, queries_data):
        """Print summary of all results."""
        expected_failures = set(queries_data.get("expected_failures", []))

        passed = 0
        failed = 0
        xfailed = 0

        for qid, data in all_results.items():
            m = data["metrics"]
            thresholds = data["metrics_thresholds"]

            passes = True
            for key, threshold in thresholds.items():
                if m.get(key, 0) < threshold:
                    passes = False
                    break

            must = data["must_contain"]
            if must - set(data["retrieved"][:10]):
                passes = False

            if passes:
                passed += 1
            elif qid in expected_failures:
                xfailed += 1
            else:
                failed += 1

        all_with_relevant = [d for d in all_results.values() if d["relevant"]]
        avg_mrr = (
            sum(d["metrics"]["mrr"] for d in all_with_relevant) / len(all_with_relevant)
            if all_with_relevant
            else 0
        )
        avg_p5 = (
            sum(d["metrics"]["precision_at_5"] for d in all_with_relevant) / len(all_with_relevant)
            if all_with_relevant
            else 0
        )

        print("\n" + "=" * 60)
        print("SEARCH QUALITY SUMMARY")
        print("=" * 60)
        print(f"  Total queries: {len(all_results)}")
        print(f"  Passed: {passed}")
        print(f"  Failed: {failed}")
        print(f"  Expected failures: {xfailed}")
        print(f"  Average MRR: {avg_mrr:.3f}")
        print(f"  Average P@5: {avg_p5:.3f}")
        print("=" * 60)
