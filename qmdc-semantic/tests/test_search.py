"""Tests for search module."""

import pytest

from qmdc_semantic.search import _is_id_query, hybrid_fusion


@pytest.mark.unit
class TestHybridFusion:
    """Tests for hybrid fusion algorithm."""

    def test_dense_only(self):
        """Test fusion with only dense results."""
        dense_results = [
            ("doc1", 0.1),  # closest (lowest distance)
            ("doc2", 0.3),
            ("doc3", 0.5),  # farthest
        ]
        fts_results = []

        scores = hybrid_fusion(dense_results, fts_results, "test query")

        # Closest (lowest distance) should have highest score
        assert scores["doc1"] > scores["doc2"]
        assert scores["doc2"] > scores["doc3"]

    def test_fts_only(self):
        """Test fusion with only FTS results."""
        dense_results = []
        fts_results = [
            ("doc1", -10.0),  # best match (most negative BM25)
            ("doc2", -5.0),
            ("doc3", -2.0),  # worst match
        ]

        scores = hybrid_fusion(dense_results, fts_results, "test query")

        # More negative BM25 = better match = higher score
        assert scores["doc1"] > scores["doc2"]
        assert scores["doc2"] > scores["doc3"]

    def test_combined_results(self):
        """Test fusion with both dense and FTS results."""
        dense_results = [
            ("doc1", 0.1),
            ("doc2", 0.3),
        ]
        fts_results = [
            ("doc2", -10.0),  # doc2 is best FTS match
            ("doc1", -5.0),
        ]

        scores = hybrid_fusion(dense_results, fts_results, "test query")

        # Both docs should have combined scores
        assert "doc1" in scores
        assert "doc2" in scores

    def test_id_query_weights(self):
        """Test that ID queries give more weight to FTS."""
        dense_results = [
            ("doc1", 0.1),  # doc1 best dense match
            ("doc2", 0.5),
        ]
        fts_results = [
            ("doc2", -10.0),  # doc2 best FTS match
            ("doc1", -2.0),
        ]

        # Semantic query: dense gets higher weight
        scores_semantic = hybrid_fusion(dense_results, fts_results, "how to test parser")

        # ID query: FTS gets higher weight
        scores_id = hybrid_fusion(dense_results, fts_results, "QMD-17")

        # With ID query, doc2 (better FTS) should rank higher relative to doc1
        ratio_semantic = scores_semantic["doc1"] / scores_semantic["doc2"]
        ratio_id = scores_id["doc1"] / scores_id["doc2"]

        # ID query should favor FTS results more
        assert ratio_id < ratio_semantic

    def test_empty_inputs(self):
        """Test fusion with empty inputs."""
        scores = hybrid_fusion([], [], "test")
        assert scores == {}


@pytest.mark.unit
class TestIsIdQuery:
    """Tests for ID query detection."""

    def test_id_patterns(self):
        """Test various ID patterns."""
        assert _is_id_query("QMD-17") is True
        assert _is_id_query("TASK-123") is True
        assert _is_id_query("QMD17") is True
        assert _is_id_query("qmd-41") is True

    def test_non_id_queries(self):
        """Test non-ID queries."""
        assert _is_id_query("how to test parser") is False
        assert _is_id_query("testing strategy regression") is False
        assert _is_id_query("semantic search embeddings") is False

    def test_mixed_queries(self):
        """Test queries with ID and other words."""
        # More than 2 words = not ID query
        assert _is_id_query("QMD-17 task details") is False
        assert _is_id_query("find QMD-17") is True  # 2 words with ID


@pytest.mark.unit
class TestHybridFusionWithTrigram:
    """Tests for hybrid fusion with trigram results."""

    def test_trigram_boosts_results(self):
        """Test that trigram results boost matching chunks."""
        dense_results = [
            ("doc1", 0.3),
            ("doc2", 0.3),  # Same dense distance
        ]
        fts_results = []
        trigram_results = [
            ("doc2", -5.0),  # doc2 has trigram match
        ]

        scores = hybrid_fusion(dense_results, fts_results, "test", trigram_results)

        # doc2 should score higher due to trigram boost
        assert scores["doc2"] > scores["doc1"]

    def test_trigram_introduces_new_results(self):
        """Test that trigram can surface results not in dense/FTS."""
        dense_results = [
            ("doc1", 0.1),
        ]
        fts_results = []
        trigram_results = [
            ("doc3", -5.0),  # doc3 only found by trigram
        ]

        scores = hybrid_fusion(dense_results, fts_results, "test", trigram_results)

        assert "doc3" in scores
        assert scores["doc3"] > 0

    def test_no_trigram_results(self):
        """Test that None trigram results work fine."""
        dense_results = [("doc1", 0.1)]
        fts_results = []

        scores = hybrid_fusion(dense_results, fts_results, "test", None)
        assert "doc1" in scores

        scores2 = hybrid_fusion(dense_results, fts_results, "test", [])
        assert "doc1" in scores2
