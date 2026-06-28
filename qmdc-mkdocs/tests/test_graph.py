"""Unit tests for graph.py — compute_graph_context and helpers."""

from qmdc_mkdocs.graph import (
    EdgeItem,
    GraphContext,
    SiblingItem,
    _get_file_label,
    _get_namespace_label,
    _get_siblings,
    _get_toc,
    _get_workspace_label,
    compute_graph_context,
)


class TestGetWorkspaceLabel:
    def test_returns_workspace_label(self, mock_workspace_db):
        label = _get_workspace_label(mock_workspace_db)
        assert label == "My Project"

    def test_returns_fallback_when_no_workspace(self, mock_workspace_db):
        # Remove the workspace object
        mock_workspace_db.db.conn.execute(
            "DELETE FROM objects WHERE __kind = '__Workspace'"
        )
        label = _get_workspace_label(mock_workspace_db)
        assert label == "Workspace"


class TestGetNamespaceLabel:
    def test_returns_namespace_label_for_nested_file(self, mock_workspace_db):
        label = _get_namespace_label("storage/tables.qmd.md", mock_workspace_db)
        assert label == "Storage Layer"

    def test_returns_none_for_root_file(self, mock_workspace_db):
        label = _get_namespace_label("readme.qmd.md", mock_workspace_db)
        assert label is None

    def test_returns_none_for_unknown_namespace(self, mock_workspace_db):
        label = _get_namespace_label("unknown/file.qmd.md", mock_workspace_db)
        assert label is None


class TestGetFileLabel:
    def test_returns_top_level_object_label(self, mock_workspace_db):
        label = _get_file_label("storage/tables.qmd.md", mock_workspace_db)
        assert label == "Users"

    def test_returns_derived_label_for_file_without_objects(self, mock_workspace_db):
        label = _get_file_label("nonexistent/file.qmd.md", mock_workspace_db)
        assert label == "File"


class TestGetSiblings:
    def test_returns_siblings_in_same_directory(self, mock_workspace_db):
        siblings = _get_siblings("storage/tables.qmd.md", mock_workspace_db)
        files = [s.file for s in siblings]
        assert "storage/readme.qmd.md" in files
        assert "storage/tables.qmd.md" in files

    def test_marks_current_file(self, mock_workspace_db):
        siblings = _get_siblings("storage/tables.qmd.md", mock_workspace_db)
        current = [s for s in siblings if s.is_current]
        assert len(current) == 1
        assert current[0].file == "storage/tables.qmd.md"

    def test_root_level_siblings(self, mock_workspace_db):
        siblings = _get_siblings("readme.qmd.md", mock_workspace_db)
        files = [s.file for s in siblings]
        assert "readme.qmd.md" in files
        # Should not include files from subdirectories
        assert "storage/tables.qmd.md" not in files


class TestGetToc:
    def test_returns_toc_entries_for_page(self, mock_workspace_db):
        toc = _get_toc("storage/tables.qmd.md", mock_workspace_db)
        assert len(toc) == 2
        ids = [entry["id"] for entry in toc]
        assert "users" in ids
        assert "orders" in ids

    def test_toc_entries_have_required_keys(self, mock_workspace_db):
        toc = _get_toc("storage/tables.qmd.md", mock_workspace_db)
        for entry in toc:
            assert "id" in entry
            assert "label" in entry
            assert "level" in entry

    def test_toc_excludes_system_objects(self, mock_workspace_db):
        toc = _get_toc("readme.qmd.md", mock_workspace_db)
        # readme.qmd.md only has the __Workspace object, which is system
        assert len(toc) == 0

    def test_toc_level_is_integer(self, mock_workspace_db):
        toc = _get_toc("storage/tables.qmd.md", mock_workspace_db)
        for entry in toc:
            assert isinstance(entry["level"], int)


class TestComputeGraphContext:
    def test_returns_graph_context_dataclass(self, mock_workspace_db):
        ctx = compute_graph_context("api/endpoints.qmd.md", mock_workspace_db)
        assert isinstance(ctx, GraphContext)

    def test_breadcrumb_components(self, mock_workspace_db):
        ctx = compute_graph_context("api/endpoints.qmd.md", mock_workspace_db)
        assert ctx.workspace_label == "My Project"
        assert ctx.namespace_label == "API Layer"
        assert ctx.file_label == "Get Users"

    def test_links_to_outgoing_edges(self, mock_workspace_db):
        # api/endpoints.qmd.md has get_users→users and get_orders→orders edges
        ctx = compute_graph_context("api/endpoints.qmd.md", mock_workspace_db)
        assert len(ctx.links_to) > 0
        target_ids = [e.obj_id for e in ctx.links_to]
        assert "users" in target_ids
        assert "orders" in target_ids

    def test_linked_from_incoming_edges(self, mock_workspace_db):
        # storage/tables.qmd.md has users referenced by get_users (from api)
        ctx = compute_graph_context("storage/tables.qmd.md", mock_workspace_db)
        assert len(ctx.linked_from) > 0
        source_ids = [e.obj_id for e in ctx.linked_from]
        assert "get_users" in source_ids

    def test_links_to_are_edge_items(self, mock_workspace_db):
        ctx = compute_graph_context("api/endpoints.qmd.md", mock_workspace_db)
        for edge in ctx.links_to:
            assert isinstance(edge, EdgeItem)
            assert edge.edge_type
            assert edge.obj_id
            assert edge.label
            assert edge.kind
            assert edge.file

    def test_siblings_list(self, mock_workspace_db):
        ctx = compute_graph_context("api/endpoints.qmd.md", mock_workspace_db)
        assert len(ctx.siblings) > 0
        files = [s.file for s in ctx.siblings]
        assert "api/endpoints.qmd.md" in files
        assert "api/readme.qmd.md" in files

    def test_toc_entries(self, mock_workspace_db):
        ctx = compute_graph_context("storage/tables.qmd.md", mock_workspace_db)
        assert len(ctx.toc) == 2
        assert ctx.toc[0]["id"] == "users"
        assert ctx.toc[0]["label"] == "Users"

    def test_no_self_edges_in_links_to(self, mock_workspace_db):
        # Edges within the same file should not appear in links_to
        ctx = compute_graph_context("storage/tables.qmd.md", mock_workspace_db)
        for edge in ctx.links_to:
            assert edge.file != "storage/tables.qmd.md"

    def test_no_self_edges_in_linked_from(self, mock_workspace_db):
        # Edges within the same file should not appear in linked_from
        ctx = compute_graph_context("storage/tables.qmd.md", mock_workspace_db)
        for edge in ctx.linked_from:
            assert edge.file != "storage/tables.qmd.md"

    def test_excludes_system_kinds_from_edges(self, mock_workspace_db):
        ctx = compute_graph_context("api/endpoints.qmd.md", mock_workspace_db)
        for edge in ctx.links_to:
            assert not edge.kind.startswith("__")
        for edge in ctx.linked_from:
            assert not edge.kind.startswith("__")
