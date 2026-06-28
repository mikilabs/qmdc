"""Unit tests for qmdc_mkdocs.syntax — QMD syntax transformation."""

from qmdc_mkdocs.syntax import transform_definitions, transform_qmdc_syntax


# =============================================================================
# Tests for transform_definitions (line-targeted, parser-driven)
# =============================================================================


class TestTransformDefinitionsBasic:
    """transform_definitions uses __line to locate and transform heading lines."""

    def test_nested_object_anchor_uses_global_id(self):
        """A nested object's anchor must be its full __id (dot-path), not the
        literal local id written in the heading.

        The parser composes nested ids as ``parent.child`` (``__id``), while the
        heading literally says ``[[child: Kind]]`` (the ``__local_id``).
        References and the graph sidebar link to ``#__id``, so the heading
        anchor must match ``__id`` — otherwise MkDocs reports a missing anchor.
        """
        lines = ["## Inferred Edges [[algo_inferred_edges: Algorithm]]\n"]
        file_objects = [
            {
                "__line": 1,
                "__id": "semantic_algorithms.algo_inferred_edges",
                "__local_id": "algo_inferred_edges",
                "__kind": "Algorithm",
            }
        ]
        result = transform_definitions(lines, file_objects)
        assert 'id="semantic_algorithms.algo_inferred_edges"' in result[0]
        # The local-id-only anchor must NOT be what we emit.
        assert 'id="algo_inferred_edges"' not in result[0]
        # Kind badge still present.
        assert ">Algorithm</span>" in result[0]

    def test_transforms_heading_at_correct_line(self):
        lines = [
            "# My Document\n",
            "\n",
            "## Users [[users: Table]]\n",
            "\n",
            "- name: Alice\n",
        ]
        file_objects = [{"__line": 3, "__id": "users", "__kind": "Table"}]
        result = transform_definitions(lines, file_objects)
        # Line 3 (index 2) should be transformed
        assert '<span class="qmdc-id" data-pagefind-ignore id="users">' in result[2]
        assert '<span class="qmdc-kind" data-pagefind-filter="kind">Table</span>' in result[2]

    def test_multiple_objects_on_different_lines(self):
        lines = [
            "# Doc [[doc: __Namespace]]\n",
            "\n",
            "## Users [[users: Table]]\n",
            "\n",
            "### Address [[address]]\n",
        ]
        file_objects = [
            {"__line": 1, "__id": "doc", "__kind": "__Namespace"},
            {"__line": 3, "__id": "users", "__kind": "Table"},
            {"__line": 5, "__id": "address", "__kind": ""},
        ]
        result = transform_definitions(lines, file_objects)
        # Line 1: system type — no badge
        assert 'id="doc"' in result[0]
        assert "qmdc-kind" not in result[0]
        # Line 3: regular kind — has badge
        assert 'id="users"' in result[2]
        assert ">Table</span>" in result[2]
        # Line 5: bare id
        assert 'id="address"' in result[4]


class TestTransformDefinitionsKindBadge:
    """[[id: Kind]] produces hidden span + kind badge on the correct line."""

    def test_kind_badge_rendered(self):
        lines = ["## API [[api: Service]]\n"]
        file_objects = [{"__line": 1, "__id": "api", "__kind": "Service"}]
        result = transform_definitions(lines, file_objects)
        assert (
            '<span class="qmdc-id" data-pagefind-ignore id="api">'
            "[[api: Service]]</span>"
            '<span class="qmdc-kind" data-pagefind-filter="kind">Service</span>'
        ) in result[0]

    def test_kind_with_whitespace_trimmed(self):
        lines = ["## Config [[cfg:   Config]]\n"]
        file_objects = [{"__line": 1, "__id": "cfg", "__kind": "Config"}]
        result = transform_definitions(lines, file_objects)
        assert 'id="cfg"' in result[0]
        assert ">Config</span>" in result[0]

    def test_kind_badge_html_escaped(self):
        lines = ['## Test [[t: Kind<"special">]]\n']
        file_objects = [{"__line": 1, "__id": "t", "__kind": 'Kind<"special">'}]
        result = transform_definitions(lines, file_objects)
        assert "Kind&lt;&quot;special&quot;&gt;</span>" in result[0]


class TestTransformDefinitionsSystemTypes:
    """System types ([[id: __Namespace]]) produce hidden span only, no badge."""

    def test_namespace_no_badge(self):
        lines = ["# Storage [[storage: __Namespace]]\n"]
        file_objects = [{"__line": 1, "__id": "storage", "__kind": "__Namespace"}]
        result = transform_definitions(lines, file_objects)
        assert 'id="storage"' in result[0]
        assert "[[storage: __Namespace]]</span>" in result[0]
        assert "qmdc-kind" not in result[0]

    def test_workspace_no_badge(self):
        lines = ["# Project [[proj: __Workspace]]\n"]
        file_objects = [{"__line": 1, "__id": "proj", "__kind": "__Workspace"}]
        result = transform_definitions(lines, file_objects)
        assert 'id="proj"' in result[0]
        assert "qmdc-kind" not in result[0]

    def test_document_no_badge(self):
        lines = ["## Notes [[notes: __Document]]\n"]
        file_objects = [{"__line": 1, "__id": "notes", "__kind": "__Document"}]
        result = transform_definitions(lines, file_objects)
        assert 'id="notes"' in result[0]
        assert "qmdc-kind" not in result[0]


