"""Tests for storage module."""

import pytest

from qmdc_semantic.storage import Storage


@pytest.mark.integration
class TestStorage:
    """Tests for SQLite storage."""

    def test_create_storage(self, tmp_path):
        """Test creating storage initializes schema."""
        workspace = tmp_path / "workspace"
        workspace.mkdir()

        storage = Storage(workspace)

        # Check that tables exist
        cursor = storage.conn.cursor()
        cursor.execute("SELECT name FROM sqlite_master WHERE type='table'")
        tables = {row[0] for row in cursor.fetchall()}

        assert "chunks" in tables
        assert "meta" in tables
        assert "inferred_edges" in tables
        assert "chunks_fts" in tables

        storage.close()

    def test_save_and_get_chunk(self, tmp_path):
        """Test saving and retrieving a chunk."""
        workspace = tmp_path / "workspace"
        workspace.mkdir()

        storage = Storage(workspace)

        chunk = {
            "chunk_id": "test_chunk",
            "object_id": "::test",
            "object_kind": "Feature",
            "chunk_type": "combined",
            "source_file": "test.qmd.md",
            "text": "Test chunk text",
            "text_hash": "abc123",
        }

        storage.save_chunks([chunk])

        retrieved = storage.get_chunk("test_chunk")
        assert retrieved is not None
        assert retrieved["chunk_id"] == "test_chunk"
        assert retrieved["text"] == "Test chunk text"

        storage.close()

    def test_compute_diff_new(self, tmp_path):
        """Test diff computation for new chunks."""
        workspace = tmp_path / "workspace"
        workspace.mkdir()

        storage = Storage(workspace)

        chunks = [
            {"chunk_id": "c1", "text_hash": "h1"},
            {"chunk_id": "c2", "text_hash": "h2"},
        ]

        diff = storage.compute_diff(chunks)

        assert len(diff["new"]) == 2
        assert len(diff["changed"]) == 0
        assert len(diff["unchanged"]) == 0
        assert len(diff["deleted"]) == 0

        storage.close()

    def test_compute_diff_changed(self, tmp_path):
        """Test diff computation for changed chunks."""
        workspace = tmp_path / "workspace"
        workspace.mkdir()

        storage = Storage(workspace)

        # Save initial chunk
        storage.save_chunks(
            [
                {
                    "chunk_id": "c1",
                    "object_id": "::test",
                    "text": "original",
                    "text_hash": "h1",
                }
            ]
        )

        # Compute diff with changed hash
        chunks = [{"chunk_id": "c1", "text_hash": "h2"}]
        diff = storage.compute_diff(chunks)

        assert len(diff["new"]) == 0
        assert len(diff["changed"]) == 1
        assert len(diff["unchanged"]) == 0

        storage.close()

    def test_compute_diff_deleted(self, tmp_path):
        """Test diff computation for deleted chunks."""
        workspace = tmp_path / "workspace"
        workspace.mkdir()

        storage = Storage(workspace)

        # Save initial chunk
        storage.save_chunks(
            [
                {
                    "chunk_id": "c1",
                    "object_id": "::test",
                    "text": "text",
                    "text_hash": "h1",
                }
            ]
        )

        # Compute diff with empty workspace
        diff = storage.compute_diff([])

        assert len(diff["new"]) == 0
        assert len(diff["deleted"]) == 1
        assert diff["deleted"][0]["chunk_id"] == "c1"

        storage.close()

    def test_delete_chunks(self, tmp_path):
        """Test deleting chunks."""
        workspace = tmp_path / "workspace"
        workspace.mkdir()

        storage = Storage(workspace)

        storage.save_chunks(
            [
                {
                    "chunk_id": "c1",
                    "object_id": "::test",
                    "text": "text",
                    "text_hash": "h1",
                }
            ]
        )

        assert storage.get_chunk("c1") is not None

        storage.delete_chunks(["c1"])

        assert storage.get_chunk("c1") is None

        storage.close()

    def test_fts_search(self, tmp_path):
        """Test FTS5 search."""
        workspace = tmp_path / "workspace"
        workspace.mkdir()

        storage = Storage(workspace)

        storage.save_chunks(
            [
                {
                    "chunk_id": "c1",
                    "object_id": "::test1",
                    "text": "Python programming language",
                    "text_hash": "h1",
                },
                {
                    "chunk_id": "c2",
                    "object_id": "::test2",
                    "text": "JavaScript web development",
                    "text_hash": "h2",
                },
            ]
        )

        results = storage.fts_search("Python")
        assert len(results) == 1
        assert results[0][0] == "c1"

        storage.close()

    def test_save_and_get_inferred_edges(self, tmp_path):
        """Test saving and retrieving inferred edges."""
        workspace = tmp_path / "workspace"
        workspace.mkdir()

        storage = Storage(workspace)

        edges = [
            ("::obj1", "::obj2", 0.85),
            ("::obj1", "::obj3", 0.72),
            ("::obj2", "::obj3", 0.65),  # Below threshold
        ]

        storage.save_inferred_edges(edges)

        # Get edges above threshold
        retrieved = storage.get_inferred_edges(threshold=0.7)
        assert len(retrieved) == 2

        # Check that low similarity edge is filtered
        ids = {(e[0], e[1]) for e in retrieved}
        assert ("::obj2", "::obj3") not in ids

        storage.close()

    def test_get_neighbors(self, tmp_path):
        """Test getting neighbors of an object."""
        workspace = tmp_path / "workspace"
        workspace.mkdir()

        storage = Storage(workspace)

        storage.save_inferred_edges(
            [
                ("::obj1", "::obj2", 0.85),
                ("::obj3", "::obj1", 0.75),
            ]
        )

        neighbors = storage.get_neighbors("::obj1")

        # Should find both (bidirectional)
        neighbor_ids = {n[0] for n in neighbors}
        assert "::obj2" in neighbor_ids
        assert "::obj3" in neighbor_ids

        storage.close()

    def test_meta_get_set(self, tmp_path):
        """Test metadata get/set."""
        workspace = tmp_path / "workspace"
        workspace.mkdir()

        storage = Storage(workspace)

        storage.set_meta("test_key", "test_value")
        assert storage.get_meta("test_key") == "test_value"
        assert storage.get_meta("nonexistent") is None

        storage.close()

    def test_trigram_search(self, tmp_path):
        """Test trigram substring search finds tokens within words."""
        workspace = tmp_path / "workspace"
        workspace.mkdir()

        storage = Storage(workspace)

        storage.save_chunks(
            [
                {
                    "chunk_id": "c1",
                    "object_id": "::test1",
                    "text": "/me333 shows account info and /delme333 deletes user",
                    "text_hash": "h1",
                },
                {
                    "chunk_id": "c2",
                    "object_id": "::test2",
                    "text": "JavaScript web development with React",
                    "text_hash": "h2",
                },
            ]
        )

        # Trigram should find "333" as substring within "me333" and "delme333"
        results = storage.trigram_search("333")
        assert len(results) == 1
        assert results[0][0] == "c1"

        # Regular FTS would NOT find "333" since it's not a standalone token
        fts_results = storage.fts_search("333")
        assert len(fts_results) == 0

        storage.close()

    def test_trigram_search_phrase(self, tmp_path):
        """Test trigram search with multi-word phrases."""
        workspace = tmp_path / "workspace"
        workspace.mkdir()

        storage = Storage(workspace)

        storage.save_chunks(
            [
                {
                    "chunk_id": "c1",
                    "object_id": "::test1",
                    "text": "The admin_commands module handles privileged operations",
                    "text_hash": "h1",
                },
                {
                    "chunk_id": "c2",
                    "object_id": "::test2",
                    "text": "Regular user commands for the bot",
                    "text_hash": "h2",
                },
            ]
        )

        # Should find substring "admin_command" within "admin_commands"
        results = storage.trigram_search("admin_command")
        assert len(results) >= 1
        assert results[0][0] == "c1"

        storage.close()

    def test_chunks_trigram_table_exists(self, tmp_path):
        """Test that the trigram FTS table is created."""
        workspace = tmp_path / "workspace"
        workspace.mkdir()

        storage = Storage(workspace)

        cursor = storage.conn.cursor()
        cursor.execute("SELECT name FROM sqlite_master WHERE type='table'")
        tables = {row[0] for row in cursor.fetchall()}

        assert "chunks_trigram" in tables

        storage.close()
