"""Unit tests for qmdc_mkdocs.hints module."""

import json

import pytest

from qmdc_mkdocs.hints import HintEntry, get_page_hints, load_hints


class TestLoadHints:
    """Tests for load_hints function."""

    def test_missing_file_returns_empty_dict(self, tmp_path):
        """When hints.json does not exist, load_hints returns empty dict."""
        result = load_hints(tmp_path)
        assert result == {}

    def test_missing_qmdc_semantic_dir_returns_empty_dict(self, tmp_path):
        """When .qmdc-semantic/ directory doesn't exist, returns empty dict."""
        result = load_hints(tmp_path)
        assert result == {}

    def test_valid_json_returns_parsed_hint_entries(self, tmp_path):
        """Valid hints.json is parsed into dict of HintEntry lists."""
        semantic_dir = tmp_path / ".qmdc-semantic"
        semantic_dir.mkdir()
        hints_data = {
            "storage::users": [
                {"label": "Orders", "kind": "Table", "file": "storage/tables.qmd.md", "score": 0.85},
                {"label": "Get Users", "kind": "Endpoint", "file": "api/endpoints.qmd.md", "score": 0.72},
            ],
            "field:name@storage/tables.qmd.md": [
                {"label": "User Name", "kind": "Column", "file": "storage/columns.qmd.md", "score": 0.68},
            ],
        }
        (semantic_dir / "hints.json").write_text(json.dumps(hints_data))

        result = load_hints(tmp_path)

        assert "storage::users" in result
        assert len(result["storage::users"]) == 2
        assert isinstance(result["storage::users"][0], HintEntry)
        assert result["storage::users"][0].label == "Orders"
        assert result["storage::users"][0].kind == "Table"
        assert result["storage::users"][0].file == "storage/tables.qmd.md"
        assert result["storage::users"][0].score == 0.85

    def test_hint_entry_with_null_kind(self, tmp_path):
        """HintEntry supports None for kind field."""
        semantic_dir = tmp_path / ".qmdc-semantic"
        semantic_dir.mkdir()
        hints_data = {
            "ns::obj": [
                {"label": "Something", "kind": None, "file": "file.qmd.md", "score": 0.5},
            ],
        }
        (semantic_dir / "hints.json").write_text(json.dumps(hints_data))

        result = load_hints(tmp_path)

        assert result["ns::obj"][0].kind is None

    def test_empty_hints_json_returns_empty_dict(self, tmp_path):
        """An empty JSON object results in an empty dict."""
        semantic_dir = tmp_path / ".qmdc-semantic"
        semantic_dir.mkdir()
        (semantic_dir / "hints.json").write_text("{}")

        result = load_hints(tmp_path)

        assert result == {}


class TestGetPageHints:
    """Tests for get_page_hints function."""

    def test_empty_all_hints_returns_empty(self, mock_workspace_db):
        """When all_hints is empty, returns empty dict regardless of page."""
        result = get_page_hints("storage/tables.qmd.md", mock_workspace_db, {})
        assert result == {}

    def test_filters_by_global_id(self, mock_workspace_db, sample_hints):
        """Hints keyed by global_id are matched to objects on the page."""
        # sample_hints has "storage::users" which maps to object "users" in storage/tables.qmd.md
        all_hints = {
            key: [HintEntry(**h) for h in entries]
            for key, entries in sample_hints.items()
        }

        result = get_page_hints("storage/tables.qmd.md", mock_workspace_db, all_hints)

        # "users" object has global_id "storage::users" which is in hints
        assert "users" in result
        assert len(result["users"]) == 2
        assert result["users"][0].label == "Orders"
        assert result["users"][0].score == 0.85

    def test_field_level_hint_key_matching(self, mock_workspace_db, sample_hints):
        """Field-level hints (field:id@file) are matched by file suffix."""
        all_hints = {
            key: [HintEntry(**h) for h in entries]
            for key, entries in sample_hints.items()
        }

        result = get_page_hints("storage/tables.qmd.md", mock_workspace_db, all_hints)

        # "field:name@storage/tables.qmd.md" should match this page
        assert "name" in result
        assert len(result["name"]) == 1
        assert result["name"][0].label == "User Name Field"
        assert result["name"][0].score == 0.68

    def test_no_match_for_different_page(self, mock_workspace_db, sample_hints):
        """Hints for storage/tables.qmd.md don't appear for api/endpoints.qmd.md."""
        all_hints = {
            key: [HintEntry(**h) for h in entries]
            for key, entries in sample_hints.items()
        }

        result = get_page_hints("api/endpoints.qmd.md", mock_workspace_db, all_hints)

        # "storage::users" belongs to storage/tables.qmd.md, not api/endpoints.qmd.md
        assert "users" not in result
        # field:name@storage/tables.qmd.md doesn't match api/endpoints.qmd.md
        assert "name" not in result

    def test_excludes_system_type_objects(self, mock_workspace_db):
        """Objects with system types (__Workspace, __Namespace) are excluded from matching."""
        # The workspace object "myproject" has global_id "myproject"
        all_hints = {
            "myproject": [HintEntry(label="Something", kind="X", file="other.qmd.md", score=0.9)],
        }

        # readme.qmd.md has the __Workspace object, but it should be excluded
        result = get_page_hints("readme.qmd.md", mock_workspace_db, all_hints)

        # __Workspace objects are filtered out by the query (NOT GLOB '__*')
        assert "myproject" not in result

    def test_multiple_objects_on_same_page(self, mock_workspace_db):
        """Multiple objects on the same page each get their hints."""
        all_hints = {
            "myproject:storage:users": [HintEntry(label="A", kind="X", file="a.qmd.md", score=0.8)],
            "myproject:storage:orders": [HintEntry(label="B", kind="Y", file="b.qmd.md", score=0.7)],
        }

        result = get_page_hints("storage/tables.qmd.md", mock_workspace_db, all_hints)

        assert "users" in result
        assert "orders" in result
        assert result["users"][0].label == "A"
        assert result["orders"][0].label == "B"