class TestTransformDefinitionsBareId:
    """[[id]] produces hidden span with id attribute."""

    def test_bare_id_span(self):
        lines = ["### Address [[address]]\n"]
        file_objects = [{"__line": 1, "__id": "address", "__kind": ""}]
        result = transform_definitions(lines, file_objects)
        assert (
            '<span class="qmdc-id" data-pagefind-ignore id="address">'
            "[[address]]</span>"
        ) in result[0]

    def test_bare_id_no_kind_badge(self):
        lines = ["## Anchor [[my_anchor]]\n"]
        file_objects = [{"__line": 1, "__id": "my_anchor", "__kind": ""}]
        result = transform_definitions(lines, file_objects)
        assert "qmdc-kind" not in result[0]

    def test_bare_id_html_escaped(self):
        lines = ['## Test [[a&b"c]]\n']
        file_objects = [{"__line": 1, "__id": 'a&b"c', "__kind": ""}]
        result = transform_definitions(lines, file_objects)
        assert 'id="a&amp;b&quot;c"' in result[0]


class TestTransformDefinitionsUntouchedLines:
    """Lines NOT referenced by any object's __line are left untouched."""

    def test_non_heading_lines_unchanged(self):
        lines = [
            "## Title [[title: Section]]\n",
            "\n",
            "- field: value\n",
            "Some paragraph with [[looks_like_id]] in it\n",
            "\n",
        ]
        file_objects = [{"__line": 1, "__id": "title", "__kind": "Section"}]
        result = transform_definitions(lines, file_objects)
        # Only line 1 is transformed
        assert 'id="title"' in result[0]
        # Other lines remain exactly as-is
        assert result[1] == "\n"
        assert result[2] == "- field: value\n"
        assert result[3] == "Some paragraph with [[looks_like_id]] in it\n"
        assert result[4] == "\n"

    def test_empty_file_objects_leaves_all_lines_unchanged(self):
        lines = [
            "## Heading [[id: Kind]]\n",
            "- field: [[#ref]]\n",
        ]
        result = transform_definitions(lines, [])
        assert result == lines

    def test_object_with_no_line_skipped(self):
        lines = ["## Title [[title]]\n", "Some text\n"]
        file_objects = [{"__id": "title", "__kind": ""}]  # No __line key
        result = transform_definitions(lines, file_objects)
        assert result == lines

    def test_object_with_invalid_line_skipped(self):
        lines = ["## Title [[title]]\n"]
        file_objects = [{"__line": 0, "__id": "title", "__kind": ""}]  # 0 is invalid (1-based)
        result = transform_definitions(lines, file_objects)
        assert result == lines

    def test_object_with_line_beyond_file_skipped(self):
        lines = ["## Title [[title]]\n"]
        file_objects = [{"__line": 99, "__id": "title", "__kind": ""}]
        result = transform_definitions(lines, file_objects)
        assert result == lines


class TestTransformDefinitionsCodeFenceEdgeCase:
    """Code fences on heading lines are not transformed (edge case).

    If __line points to a line that starts with # but is inside a code fence,
    the function still transforms it (it only checks if the line starts with #).
    However, in practice the parser never sets __line to a line inside a code fence.

    The real protection is that __line only points to actual heading lines.
    But if __line points to a non-heading line, it's skipped.
    """

    def test_non_heading_line_not_transformed(self):
        """If __line points to a non-heading line, it's skipped entirely."""
        lines = [
            "```markdown\n",
            "[[users: Table]]\n",
            "```\n",
        ]
        # Parser would never do this, but if __line points to line 2 (not a heading)
        file_objects = [{"__line": 2, "__id": "users", "__kind": "Table"}]
        result = transform_definitions(lines, file_objects)
        # Line 2 doesn't start with #, so it's skipped
        assert result[1] == "[[users: Table]]\n"

    def test_heading_inside_code_fence_with_line_pointing_to_it(self):
        """Edge case: if __line somehow points to a # line inside a fence.

        In practice the parser never does this, but the function would transform it.
        This documents the behavior — the caller is responsible for correct __line values.
        """
        lines = [
            "## Real Heading [[real: Section]]\n",
            "```markdown\n",
            "## Fake Heading [[fake: Example]]\n",
            "```\n",
        ]
        # Only the real heading is referenced by an object
        file_objects = [{"__line": 1, "__id": "real", "__kind": "Section"}]
        result = transform_definitions(lines, file_objects)
        # Real heading transformed
        assert 'id="real"' in result[0]
        # Fake heading inside fence NOT transformed (no object points to it)
        assert result[2] == "## Fake Heading [[fake: Example]]\n"


