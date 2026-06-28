"""Unit tests for references.py — position-based [[#id]] resolution."""

import pytest

from qmdc_mkdocs.references import (
    Replacement,
    _compute_relative_path,
    _direct_lookup,
    _find_target,
    _compute_global_id,
    broken_link_span,
    obj_label,
    resolve_references,
)


class TestHelpers:
    """broken_link_span / obj_label — small shared helpers."""

    def test_obj_label_prefers_label(self):
        assert obj_label({"__label": "Users", "__id": "users"}) == "Users"

    def test_obj_label_falls_back_to_id(self):
        assert obj_label({"__id": "users"}) == "users"
        assert obj_label({"__label": "", "__id": "users"}) == "users"

    def test_broken_link_span_escapes(self):
        out = broken_link_span("a<b>&c")
        assert out == '<span class="broken-link">a&lt;b&gt;&amp;c</span>'


class TestComputeRelativePath:
    """Tests for _compute_relative_path."""

    def test_same_directory(self):
        result = _compute_relative_path(
            "storage/tables.qmd.md", "storage/indexes.qmd.md"
        )
        assert result == "indexes.md"

    def test_sibling_directory(self):
        result = _compute_relative_path(
            "api/endpoints.qmd.md", "storage/tables.qmd.md"
        )
        assert result == "../storage/tables.md"

    def test_root_to_subdirectory(self):
        result = _compute_relative_path(
            "readme.qmd.md", "storage/tables.qmd.md"
        )
        assert result == "storage/tables.md"

    def test_subdirectory_to_root(self):
        result = _compute_relative_path(
            "storage/tables.qmd.md", "readme.qmd.md"
        )
        # readme.qmd.md → index.md
        assert result == "../index.md"

    def test_deeply_nested(self):
        result = _compute_relative_path(
            "a/b/c/file.qmd.md", "x/y/target.qmd.md"
        )
        assert result == "../../../x/y/target.md"

    def test_same_file(self):
        result = _compute_relative_path(
            "storage/tables.qmd.md", "storage/tables.qmd.md"
        )
        assert result == "tables.md"


class TestDirectLookup:
    """Tests for _direct_lookup with the workspace DB."""

    def test_simple_ref_found(self, mock_workspace_db):
        result = _direct_lookup(["users"], mock_workspace_db)
        assert result is not None
        assert result["__file"] == "storage/tables.qmd.md"
        assert result["__id"] == "users"
        assert result["__label"] == "Users"

    def test_simple_ref_not_found(self, mock_workspace_db):
        result = _direct_lookup(["nonexistent"], mock_workspace_db)
        assert result is None

    def test_kind_qualified_ref(self, mock_workspace_db):
        result = _direct_lookup(["Table", "users"], mock_workspace_db)
        assert result is not None
        assert result["__file"] == "storage/tables.qmd.md"
        assert result["__id"] == "users"

    def test_kind_qualified_ref_endpoint(self, mock_workspace_db):
        result = _direct_lookup(["Endpoint", "get_users"], mock_workspace_db)
        assert result is not None
        assert result["__file"] == "api/endpoints.qmd.md"
        assert result["__id"] == "get_users"
        assert result["__label"] == "Get Users"

    def test_namespace_qualified_ref(self, mock_workspace_db):
        result = _direct_lookup(["storage", "users"], mock_workspace_db)
        assert result is not None
        assert result["__file"] == "storage/tables.qmd.md"
        assert result["__id"] == "users"

    def test_full_qualified_ref(self, mock_workspace_db):
        result = _direct_lookup(["storage", "Table", "users"], mock_workspace_db)
        assert result is not None
        assert result["__file"] == "storage/tables.qmd.md"
        assert result["__id"] == "users"

    def test_invalid_format_too_many_parts(self, mock_workspace_db):
        result = _direct_lookup(["a", "b", "c", "d"], mock_workspace_db)
        assert result is None

    def test_resolves_system_types_for_simple_ref(self, mock_workspace_db):
        """Simple refs resolve to system objects too (e.g. a __Workspace id).

        System objects (__Workspace/__Namespace) are valid reference targets, so
        a bare ref like [[#myproject]] should resolve to the workspace object.
        """
        result = _direct_lookup(["myproject"], mock_workspace_db)
        assert result is not None
        assert result["__id"] == "myproject"


