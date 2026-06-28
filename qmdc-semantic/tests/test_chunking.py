"""Tests for chunking module."""

import pytest

from qmdc_semantic.chunking import (
    _collect_text_fields,
    _compute_hash,
    extract_object_chunks,
)
from qmdc_semantic.config import ChunkingConfig


@pytest.mark.unit
class TestChunking:
    """Tests for chunking algorithm."""

    def test_compute_hash(self):
        """Test hash computation."""
        hash1 = _compute_hash("test text")
        hash2 = _compute_hash("test text")
        hash3 = _compute_hash("different text")

        assert hash1 == hash2
        assert hash1 != hash3
        assert len(hash1) == 16

    def test_collect_text_fields(self):
        """Test collecting text fields from object."""
        obj = {
            "__id": "test",
            "__kind": "Feature",
            "name": "Test Feature",
            "description": "A test",
            "count": 123,  # non-string
        }

        fields = _collect_text_fields(obj)
        assert "name" in fields
        assert "description" in fields
        assert "__id" not in fields
        assert "count" not in fields

    def test_short_fields_combined_chunk(self):
        """Test that short fields create a combined chunk."""
        obj = {
            "__id": "test",
            "__kind": "Feature",
            "__label": "Test Feature",
            "__file": "test.qmd.md",
            "__global_id": "::test",
            "status": "planned",
            "priority": "high",
        }

        config = ChunkingConfig(min_text_length=10, long_field_threshold=50)
        chunks = extract_object_chunks(obj, config)

        assert len(chunks) == 1
        assert chunks[0]["chunk_type"] == "combined"
        assert "Feature: Test Feature" in chunks[0]["text"]
        assert "status: planned" in chunks[0]["text"]

    def test_long_fields_create_children(self):
        """Test that long fields create parent + child chunks."""
        long_text = "This is a very long description that exceeds the threshold. " * 3
        obj = {
            "__id": "test",
            "__kind": "Feature",
            "__label": "Test Feature",
            "__file": "test.qmd.md",
            "__global_id": "::test",
            "description": long_text,
            "status": "planned",
        }

        config = ChunkingConfig(min_text_length=10, long_field_threshold=50)
        chunks = extract_object_chunks(obj, config)

        assert len(chunks) == 2
        chunk_types = {c["chunk_type"] for c in chunks}
        assert "parent" in chunk_types
        assert "child" in chunk_types

        # Parent chunk should have short fields
        parent = next(c for c in chunks if c["chunk_type"] == "parent")
        assert "status: planned" in parent["text"]

        # Child chunk should have long field
        child = next(c for c in chunks if c["chunk_type"] == "child")
        assert "description:" in child["text"]
        assert long_text in child["text"]

    def test_min_text_length_filter(self):
        """Test that chunks below min_text_length are filtered."""
        obj = {
            "__id": "t",
            "__kind": "X",
            "__label": "Y",
            "__file": "test.qmd.md",
            "__global_id": "::t",
            "a": "b",
        }

        config = ChunkingConfig(min_text_length=100, long_field_threshold=50)
        chunks = extract_object_chunks(obj, config)

        # Should be empty because combined text is too short
        assert len(chunks) == 0

    def test_empty_object(self):
        """Test chunking an object with no text fields."""
        obj = {
            "__id": "test",
            "__kind": "Feature",
        }

        config = ChunkingConfig()
        chunks = extract_object_chunks(obj, config)

        assert len(chunks) == 0

    def test_chunk_ids_use_global_id(self):
        """Test that chunk IDs use __global_id format."""
        obj = {
            "__id": "test",
            "__kind": "Feature",
            "__label": "Test",
            "__file": "test.qmd.md",
            "__global_id": "workspace:namespace:test",
            "status": "planned",
        }

        config = ChunkingConfig()
        chunks = extract_object_chunks(obj, config)

        assert len(chunks) == 1
        assert chunks[0]["chunk_id"] == "workspace:namespace:test"
        assert chunks[0]["object_id"] == "workspace:namespace:test"