class TestDefinitionWithKind:
    """[[id: Kind]] produces hidden span + kind badge."""

    def test_basic_kind_definition(self):
        result = transform_qmdc_syntax("## Users [[users: Table]]")
        assert '<span class="qmdc-id" data-pagefind-ignore id="users">' in result
        assert "[[users: Table]]</span>" in result
        assert '<span class="qmdc-kind" data-pagefind-filter="kind">Table</span>' in result

    def test_kind_badge_adjacent_to_id_span(self):
        result = transform_qmdc_syntax("[[myobj: Service]]")
        # Kind badge immediately follows the ID span
        assert (
            '[[myobj: Service]]</span>'
            '<span class="qmdc-kind" data-pagefind-filter="kind">Service</span>'
        ) in result

    def test_kind_with_extra_whitespace(self):
        result = transform_qmdc_syntax("[[cfg:   Config]]")
        assert 'id="cfg"' in result
        assert ">Config</span>" in result


class TestSystemTypes:
    """System types ([[id: __Namespace]]) produce hidden span only, no badge."""

    def test_namespace_system_type(self):
        result = transform_qmdc_syntax("# Storage [[storage: __Namespace]]")
        assert '<span class="qmdc-id" data-pagefind-ignore id="storage">' in result
        assert "[[storage: __Namespace]]</span>" in result
        assert "qmdc-kind" not in result

    def test_workspace_system_type(self):
        result = transform_qmdc_syntax("# Project [[proj: __Workspace]]")
        assert 'id="proj"' in result
        assert "qmdc-kind" not in result

    def test_document_system_type(self):
        result = transform_qmdc_syntax("[[doc: __Document]]")
        assert 'id="doc"' in result
        assert "qmdc-kind" not in result


class TestBareIdDefinition:
    """[[id]] produces hidden span with id attribute."""

    def test_simple_id(self):
        result = transform_qmdc_syntax("## Users [[users]]")
        assert '<span class="qmdc-id" data-pagefind-ignore id="users">' in result
        assert "[[users]]</span>" in result

    def test_no_kind_badge(self):
        result = transform_qmdc_syntax("[[myanchor]]")
        assert "qmdc-kind" not in result

    def test_id_with_underscores(self):
        result = transform_qmdc_syntax("[[my_long_id]]")
        assert 'id="my_long_id"' in result


class TestCodeFenceProtection:
    """Code fences are not transformed."""

    def test_definition_inside_code_fence(self):
        content = "```markdown\n[[users: Table]]\n```"
        result = transform_qmdc_syntax(content)
        # Should remain untouched
        assert "[[users: Table]]" in result
        assert "qmdc-id" not in result

    def test_bare_id_inside_code_fence(self):
        content = "```\n[[myid]]\n```"
        result = transform_qmdc_syntax(content)
        assert "[[myid]]" in result
        assert "qmdc-id" not in result

    def test_mixed_content_with_fence(self):
        content = "## Title [[title: Section]]\n\n```python\n[[inside: Code]]\n```\n\n[[after]]"
        result = transform_qmdc_syntax(content)
        # Outside fence: transformed
        assert 'id="title"' in result
        assert 'id="after"' in result
        # Inside fence: untouched
        assert "[[inside: Code]]" in result


class TestInlineCodeProtection:
    """Inline code is not transformed."""

    def test_definition_inside_inline_code(self):
        result = transform_qmdc_syntax("Use `[[id: Kind]]` syntax")
        assert "`[[id: Kind]]`" in result
        assert "qmdc-id" not in result

    def test_bare_id_inside_inline_code(self):
        result = transform_qmdc_syntax("Reference `[[myid]]` here")
        assert "`[[myid]]`" in result
        assert "qmdc-id" not in result

    def test_mixed_inline_and_regular(self):
        content = "## Heading [[heading]]\n\nSee `[[example]]` for details"
        result = transform_qmdc_syntax(content)
        # Regular: transformed
        assert 'id="heading"' in result
        # Inline code: untouched
        assert "`[[example]]`" in result


class TestAlreadyResolvedReferences:
    """Already-resolved references (Markdown links) are not affected."""

    def test_markdown_link_not_transformed(self):
        content = "See [Users](storage/tables.md#users) for details"
        result = transform_qmdc_syntax(content)
        assert result == content

    def test_remaining_hash_refs_left_as_is(self):
        # [[#ref]] patterns that weren't resolved remain untouched
        content = "This references [[#some_object]] in text"
        result = transform_qmdc_syntax(content)
        assert "[[#some_object]]" in result

    def test_resolved_links_coexist_with_definitions(self):
        content = "## API [[api: Service]]\n\nSee [Users](tables.md#users)"
        result = transform_qmdc_syntax(content)
        # Definition transformed
        assert 'id="api"' in result
        assert ">Service</span>" in result
        # Link untouched
        assert "[Users](tables.md#users)" in result
