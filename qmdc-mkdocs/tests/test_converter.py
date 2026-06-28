"""Tests for the converter pipeline orchestration."""

from pathlib import Path

import pytest
import yaml

from qmdc_mkdocs.converter import (
    _build_front_matter,
    _compute_href,
    _resolve_remaining_refs_regex,
    convert_workspace,
)
from qmdc_mkdocs.graph import GraphContext, EdgeItem, SiblingItem
from qmdc_mkdocs.hints import HintEntry


class TestRegexFallbackDeadLinks:
    """The regex fallback resolver honors page exclusion (same as references.py).

    The position-based resolver only handles refs the parser extracted into
    __references; the regex fallback catches the rest. Both must turn a ref whose
    target page is excluded into a dead link, not a live (404-ing) link.
    """

    def test_fallback_ref_to_ignored_target_is_dead_link(self, sample_ws_data):
        # A bare [[#users]] in free text — resolved only by the regex fallback,
        # not by parser __references (we pass none).
        content = "See [[#users]] for the schema.\n"
        result = _resolve_remaining_refs_regex(
            content,
            "api/endpoints.qmd.md",
            sample_ws_data,
            ignore_patterns=["storage/**"],
        )
        assert '<span class="broken-link">Users</span>' in result
        assert "](../storage/tables.md#users)" not in result

    def test_fallback_ref_to_non_ignored_target_links(self, sample_ws_data):
        content = "See [[#users]] for the schema.\n"
        result = _resolve_remaining_refs_regex(
            content,
            "api/endpoints.qmd.md",
            sample_ws_data,
            ignore_patterns=["tracking/**"],
        )
        assert "[Users](../storage/tables.md#users)" in result

    def test_fallback_namespace_stripped_exclusion(self, sample_ws_data):
        """Building the 'storage' namespace, a namespace-relative pattern excludes
        the target, so the fallback must also emit a dead link (parity with the
        converter's page-skip)."""
        content = "See [[#users]] for the schema.\n"
        result = _resolve_remaining_refs_regex(
            content,
            "storage/other.qmd.md",
            sample_ws_data,
            ignore_patterns=["tables.qmd.md"],
            namespace_prefix="storage",
        )
        assert '<span class="broken-link">Users</span>' in result