@pytest.mark.unit
class TestSectionSplitting:
    """Tests for section-aware chunk splitting."""

    def test_large_child_chunk_is_split(self):
        """Test that a child chunk exceeding max_chunk_size is split into sections."""
        # Create text with clear section headers
        sections = []
        for i in range(5):
            section = f"Section {i} heading:\n\n" + f"Content for section {i}. " * 80
            sections.append(section)
        long_text = "\n\n".join(sections)

        obj = {
            "__id": "test",
            "__kind": "Module",
            "__label": "Test Module",
            "__file": "test.qmd.md",
            "__global_id": "ws:ns:test",
            "spec": long_text,
            "status": "active",
        }

        config = ChunkingConfig(min_text_length=10, long_field_threshold=50, max_chunk_size=1000)
        chunks = extract_object_chunks(obj, config)

        # Should have multiple child chunks (split) + parent
        child_chunks = [c for c in chunks if c["chunk_type"] == "child"]
        assert len(child_chunks) > 1, f"Expected multiple child chunks, got {len(child_chunks)}"

        # All child chunks should reference the same object
        for chunk in child_chunks:
            assert chunk["object_id"] == "ws:ns:test"
            assert chunk["parent_chunk_id"] == "ws:ns:test"

    def test_small_child_chunk_not_split(self):
        """Test that a child chunk under max_chunk_size is not split."""
        text = "Short description that fits in one chunk. " * 5

        obj = {
            "__id": "test",
            "__kind": "Module",
            "__label": "Test Module",
            "__file": "test.qmd.md",
            "__global_id": "ws:ns:test",
            "description": text,
            "status": "active",
        }

        config = ChunkingConfig(min_text_length=10, long_field_threshold=50, max_chunk_size=3000)
        chunks = extract_object_chunks(obj, config)

        child_chunks = [c for c in chunks if c["chunk_type"] == "child"]
        assert len(child_chunks) == 1

    def test_split_preserves_admin_commands_section(self):
        """Test that section splitting correctly isolates an 'Admin commands' section."""
        text = (
            """What it does:

- Runs as a long-polling application
- Handles messages in forum topics

Threading model:

- Each topic is a separate conversation
- Messages are processed sequentially
- """
            + "More threading details. " * 50
            + """

Bot commands:

- /start — welcome message
- /help — shows help
- /usage — shows usage stats

Admin/debug commands:

- /me333 — shows account info
- /delme333 — deletes user
- /setbalance333 — sets balance

Error handling:

- Errors are logged and reported
- """
            + "More error handling details. " * 50
        )

        obj = {
            "__id": "telegram_bot",
            "__kind": "Module",
            "__label": "Telegram Bot",
            "__file": "modules/telegram-bot.qmd.md",
            "__global_id": "ws:arch:telegram_bot",
            "target_spec": text,
            "status": "active",
        }

        config = ChunkingConfig(min_text_length=10, long_field_threshold=50, max_chunk_size=800)
        chunks = extract_object_chunks(obj, config)

        child_chunks = [c for c in chunks if c["chunk_type"] == "child"]

        # Find the chunk containing admin commands
        admin_chunks = [c for c in child_chunks if "333" in c["text"]]
        assert len(admin_chunks) >= 1, "Should have a chunk containing admin commands with 333"

        # The admin chunk should be focused (not the entire 13K text)
        for chunk in admin_chunks:
            assert len(chunk["text"]) < 1500, (
                f"Admin chunk should be focused, got {len(chunk['text'])} chars"
            )


@pytest.mark.unit
class TestSplitTextBySections:
    """Tests for _split_text_by_sections helper."""

    def test_short_text_not_split(self):
        """Text under max_size returns as single section."""
        from qmdc_semantic.chunking import _split_text_by_sections

        text = "Short text that doesn't need splitting."
        result = _split_text_by_sections(text, 3000)
        assert len(result) == 1
        assert result[0][1] == text

    def test_sections_detected_by_colon_headers(self):
        """Detects 'Section Name:' style headers."""
        from qmdc_semantic.chunking import _split_text_by_sections

        text = (
            "Preamble text here.\n\n"
            + "A" * 500
            + "\n\nFirst Section:\n\n"
            + "B" * 500
            + "\n\nSecond Section:\n\n"
            + "C" * 500
        )
        result = _split_text_by_sections(text, 400)
        assert len(result) > 1

    def test_paragraph_fallback(self):
        """Falls back to paragraph splitting when no sections detected."""
        from qmdc_semantic.chunking import _split_text_by_sections

        # Text with no section headers, just paragraphs
        paragraphs = ["Paragraph " + str(i) + ". " * 50 for i in range(10)]
        text = "\n\n".join(paragraphs)
        result = _split_text_by_sections(text, 500)
        assert len(result) > 1
