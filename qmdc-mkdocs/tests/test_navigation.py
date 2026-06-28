"""Unit tests for navigation tree generation."""

from qmdc_mkdocs.navigation import (
    _derive_page_title,
    _derive_section_title,
    _title_from_dirname,
    generate_nav,
)


class TestGenerateNav:
    """Tests for the full generate_nav function."""

    def test_basic_nav_structure(self, mock_workspace_db):
        """Test that nav groups files by directory with correct structure."""
        nav = generate_nav(mock_workspace_db)

        # Should have root-level items and section groups
        assert isinstance(nav, list)
        assert len(nav) > 0

    def test_root_files_at_top_level(self, mock_workspace_db):
        """Root files (no directory) appear as top-level nav items."""
        nav = generate_nav(mock_workspace_db)

        # readme.qmd.md is in root → should be a top-level dict
        root_items = [item for item in nav if isinstance(item, dict) and len(item) == 1]
        # The first item should be the root readme (converted to index.md)
        first_key = list(nav[0].keys())[0]
        first_val = list(nav[0].values())[0]
        assert first_val == "index.md"

    def test_namespace_sections_use_label(self, mock_workspace_db):
        """Namespace directories use __label as section title."""
        nav = generate_nav(mock_workspace_db)

        # Find section titles
        section_titles = []
        for item in nav:
            if isinstance(item, dict):
                key = list(item.keys())[0]
                val = list(item.values())[0]
                if isinstance(val, list):
                    section_titles.append(key)

        # "storage" namespace has __label "Storage Layer"
        assert "Storage Layer" in section_titles
        # "api" namespace has __label "API Layer"
        assert "API Layer" in section_titles

    def test_readme_first_in_sections(self, mock_workspace_db):
        """readme.qmd.md files appear first within each section."""
        nav = generate_nav(mock_workspace_db)

        for item in nav:
            if isinstance(item, dict):
                val = list(item.values())[0]
                if isinstance(val, list) and len(val) > 1:
                    # First item in section should be the readme (→ index.md)
                    first_page_path = list(val[0].values())[0]
                    assert "index.md" in first_page_path

    def test_qmdc_extension_replaced(self, mock_workspace_db):
        """Output paths use .md instead of .qmd.md."""
        nav = generate_nav(mock_workspace_db)

        def check_paths(items):
            for item in items:
                if isinstance(item, dict):
                    for key, val in item.items():
                        if isinstance(val, str):
                            assert ".qmd.md" not in val
                            assert val.endswith(".md")
                        elif isinstance(val, list):
                            check_paths(val)

        check_paths(nav)

    def test_sections_sorted_alphabetically(self, mock_workspace_db):
        """Sections are sorted alphabetically by directory name."""
        nav = generate_nav(mock_workspace_db)

        section_titles = []
        for item in nav:
            if isinstance(item, dict):
                val = list(item.values())[0]
                if isinstance(val, list):
                    section_titles.append(list(item.keys())[0])

        # API Layer comes before Storage Layer alphabetically
        assert section_titles.index("API Layer") < section_titles.index("Storage Layer")

    def test_page_titles_from_labels(self, mock_workspace_db):
        """Pages use __label from top-level objects when available."""
        nav = generate_nav(mock_workspace_db)

        # Find the storage section
        for item in nav:
            if isinstance(item, dict):
                key = list(item.keys())[0]
                if key == "Storage Layer":
                    pages = list(item.values())[0]
                    page_titles = [list(p.keys())[0] for p in pages]
                    # "Storage Layer" is the namespace label for readme
                    # The tables file has level-2 objects, not level-1
                    # so it should derive from filename
                    break


class TestDeriveSectionTitle:
    """Tests for _derive_section_title."""

    def test_uses_namespace_label(self):
        """Returns namespace __label when available."""
        namespaces = {"storage": "Storage Layer", "api": "API Layer"}
        assert _derive_section_title("storage", namespaces) == "Storage Layer"
        assert _derive_section_title("api", namespaces) == "API Layer"

    def test_falls_back_to_dirname(self):
        """Falls back to title-cased directory name when no namespace label."""
        namespaces = {}
        assert _derive_section_title("my-docs", namespaces) == "My Docs"

    def test_handles_nested_directory(self):
        """Uses first path component for namespace lookup."""
        namespaces = {"storage": "Storage Layer"}
        assert _derive_section_title("storage/sub", namespaces) == "Storage Layer"

    def test_handles_none_label(self):
        """Falls back when namespace label is None."""
        namespaces = {"storage": None}
        assert _derive_section_title("storage", namespaces) == "Storage"

    def test_handles_empty_label(self):
        """Falls back when namespace label is empty string."""
        namespaces = {"storage": ""}
        assert _derive_section_title("storage", namespaces) == "Storage"


class TestDerivePageTitle:
    """Tests for _derive_page_title."""

    def test_uses_file_label(self):
        """Returns file label when available."""
        labels = {"storage/tables.qmd.md": "Database Tables"}
        assert _derive_page_title("storage/tables.qmd.md", labels) == "Database Tables"

    def test_falls_back_to_filename(self):
        """Derives title from filename when no label."""
        labels = {}
        assert _derive_page_title("storage/tables.qmd.md", labels) == "Tables"

    def test_handles_hyphens_and_underscores(self):
        """Replaces hyphens and underscores with spaces."""
        labels = {}
        assert _derive_page_title("my-cool_page.qmd.md", labels) == "My Cool Page"

    def test_strips_qmdc_extension(self):
        """Strips .qmd from stem before title-casing."""
        labels = {}
        # stem of "readme.qmd.md" is "readme.qmd", then strip ".qmd" → "readme"
        assert _derive_page_title("readme.qmd.md", labels) == "Readme"


class TestTitleFromDirname:
    """Tests for _title_from_dirname."""

    def test_simple_name(self):
        assert _title_from_dirname("storage") == "Storage"

    def test_hyphenated(self):
        assert _title_from_dirname("my-docs") == "My Docs"

    def test_underscored(self):
        assert _title_from_dirname("my_docs") == "My Docs"

    def test_mixed(self):
        assert _title_from_dirname("my-cool_project") == "My Cool Project"