class TestConvertWorkspace:
    """Integration tests for convert_workspace."""

    def test_converts_qmdc_files_to_md(self, tmp_path, mock_workspace_db):
        """Test that .qmd.md files are converted to .md in docs/ directory."""
        workspace = tmp_path / "workspace"
        workspace.mkdir()
        (workspace / "readme.qmd.md").write_text("# Hello [[hello]]\n")

        tmpdir = tmp_path / "build"
        tmpdir.mkdir()

        count = convert_workspace(workspace, tmpdir, mock_workspace_db, {})

        assert count == 1
        # readme.qmd.md → index.md (MkDocs convention)
        output = tmpdir / "docs" / "index.md"
        assert output.exists()

    def test_preserves_directory_structure(self, tmp_path, mock_workspace_db):
        """Test that subdirectory structure is preserved in output."""
        workspace = tmp_path / "workspace"
        workspace.mkdir()
        (workspace / "storage").mkdir()
        (workspace / "storage" / "tables.qmd.md").write_text("# Tables [[tables]]\n")

        tmpdir = tmp_path / "build"
        tmpdir.mkdir()

        count = convert_workspace(workspace, tmpdir, mock_workspace_db, {})

        assert count == 1
        output = tmpdir / "docs" / "storage" / "tables.md"
        assert output.exists()

    def test_skips_hidden_directories(self, tmp_path, mock_workspace_db):
        """Test that files in hidden directories are skipped."""
        workspace = tmp_path / "workspace"
        workspace.mkdir()
        (workspace / ".hidden").mkdir()
        (workspace / ".hidden" / "secret.qmd.md").write_text("# Secret\n")
        (workspace / "visible.qmd.md").write_text("# Visible [[visible]]\n")

        tmpdir = tmp_path / "build"
        tmpdir.mkdir()

        count = convert_workspace(workspace, tmpdir, mock_workspace_db, {})

        assert count == 1
        assert not (tmpdir / "docs" / ".hidden" / "secret.md").exists()
        assert (tmpdir / "docs" / "visible.md").exists()

    def test_skips_pages_directory(self, tmp_path, mock_workspace_db):
        """Test that _pages/*.qmd.md files are not processed as QMD."""
        workspace = tmp_path / "workspace"
        workspace.mkdir()
        (workspace / "_pages").mkdir()
        (workspace / "_pages" / "custom.qmd.md").write_text("# Custom\n")
        (workspace / "readme.qmd.md").write_text("# Hello [[hello]]\n")

        tmpdir = tmp_path / "build"
        tmpdir.mkdir()

        count = convert_workspace(workspace, tmpdir, mock_workspace_db, {})

        assert count == 1  # Only readme, not the _pages file

    def test_copies_pages_md_unchanged(self, tmp_path, mock_workspace_db):
        """Test that _pages/*.md files are copied unchanged to docs/_pages/."""
        workspace = tmp_path / "workspace"
        workspace.mkdir()
        (workspace / "_pages").mkdir()
        custom_content = "# Custom Page\n\nThis is a custom page.\n"
        (workspace / "_pages" / "about.md").write_text(custom_content)

        tmpdir = tmp_path / "build"
        tmpdir.mkdir()

        convert_workspace(workspace, tmpdir, mock_workspace_db, {})

        output = tmpdir / "docs" / "_pages" / "about.md"
        assert output.exists()
        assert output.read_text() == custom_content

    def test_no_pages_dir_is_fine(self, tmp_path, mock_workspace_db):
        """Test that missing _pages/ directory doesn't cause errors."""
        workspace = tmp_path / "workspace"
        workspace.mkdir()
        (workspace / "readme.qmd.md").write_text("# Hello [[hello]]\n")

        tmpdir = tmp_path / "build"
        tmpdir.mkdir()

        count = convert_workspace(workspace, tmpdir, mock_workspace_db, {})
        assert count == 1

    def test_returns_page_count(self, tmp_path, mock_workspace_db):
        """Test that the function returns the correct page count."""
        workspace = tmp_path / "workspace"
        workspace.mkdir()
        (workspace / "a.qmd.md").write_text("# A [[a]]\n")
        (workspace / "b.qmd.md").write_text("# B [[b]]\n")
        (workspace / "sub").mkdir()
        (workspace / "sub" / "c.qmd.md").write_text("# C [[c]]\n")

        tmpdir = tmp_path / "build"
        tmpdir.mkdir()

        count = convert_workspace(workspace, tmpdir, mock_workspace_db, {})
        assert count == 3

    def test_output_has_yaml_front_matter(self, tmp_path, mock_workspace_db):
        """Test that output files contain YAML front matter."""
        workspace = tmp_path / "workspace"
        workspace.mkdir()
        (workspace / "readme.qmd.md").write_text("# Hello [[hello]]\n")

        tmpdir = tmp_path / "build"
        tmpdir.mkdir()

        convert_workspace(workspace, tmpdir, mock_workspace_db, {})

        # readme.qmd.md → index.md (MkDocs convention)
        output = tmpdir / "docs" / "index.md"
        content = output.read_text()
        assert content.startswith("---\n")
        assert "\n---\n" in content

    def test_empty_workspace_returns_zero(self, tmp_path, mock_workspace_db):
        """Test that an empty workspace returns 0 pages."""
        workspace = tmp_path / "workspace"
        workspace.mkdir()

        tmpdir = tmp_path / "build"
        tmpdir.mkdir()

        count = convert_workspace(workspace, tmpdir, mock_workspace_db, {})
        assert count == 0


