"""Unit tests for qmdc_mkdocs.database module."""

from pathlib import Path

import pytest

from qmdc_mkdocs.database import WorkspaceData, WorkspaceLoadError, load_workspace


class TestLoadWorkspace:
    """Tests for load_workspace function."""

    def test_loads_valid_workspace(self, sample_workspace):
        """load_workspace returns WorkspaceData for a valid workspace."""
        ws = load_workspace(sample_workspace)
        try:
            assert isinstance(ws, WorkspaceData)
            assert ws.result is not None
            assert ws.db is not None
            assert len(ws.result.objects) > 0
        finally:
            ws.close()

    def test_raises_load_error_for_nonexistent_path(self):
        """load_workspace raises WorkspaceLoadError for a path that doesn't exist."""
        with pytest.raises(WorkspaceLoadError, match="does not exist"):
            load_workspace(Path("/nonexistent/path"))

    def test_raises_load_error_for_empty_workspace(self, tmp_path):
        """load_workspace raises WorkspaceLoadError when no .qmd.md files found."""
        with pytest.raises(WorkspaceLoadError, match="No .qmd.md files found"):
            load_workspace(tmp_path)

    def test_objects_by_file_grouping(self, sample_workspace):
        """load_workspace groups objects by __file correctly."""
        ws = load_workspace(sample_workspace)
        try:
            assert len(ws.objects_by_file) > 0
            for file_path, objects in ws.objects_by_file.items():
                assert isinstance(file_path, str)
                assert len(objects) > 0
                for obj in objects:
                    assert obj.get("__file") == file_path
        finally:
            ws.close()

    def test_result_has_files(self, sample_workspace):
        """load_workspace result contains parsed files list."""
        ws = load_workspace(sample_workspace)
        try:
            assert len(ws.result.files) > 0
            assert "readme.qmd.md" in ws.result.files
        finally:
            ws.close()


class TestWorkspaceDataQuery:
    """Tests for WorkspaceData.query method."""

    def test_returns_list_of_dicts(self, sample_workspace):
        """query() returns a list of dictionaries."""
        ws = load_workspace(sample_workspace)
        try:
            results = ws.query("SELECT __id, __kind FROM objects LIMIT 1")
            assert isinstance(results, list)
            assert len(results) == 1
            assert isinstance(results[0], dict)
            assert "__id" in results[0]
            assert "__kind" in results[0]
        finally:
            ws.close()

    def test_query_returns_correct_data(self, sample_workspace):
        """query() returns correct data from the database."""
        ws = load_workspace(sample_workspace)
        try:
            results = ws.query(
                "SELECT __id FROM objects WHERE __kind = 'Table'"
            )
            ids = {r["__id"] for r in results}
            assert "users" in ids
            assert "orders" in ids
        finally:
            ws.close()

    def test_query_empty_result(self, sample_workspace):
        """query() returns empty list when no rows match."""
        ws = load_workspace(sample_workspace)
        try:
            results = ws.query(
                "SELECT __id FROM objects WHERE __kind = 'NonExistent'"
            )
            assert results == []
        finally:
            ws.close()

    def test_query_edges_table(self, sample_workspace):
        """query() works on the edges table."""
        ws = load_workspace(sample_workspace)
        try:
            results = ws.query("SELECT source_id, target_id FROM edges LIMIT 5")
            assert isinstance(results, list)
            # The sample workspace has references, so edges should exist
            if results:
                assert "source_id" in results[0]
                assert "target_id" in results[0]
        finally:
            ws.close()


class TestWorkspaceDataClose:
    """Tests for WorkspaceData.close method."""

    def test_close_does_not_raise(self, sample_workspace):
        """close() completes without error."""
        ws = load_workspace(sample_workspace)
        ws.close()  # Should not raise
