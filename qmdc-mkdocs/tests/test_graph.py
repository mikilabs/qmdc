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


HIDDEN_KIND_FILES = {
    "readme.qmd.md": "# Repro [[repro: __Workspace]]\n",
    "product.qmd.md": """\
# Product [[p: Product]]

Text.

## Quality score [[s: Score]]

- axis: [[#quality]]
- value: 3

## Speed score [[s2: Score]]

- axis: [[#quality]]
- value: 4
""",
    "quality.qmd.md": """\
# Quality [[quality: Axis]]

What quality means.
""",
    "scores.qmd.md": """\
# Loose score [[loose: Score]]

- axis: [[#quality]]
""",
}


class TestHiddenKinds:
    """`hidden_kinds` hides an object, not the page that holds it (#42)."""

    @staticmethod
    def _ws(tmp_path):
        from qmdc_mkdocs.database import load_workspace

        for rel_path, content in HIDDEN_KIND_FILES.items():
            (tmp_path / rel_path).write_text(content, encoding="utf-8")
        return load_workspace(tmp_path)

    def test_hidden_source_is_credited_to_its_page(self, tmp_path):
        ws = self._ws(tmp_path)
        try:
            ctx = compute_graph_context("quality.qmd.md", ws, hidden_kinds=["Score"])
        finally:
            ws.close()
        # Two Score objects in product.qmd.md give one Product entry; the file
        # whose top-level object is itself a Score gives none.
        assert [(e.edge_type, e.obj_id, e.label, e.kind, e.file) for e in ctx.linked_from] == [
            ("axis", "p", "Product", "Product", "product.qmd.md"),
        ]

    def test_without_hidden_kinds_sources_are_listed(self, tmp_path):
        ws = self._ws(tmp_path)
        try:
            ctx = compute_graph_context("quality.qmd.md", ws)
        finally:
            ws.close()
        assert sorted(e.obj_id for e in ctx.linked_from) == ["loose", "p.s", "p.s2"]

    def test_links_to_side_is_unchanged(self, tmp_path):
        ws = self._ws(tmp_path)
        try:
            ctx = compute_graph_context("product.qmd.md", ws, hidden_kinds=["Score"])
        finally:
            ws.close()
        assert [(e.edge_type, e.obj_id) for e in ctx.links_to] == [("axis", "quality")]


NAMESPACE_INDEX_FILES = {
    "readme.qmd.md": "# A long descriptive workspace title for the whole site [[ws: __Workspace]]\n",
    "news/readme.qmd.md": """\
# News [[news: __Namespace]]

Latest updates.

### First update [[u1: Update]]

Text.

### Second update [[u2: Update]]

Text.
""",
    "news/archive.qmd.md": """\
# Archive [[archive: Page]]

Older updates.
""",
}


class TestIndexPageBreadcrumb:
    """A namespace or workspace readme is named by its own heading (#41)."""

    WS = "A long descriptive workspace title for the whole site"

    @staticmethod
    def _ws(tmp_path):
        from qmdc_mkdocs.database import load_workspace

        for rel_path, content in NAMESPACE_INDEX_FILES.items():
            p = tmp_path / rel_path
            p.parent.mkdir(parents=True, exist_ok=True)
            p.write_text(content, encoding="utf-8")
        return load_workspace(tmp_path)

    def _ctx(self, tmp_path, source_file, **kwargs):
        ws = self._ws(tmp_path)
        try:
            return compute_graph_context(source_file, ws, **kwargs)
        finally:
            ws.close()

    def test_namespace_readme_is_the_namespace(self, tmp_path):
        ctx = self._ctx(tmp_path, "news/readme.qmd.md")
        assert ctx.file_label == "News"
        assert ctx.current_level == "namespace"
        assert ctx.workspace_page == "readme.qmd.md"
        assert ctx.namespace_page is None  # it is the current page

    def test_namespace_readme_sibling_label(self, tmp_path):
        ctx = self._ctx(tmp_path, "news/readme.qmd.md")
        assert [(s.label, s.is_current) for s in ctx.siblings] == [
            ("Archive", False),
            ("News", True),
        ]

    def test_page_in_namespace_links_both_ancestors(self, tmp_path):
        ctx = self._ctx(tmp_path, "news/archive.qmd.md")
        assert ctx.file_label == "Archive"
        assert ctx.current_level == "file"
        assert ctx.workspace_page == "readme.qmd.md"
        assert ctx.namespace_page == "news/readme.qmd.md"

    def test_workspace_readme_is_the_workspace(self, tmp_path):
        ctx = self._ctx(tmp_path, "readme.qmd.md")
        assert ctx.file_label == self.WS
        assert ctx.current_level == "workspace"
        assert ctx.workspace_page is None

    def test_namespace_build_has_no_workspace_link(self, tmp_path):
        ctx = self._ctx(tmp_path, "news/archive.qmd.md", namespace_prefix="news")
        assert ctx.workspace_page is None
        assert ctx.namespace_page == "readme.qmd.md"  # output path, prefix stripped

    def test_ignored_index_page_is_not_linked(self, tmp_path):
        ctx = self._ctx(
            tmp_path, "news/archive.qmd.md", ignore_patterns=["news/readme.qmd.md"]
        )
        assert ctx.namespace_page is None
        assert ctx.workspace_page == "readme.qmd.md"

    def test_breadcrumb_template(self, tmp_path):
        from pathlib import Path

        import jinja2

        import qmdc_mkdocs
        from qmdc_mkdocs.converter import _build_front_matter
        import yaml

        templates = Path(qmdc_mkdocs.__file__).parent / "templates"
        env = jinja2.Environment(loader=jinja2.FileSystemLoader(str(templates)))
        tmpl = env.get_template("partials/toc.html")

        def render(source_file):
            ctx = self._ctx(tmp_path, source_file)
            fm = _build_front_matter(ctx, {}, source_file)
            sidebar = yaml.safe_load(fm.split("---\n")[1])["_graph_sidebar"]
            html = tmpl.render(qmdc_graph_sidebar=sidebar)
            return html.split('<div class="sb-breadcrumb">', 1)[1].split("</div>", 1)[0]

        crumb = render("news/readme.qmd.md")
        assert f'href="../">{self.WS}</a>' in crumb
        assert "News</span>" in crumb
        assert "First update" not in crumb
        assert crumb.count("sb-breadcrumb__sep") == 1

        crumb = render("news/archive.qmd.md")
        assert f'href="../../">{self.WS}</a>' in crumb
        assert 'href="../">News</a>' in crumb
        assert "Archive</span>" in crumb