class TestAssetCopy:
    """Reference-driven media copy: every image/video a page references is copied into
    the built site and its reference rewritten, regardless of where the source lives
    (hidden dir, outside the workspace, absolute path). MkDocs ignores dotfiles and can't
    serve external paths, so in-place serving is impossible for those — they must be copied.
    """

    _PNG = b"\x89PNG\r\n\x1a\n\x00FAKEPNGDATA"
    _REWRITE_RE = r"\(assets/qmdc/[0-9a-f]+/hero\.png\)"

    def _write(self, p: Path, data: bytes = _PNG):
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_bytes(data)

    def test_hidden_dir_asset_copied_and_rewritten(self, tmp_path, mock_workspace_db):
        """An image in a hidden `.assets/` dir (MkDocs would ignore it) is copied into a
        normal site dir and the page reference is rewritten away from the dot-path."""
        import re

        workspace = tmp_path / "workspace"
        workspace.mkdir()
        self._write(workspace / ".assets" / "hero.png")
        (workspace / "guide.qmd.md").write_text(
            "# Guide [[guide]]\n\n![Hero](.assets/hero.png)\n"
        )
        tmpdir = tmp_path / "build"
        tmpdir.mkdir()

        convert_workspace(workspace, tmpdir, mock_workspace_db, {})

        out = (tmpdir / "docs" / "guide.md").read_text()
        assert ".assets/hero.png" not in out, "dot-path ref must be rewritten"
        assert re.search(self._REWRITE_RE, out), f"ref not rewritten as expected: {out!r}"
        copied = list((tmpdir / "docs" / "assets" / "qmdc").rglob("hero.png"))
        assert copied, "image should be copied under docs/assets/qmdc/"
        assert copied[0].read_bytes() == self._PNG

    def test_asset_outside_workspace_copied(self, tmp_path, mock_workspace_db):
        """An absolute path to a file OUTSIDE the workspace is copied in and rewritten."""
        import re

        workspace = tmp_path / "workspace"
        workspace.mkdir()
        external = tmp_path / "downloads" / "hero.png"
        self._write(external)
        (workspace / "guide.qmd.md").write_text(
            f"# Guide [[guide]]\n\n![Hero]({external})\n"
        )
        tmpdir = tmp_path / "build"
        tmpdir.mkdir()

        convert_workspace(workspace, tmpdir, mock_workspace_db, {})

        out = (tmpdir / "docs" / "guide.md").read_text()
        assert str(external) not in out, "absolute external path must be rewritten"
        assert re.search(self._REWRITE_RE, out)
        assert list((tmpdir / "docs" / "assets" / "qmdc").rglob("hero.png"))

    def test_missing_referenced_file_is_skipped_not_fatal(
        self, tmp_path, mock_workspace_db, capsys
    ):
        """A reference to a file that does not exist must NOT break the build: warn + skip,
        leaving the original reference untouched (no copy, no exception)."""
        workspace = tmp_path / "workspace"
        workspace.mkdir()
        (workspace / "guide.qmd.md").write_text(
            "# Guide [[guide]]\n\n![Missing](assets/quickstart-terminal.png)\n"
        )
        tmpdir = tmp_path / "build"
        tmpdir.mkdir()

        # Must not raise.
        convert_workspace(workspace, tmpdir, mock_workspace_db, {})

        out = (tmpdir / "docs" / "guide.md").read_text()
        assert "assets/quickstart-terminal.png" in out, "missing ref left as-is"
        assert not (tmpdir / "docs" / "assets" / "qmdc").exists()
        warn = capsys.readouterr().err
        assert "quickstart-terminal.png" in warn and "WARN" in warn

    def test_remote_and_data_uris_untouched(self, tmp_path, mock_workspace_db):
        """http(s)://, protocol-relative and data: URIs are never treated as local files."""
        workspace = tmp_path / "workspace"
        workspace.mkdir()
        (workspace / "guide.qmd.md").write_text(
            "# Guide [[guide]]\n\n"
            "![Remote](https://example.com/a.png)\n\n"
            "![Data](data:image/png;base64,iVBORw0KGgo=)\n"
        )
        tmpdir = tmp_path / "build"
        tmpdir.mkdir()

        convert_workspace(workspace, tmpdir, mock_workspace_db, {})

        out = (tmpdir / "docs" / "guide.md").read_text()
        assert "https://example.com/a.png" in out
        assert "data:image/png;base64" in out
        assert not (tmpdir / "docs" / "assets" / "qmdc").exists()

    def test_inplace_servable_asset_left_untouched(self, tmp_path, mock_workspace_db):
        """An image already servable in place (non-dot dir, inside the workspace) is left
        alone — minimal intervention, no needless copy/rewrite."""
        workspace = tmp_path / "workspace"
        workspace.mkdir()
        self._write(workspace / "assets" / "inplace.png")
        (workspace / "guide.qmd.md").write_text(
            "# Guide [[guide]]\n\n![In place](assets/inplace.png)\n"
        )
        tmpdir = tmp_path / "build"
        tmpdir.mkdir()

        convert_workspace(workspace, tmpdir, mock_workspace_db, {})

        out = (tmpdir / "docs" / "guide.md").read_text()
        assert "![In place](assets/inplace.png)" in out
        assert not (tmpdir / "docs" / "assets" / "qmdc").exists()


