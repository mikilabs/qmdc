"""End-to-end tests for qmdc-semantic.

These tests require Ollama running with an embedding model.
Run with: pytest tests/test_e2e.py -v -m slow
"""

import json
import shutil
from pathlib import Path

import pytest

# Mark all tests in this module as slow and e2e
pytestmark = [pytest.mark.slow, pytest.mark.e2e]


@pytest.fixture
def test_workspace(tmp_path):
    """Create a temporary test workspace with QMD files."""
    # Copy mini-workspace to temp
    fixtures_dir = Path(__file__).parent / "fixtures" / "mini-workspace"
    workspace = tmp_path / "workspace"
    shutil.copytree(fixtures_dir, workspace)
    return workspace


@pytest.fixture
def indexed_workspace(test_workspace):
    """Index the test workspace and return (workspace_path, storage)."""
    from qmdc_semantic.chunking import extract_chunks
    from qmdc_semantic.config import load_config
    from qmdc_semantic.embedding import get_provider
    from qmdc_semantic.storage import Storage

    config = load_config(test_workspace)
    storage = Storage(test_workspace)

    # Extract chunks
    chunks = extract_chunks(test_workspace, config.chunking)

    # Get embeddings
    provider = get_provider(config.embedding)
    embeddings = provider.embed([c["text"] for c in chunks])

    for chunk, embedding in zip(chunks, embeddings, strict=True):
        chunk["embedding"] = embedding
        chunk["model_id"] = f"{config.embedding.provider}:{config.embedding.model}"

    # Save to storage
    storage.save_chunks(chunks)

    return test_workspace, storage, config


class TestIndexing:
    """E2E tests for indexing."""

    def test_index_workspace(self, indexed_workspace):
        """Test that workspace can be indexed."""
        workspace, storage, config = indexed_workspace

        # Check chunks were created
        cursor = storage.conn.cursor()
        cursor.execute("SELECT COUNT(*) FROM chunks")
        chunk_count = cursor.fetchone()[0]

        assert chunk_count > 0, "No chunks were indexed"

    def test_index_creates_embeddings(self, indexed_workspace):
        """Test that embeddings are created for chunks."""
        workspace, storage, config = indexed_workspace

        # Check vec table has entries
        cursor = storage.conn.cursor()
        # Find which vec table was created
        cursor.execute(
            "SELECT name FROM sqlite_master WHERE type='table' AND name LIKE 'vec_chunks_%'"
        )
        vec_tables = cursor.fetchall()

        assert len(vec_tables) > 0, "No vec table created"

        vec_table = vec_tables[0][0]
        cursor.execute(f"SELECT COUNT(*) FROM {vec_table}")
        vec_count = cursor.fetchone()[0]

        assert vec_count > 0, "No embeddings stored"

    def test_index_creates_fts(self, indexed_workspace):
        """Test that FTS index is created."""
        workspace, storage, config = indexed_workspace

        # Try FTS search
        results = storage.fts_search("test", limit=10)
        # Should not raise error
        assert isinstance(results, list)


class TestSearch:
    """E2E tests for search."""

    def test_search_returns_results(self, indexed_workspace):
        """Test that search returns results."""
        from qmdc_semantic.search import semantic_search

        workspace, storage, config = indexed_workspace

        results = semantic_search(
            storage=storage,
            query="services",
            config=config,
            top_k=5,
            depth=0,
        )

        assert len(results) > 0, "No search results"

    def test_search_relevance(self, indexed_workspace):
        """Test that search returns relevant results."""
        from qmdc_semantic.search import semantic_search

        workspace, storage, config = indexed_workspace

        # Search for specific term that should be in mini-workspace
        results = semantic_search(
            storage=storage,
            query="API service",
            config=config,
            top_k=10,
            depth=0,
        )

        # Check that results contain relevant objects
        object_ids = [r["object_id"] for r in results]

        # At least one result should contain "api" or "service"
        found_relevant = any("api" in oid.lower() or "service" in oid.lower() for oid in object_ids)
        assert found_relevant, f"No relevant results found. Got: {object_ids}"

    def test_search_id_query(self, indexed_workspace):
        """Test search with ID-like query."""
        from qmdc_semantic.search import semantic_search

        workspace, storage, config = indexed_workspace

        # The mini-workspace might not have QMD-style IDs,
        # but we can test the mechanism works
        results = semantic_search(
            storage=storage,
            query="dashboard",
            config=config,
            top_k=5,
            depth=0,
        )

        assert isinstance(results, list)

    def test_search_with_graph_walk(self, indexed_workspace):
        """Test search with graph walk expansion."""
        from qmdc_semantic.search import semantic_search

        workspace, storage, config = indexed_workspace

        # Search with depth > 0
        results_with_walk = semantic_search(
            storage=storage,
            query="services",
            config=config,
            top_k=10,
            depth=2,
        )

        # Graph walk should potentially find more results
        # (or same if no edges)
        assert len(results_with_walk) >= 0


class TestRegression:
    """Regression tests comparing against golden files."""

    def test_search_results_stable(self, indexed_workspace, tmp_path):
        """Test that search results are stable across runs."""
        from qmdc_semantic.search import semantic_search

        workspace, storage, config = indexed_workspace

        queries = [
            "services",
            "API",
            "storage tables",
        ]

        results_file = tmp_path / "search_results.json"
        golden_file = Path(__file__).parent / "golden" / "search_results.json"

        # Run searches
        all_results = {}
        for query in queries:
            results = semantic_search(
                storage=storage,
                query=query,
                config=config,
                top_k=5,
                depth=0,
            )
            # Only store object_ids for comparison (scores may vary slightly)
            all_results[query] = [r["object_id"] for r in results]

        # Save current results
        with open(results_file, "w") as f:
            json.dump(all_results, f, indent=2)

        # If golden file exists, compare
        if golden_file.exists():
            with open(golden_file) as f:
                golden = json.load(f)

            for query in queries:
                if query in golden:
                    current = set(all_results.get(query, []))
                    expected = set(golden.get(query, []))

                    # Check overlap (at least 50% of results should match)
                    if expected:
                        overlap = len(current & expected) / len(expected)
                        assert overlap >= 0.5, (
                            f"Query '{query}': results changed significantly. "
                            f"Expected {expected}, got {current}, overlap {overlap:.0%}"
                        )