class TestResolveReferences:
    """Integration tests for resolve_references using real parser data."""

    def test_resolves_refs_from_parser_data(self, sample_workspace, sample_ws_data):
        """Test that references in the sample workspace are resolved correctly."""
        # The api/endpoints.qmd.md file has references to storage:users and storage:orders
        source_file = "api/endpoints.qmd.md"
        file_objects = sample_ws_data.objects_by_file.get(source_file, [])

        # Verify we have objects with references
        has_refs = False
        for obj in file_objects:
            if obj.get("__references"):
                has_refs = True
                break

        if not has_refs:
            pytest.skip("No __references found in test data")

        # Read the actual file content as lines
        file_path = sample_workspace / source_file
        lines = file_path.read_text().splitlines(keepends=True)

        result = resolve_references(lines, file_objects, source_file, sample_ws_data)

        # The result should have resolved references
        result_text = "\n".join(result)
        # Check that at least one reference was resolved to a link
        assert "[Users](../storage/tables.md#users)" in result_text or \
               "[Orders](../storage/tables.md#orders)" in result_text

    def test_broken_ref_renders_span(self, sample_ws_data):
        """Test that unresolvable references get broken-link spans."""
        # Create synthetic file_objects with a broken reference
        lines = ["- ref: [[#nonexistent]]\n"]
        file_objects = [
            {
                "__id": "test_obj",
                "__workspace": "myproject",
                "__namespace": "test",
                "__references": [
                    {
                        "line": 1,
                        "start_col": 7,
                        "end_col": 22,
                        "raw": "[[#nonexistent]]",
                        "target": "#nonexistent",
                        "type": "hash_local",
                    }
                ],
            }
        ]
        result = resolve_references(lines, file_objects, "test/file.qmd.md", sample_ws_data)
        assert '<span class="broken-link">[[#nonexistent]]</span>' in result[0]

    def test_ref_to_ignored_target_renders_dead_link(self, sample_ws_data):
        """A reference whose target file is ignored becomes a dead link.

        The target object exists in the graph, but its page is excluded from the
        site (.qmdc-mkdocs.ignore). Linking to it would 404 / warn in MkDocs, so
        we render the target's label as a non-navigable broken-link span (styled
        like a dead red link) instead of a Markdown link.
        """
        lines = ["- returns: [[#storage:users]]\n"]
        file_objects = [
            {
                "__id": "ep",
                "__workspace": "myproject",
                "__namespace": "api",
                "__references": [
                    {
                        "line": 1,
                        "start_col": 11,
                        "end_col": 29,
                        "raw": "[[#storage:users]]",
                        "target": "#storage:users",
                        "type": "hash_namespace",
                    }
                ],
            }
        ]
        result = resolve_references(
            lines,
            file_objects,
            "api/endpoints.qmd.md",
            sample_ws_data,
            ignore_patterns=["storage/**"],
        )
        # Shows the label, styled as a dead link — NOT a Markdown link.
        assert '<span class="broken-link">Users</span>' in result[0]
        assert "](../storage/tables.md#users)" not in result[0]

    def test_ref_to_non_ignored_target_still_links(self, sample_ws_data):
        """With ignore patterns that don't match the target, the link is normal."""
        lines = ["- returns: [[#storage:users]]\n"]
        file_objects = [
            {
                "__id": "ep",
                "__workspace": "myproject",
                "__namespace": "api",
                "__references": [
                    {
                        "line": 1,
                        "start_col": 11,
                        "end_col": 29,
                        "raw": "[[#storage:users]]",
                        "target": "#storage:users",
                        "type": "hash_namespace",
                    }
                ],
            }
        ]
        result = resolve_references(
            lines,
            file_objects,
            "api/endpoints.qmd.md",
            sample_ws_data,
            ignore_patterns=["tracking/**"],
        )
        assert "[Users](../storage/tables.md#users)" in result[0]

    def test_broken_ref_raw_text_is_html_escaped(self, sample_ws_data):
        """A broken ref's raw text is HTML-escaped inside the dead-link span.

        The visible text comes from author content and is interpolated into
        HTML; it must be escaped (matching syntax.py), so a stray ``<`` cannot
        break out of the span.
        """
        lines = ["- see: [[#a<b>&c]]\n"]
        file_objects = [
            {
                "__id": "ep",
                "__workspace": "myproject",
                "__namespace": "api",
                "__references": [
                    {
                        "line": 1,
                        "start_col": 7,
                        "end_col": 18,
                        "raw": "[[#a<b>&c]]",
                        "target": "#a<b>&c",
                        "type": "hash",
                    }
                ],
            }
        ]
        result = resolve_references(lines, file_objects, "api/endpoints.qmd.md", sample_ws_data)
        assert "&lt;b&gt;" in result[0]
        assert "&amp;c" in result[0]
        # The raw, unescaped angle brackets must NOT appear inside the span.
        assert "<b>" not in result[0]

    def test_no_refs_returns_unchanged(self, sample_ws_data):
        """Test that files without references are returned unchanged."""
        lines = ["# Title\n", "\n", "Some content\n"]
        file_objects = [{"__id": "title", "__workspace": "myproject", "__namespace": ""}]
        result = resolve_references(lines, file_objects, "readme.qmd.md", sample_ws_data)
        assert result == lines

    def test_multiple_refs_on_same_line(self, sample_ws_data):
        """Test back-to-front replacement preserves positions."""
        lines = ["- deps: [[#users]], [[#orders]]\n"]
        file_objects = [
            {
                "__id": "test_obj",
                "__workspace": "myproject",
                "__namespace": "api",
                "__references": [
                    {
                        "line": 1,
                        "start_col": 8,
                        "end_col": 19,
                        "raw": "[[#users]]",
                        "target": "#users",
                        "type": "hash_local",
                    },
                    {
                        "line": 1,
                        "start_col": 21,
                        "end_col": 33,
                        "raw": "[[#orders]]",
                        "target": "#orders",
                        "type": "hash_local",
                    },
                ],
            }
        ]
        result = resolve_references(lines, file_objects, "api/endpoints.qmd.md", sample_ws_data)
        # Both should be resolved (users and orders exist in the sample workspace)
        assert "[Users]" in result[0]
        assert "[Orders]" in result[0]
        # Verify the link paths are correct (api → storage)
        assert "../storage/tables.md#users" in result[0]
        assert "../storage/tables.md#orders" in result[0]

    def test_refs_on_different_lines(self, sample_ws_data):
        """Test references on different lines are all resolved."""
        lines = [
            "- user: [[#users]]\n",
            "- order: [[#orders]]\n",
        ]
        file_objects = [
            {
                "__id": "test_obj",
                "__workspace": "myproject",
                "__namespace": "api",
                "__references": [
                    {
                        "line": 1,
                        "start_col": 8,
                        "end_col": 19,
                        "raw": "[[#users]]",
                        "target": "#users",
                        "type": "hash_local",
                    },
                    {
                        "line": 2,
                        "start_col": 9,
                        "end_col": 21,
                        "raw": "[[#orders]]",
                        "target": "#orders",
                        "type": "hash_local",
                    },
                ],
            }
        ]
        result = resolve_references(lines, file_objects, "api/endpoints.qmd.md", sample_ws_data)
        assert "[Users](../storage/tables.md#users)" in result[0]
        assert "[Orders](../storage/tables.md#orders)" in result[1]

    def test_namespace_qualified_ref(self, sample_ws_data):
        """Test [[#storage:users]] cross-namespace reference resolution."""
        lines = ["- returns: [[#storage:users]]\n"]
        file_objects = [
            {
                "__id": "test_obj",
                "__workspace": "myproject",
                "__namespace": "api",
                "__references": [
                    {
                        "line": 1,
                        "start_col": 11,
                        "end_col": 28,
                        "raw": "[[#storage:users]]",
                        "target": "#storage:users",
                        "type": "namespace",
                    },
                ],
            }
        ]
        result = resolve_references(lines, file_objects, "api/endpoints.qmd.md", sample_ws_data)
        assert "[Users](../storage/tables.md#users)" in result[0]

    def test_kind_qualified_ref(self, sample_ws_data):
        """Test [[#Table:users]] kind-qualified reference resolution."""
        lines = ["- table: [[#Table:users]]\n"]
        file_objects = [
            {
                "__id": "test_obj",
                "__workspace": "myproject",
                "__namespace": "api",
                "__references": [
                    {
                        "line": 1,
                        "start_col": 9,
                        "end_col": 24,
                        "raw": "[[#Table:users]]",
                        "target": "#Table:users",
                        "type": "kind",
                    },
                ],
            }
        ]
        result = resolve_references(lines, file_objects, "api/endpoints.qmd.md", sample_ws_data)
        assert "[Users](../storage/tables.md#users)" in result[0]