class TestBuildFrontMatter:
    """Tests for _build_front_matter helper."""

    def test_includes_graph_sidebar(self):
        """Test that front matter includes _graph_sidebar key."""
        ctx = GraphContext(
            workspace_label="My Project",
            namespace_label="Storage",
            file_label="Tables",
            links_to=[],
            linked_from=[],
            siblings=[],
            toc=[],
        )
        result = _build_front_matter(ctx, {}, "storage/tables.qmd.md")

        assert result.startswith("---\n")
        assert result.endswith("---\n")
        # Parse the YAML
        yaml_content = result.strip("---\n")
        parsed = yaml.safe_load(yaml_content)
        assert "_graph_sidebar" in parsed
        assert parsed["_graph_sidebar"]["workspace_label"] == "My Project"
        assert parsed["_graph_sidebar"]["namespace_label"] == "Storage"
        assert parsed["_graph_sidebar"]["file_label"] == "Tables"

    def test_includes_semantic_hints(self):
        """Test that front matter includes _semantic_hints when hints exist."""
        ctx = GraphContext(
            workspace_label="WS",
            namespace_label=None,
            file_label="File",
            links_to=[],
            linked_from=[],
            siblings=[],
            toc=[],
        )
        hints = {
            "users": [
                HintEntry(label="Orders", kind="Table", file="storage/tables.qmd.md", score=0.85)
            ]
        }
        result = _build_front_matter(ctx, hints, "readme.qmd.md")

        parsed = yaml.safe_load(result.strip("---\n"))
        assert "_semantic_hints" in parsed
        assert "users" in parsed["_semantic_hints"]
        assert parsed["_semantic_hints"]["users"][0]["label"] == "Orders"
        assert parsed["_semantic_hints"]["users"][0]["score"] == 0.85

    def test_no_hints_omits_semantic_hints_key(self):
        """Test that _semantic_hints is omitted when no hints exist."""
        ctx = GraphContext(
            workspace_label="WS",
            namespace_label=None,
            file_label="File",
            links_to=[],
            linked_from=[],
            siblings=[],
            toc=[],
        )
        result = _build_front_matter(ctx, {}, "readme.qmd.md")

        parsed = yaml.safe_load(result.strip("---\n"))
        assert "_semantic_hints" not in parsed

    def test_links_to_includes_href(self):
        """Test that links_to entries include computed href."""
        ctx = GraphContext(
            workspace_label="WS",
            namespace_label=None,
            file_label="File",
            links_to=[
                EdgeItem(
                    edge_type="depends",
                    obj_id="auth",
                    label="Auth",
                    kind="Service",
                    file="api/auth.qmd.md",
                )
            ],
            linked_from=[],
            siblings=[],
            toc=[],
        )
        result = _build_front_matter(ctx, {}, "storage/tables.qmd.md")

        parsed = yaml.safe_load(result.strip("---\n"))
        link = parsed["_graph_sidebar"]["links_to"][0]
        # Directory-style URL: from storage/tables/ to api/auth/#auth
        assert link["href"] == "../../api/auth/#auth"

    def test_siblings_include_href(self):
        """Test that siblings include computed href."""
        ctx = GraphContext(
            workspace_label="WS",
            namespace_label=None,
            file_label="Tables",
            links_to=[],
            linked_from=[],
            siblings=[
                SiblingItem(file="storage/indexes.qmd.md", label="Indexes", is_current=False),
                SiblingItem(file="storage/tables.qmd.md", label="Tables", is_current=True),
            ],
            toc=[],
        )
        result = _build_front_matter(ctx, {}, "storage/tables.qmd.md")

        parsed = yaml.safe_load(result.strip("---\n"))
        siblings = parsed["_graph_sidebar"]["siblings"]
        # Directory-style URL: from storage/tables/ to storage/indexes/
        assert siblings[0]["href"] == "../indexes/"
        assert siblings[0]["file"] == "storage/indexes.md"


class TestComputeHref:
    """Tests for _compute_href helper.

    _compute_href produces directory-style URLs for use in sidebar templates.
    MkDocs with use_directory_urls=true serves:
    - storage/tables.md → /storage/tables/ (directory)
    - index.md → / (root)
    - storage/index.md → /storage/ (parent directory)
    """

    def test_same_directory(self):
        """Test href for files in the same directory."""
        result = _compute_href("storage/tables.qmd.md", "storage/indexes.qmd.md", "idx")
        # From storage/tables/ to storage/indexes/#idx
        assert result == "../indexes/#idx"

    def test_cross_directory(self):
        """Test href for files in different directories."""
        result = _compute_href("storage/tables.qmd.md", "api/endpoints.qmd.md", "get_users")
        # From storage/tables/ to api/endpoints/#get_users
        assert result == "../../api/endpoints/#get_users"

    def test_no_anchor(self):
        """Test href without anchor."""
        result = _compute_href("storage/tables.qmd.md", "storage/indexes.qmd.md", None)
        # From storage/tables/ to storage/indexes/
        assert result == "../indexes/"

    def test_root_to_subdirectory(self):
        """Test href from root file to subdirectory file."""
        result = _compute_href("readme.qmd.md", "storage/tables.qmd.md", "users")
        # readme.qmd.md → index.md → served at /
        # From / to storage/tables/#users
        assert result == "storage/tables/#users"

    def test_subdirectory_to_root(self):
        """Test href from subdirectory file to root file."""
        result = _compute_href("storage/tables.qmd.md", "readme.qmd.md", "project")
        # From storage/tables/ to / (root index)
        assert result == "../../#project"
