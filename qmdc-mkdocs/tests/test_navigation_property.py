"""Property-based tests for navigation tree structure.

**Validates: Requirements 2.3, 9.1, 9.2, 9.3, 9.4**

Property 8: Navigation Tree Structure
For any workspace with namespaces, the generated navigation SHALL map each
namespace directory to a section titled with the namespace's __label (or
directory name title-cased if no label), with pages ordered readme-first then
alphabetically, and each page titled with its top-level object's __label (or
filename title-cased if no label).

Sub-properties tested:
- Every .qmd.md file in the workspace appears exactly once in the nav (as a .md path)
- Root files appear at the top level (not nested in sections)
- Files within namespace directories appear inside section groups
- Section titles match namespace __label (or directory name title-cased if no label)
- Page titles match top-level object __label (or filename title-cased if no label)
- Readme files appear first within their section
- Non-readme pages are ordered alphabetically after readme
- All output paths end with .md (not .qmd.md)
- Section titles are non-empty strings
"""

from __future__ import annotations

import sqlite3

import hypothesis.strategies as st
from hypothesis import given, settings

from qmdc_mkdocs.navigation import generate_nav


# --- Path conversion helper (matches _qmdc_to_nav_path logic) ---


def _to_nav_path(source_file: str) -> str:
    """Convert .qmd.md path to nav output path (readme → index)."""
    md = source_file.replace(".qmd.md", ".md")
    if md.endswith("/readme.md"):
        return md.replace("/readme.md", "/index.md")
    if md == "readme.md":
        return "index.md"
    return md


# --- Helpers to extract info from nav structure ---


def extract_all_paths(nav: list) -> list[str]:
    """Extract all file paths from a nav structure (recursively)."""
    paths = []
    for item in nav:
        if isinstance(item, dict):
            for _title, value in item.items():
                if isinstance(value, str):
                    paths.append(value)
                elif isinstance(value, list):
                    paths.extend(extract_all_paths(value))
    return paths


def extract_top_level_paths(nav: list) -> list[str]:
    """Extract paths that are at the top level (not nested in sections)."""
    paths = []
    for item in nav:
        if isinstance(item, dict):
            for _title, value in item.items():
                if isinstance(value, str):
                    paths.append(value)
    return paths


def extract_section_items(nav: list) -> list[tuple[str, list[str]]]:
    """Extract list of (section_title, paths) tuples from the nav.

    Uses a list of tuples instead of a dict to handle duplicate section titles.
    """
    sections = []
    for item in nav:
        if isinstance(item, dict):
            for title, value in item.items():
                if isinstance(value, list):
                    sections.append((title, extract_all_paths(value)))
    return sections


def extract_all_section_paths(nav: list) -> list[str]:
    """Extract all paths that are inside any section (not top-level)."""
    paths = []
    for _title, section_paths in extract_section_items(nav):
        paths.extend(section_paths)
    return paths


def extract_section_titles(nav: list) -> list[str]:
    """Extract all section titles from the nav."""
    titles = []
    for item in nav:
        if isinstance(item, dict):
            for title, value in item.items():
                if isinstance(value, list):
                    titles.append(title)
    return titles


def extract_section_title_for_path(nav: list, path: str) -> str | None:
    """Find the section title that contains a given path."""
    for item in nav:
        if isinstance(item, dict):
            for title, value in item.items():
                if isinstance(value, list):
                    section_paths = extract_all_paths(value)
                    if path in section_paths:
                        return title
    return None


def extract_page_titles(nav: list) -> dict[str, str]:
    """Extract a mapping of path -> page title from the nav (recursively)."""
    titles = {}
    for item in nav:
        if isinstance(item, dict):
            for title, value in item.items():
                if isinstance(value, str):
                    titles[value] = title
                elif isinstance(value, list):
                    titles.update(extract_page_titles(value))
    return titles


def extract_section_paths_ordered(nav: list) -> dict[str, list[str]]:
    """Extract ordered paths per section title.

    Returns dict of section_title -> list of paths in order.
    """
    sections = {}
    for item in nav:
        if isinstance(item, dict):
            for title, value in item.items():
                if isinstance(value, list):
                    paths = []
                    for sub_item in value:
                        if isinstance(sub_item, dict):
                            for _t, v in sub_item.items():
                                if isinstance(v, str):
                                    paths.append(v)
                    sections[title] = paths
    return sections


# --- Mock DB builder for Hypothesis ---


def build_mock_db(
    root_files: list[str],
    namespaces: dict[str, tuple[str, list[str]]],
) -> object:
    """Build a mock WorkspaceDB from generated workspace structure.

    Args:
        root_files: list of root-level .qmd.md filenames
        namespaces: dict of ns_id -> (label, list of filenames within that namespace)
    """
    conn = sqlite3.connect(":memory:")
    conn.execute("PRAGMA journal_mode = OFF")
    conn.execute("PRAGMA synchronous = OFF")

    columns = [
        "__global_id",
        "__id",
        "__kind",
        "__label",
        "__file",
        "__line",
        "__level",
        "__workspace",
        "__namespace",
        "data",
    ]
    col_defs = ", ".join(f'"{c}" TEXT' for c in columns)
    conn.execute(f"CREATE TABLE objects ({col_defs})")

    edge_columns = ["source_id", "source_field", "target_id", "edge_type", "__workspace"]
    edge_col_defs = ", ".join(f'"{c}" TEXT' for c in edge_columns)
    conn.execute(f"CREATE TABLE edges ({edge_col_defs})")

    rows = []

    # Workspace root object
    rows.append([
        "ws", "ws", "__Workspace", "Test Workspace",
        "readme.qmd.md", "1", "1", None, None, "{}",
    ])

    # Root files (each gets a level-1 object)
    for f in root_files:
        obj_id = f.replace(".qmd.md", "").replace("/", "_")
        label = obj_id.replace("_", " ").replace("-", " ").title()
        rows.append([
            obj_id, obj_id, "Doc", label,
            f, "1", "1", "ws", None, "{}",
        ])

    # Namespace objects and their files
    for ns_id, (ns_label, ns_files) in namespaces.items():
        # Namespace object
        rows.append([
            ns_id, ns_id, "__Namespace", ns_label,
            f"{ns_id}/readme.qmd.md", "1", "1", "ws", None, "{}",
        ])
        # Files within namespace
        for f in ns_files:
            file_path = f"{ns_id}/{f}"
            obj_id = f"{ns_id}::{f.replace('.qmd.md', '')}"
            label = f.replace(".qmd.md", "").replace("_", " ").replace("-", " ").title()
            rows.append([
                obj_id, obj_id, "Page", label,
                file_path, "3", "1", "ws", ns_id, "{}",
            ])

    placeholders = ", ".join("?" * len(columns))
    conn.executemany(f"INSERT INTO objects VALUES ({placeholders})", rows)

    # Create indexes
    conn.executescript("""
        CREATE INDEX idx_obj_file ON objects(__file);
        CREATE INDEX idx_obj_id ON objects(__id);
        CREATE INDEX idx_obj_gid ON objects(__global_id);
        CREATE INDEX idx_obj_kind ON objects(__kind);
        CREATE INDEX idx_obj_ns ON objects(__namespace);
        CREATE INDEX idx_obj_level ON objects(__level);
        CREATE INDEX idx_edge_src ON edges(source_id);
        CREATE INDEX idx_edge_tgt ON edges(target_id);
    """)

    class MockDB:
        def __init__(self, connection):
            self.conn = connection

        def query(self, sql, params=()):
            cur = self.conn.execute(sql, params)
            cols = [d[0] for d in cur.description]
            return [dict(zip(cols, row)) for row in cur.fetchall()]

        def close(self):
            self.conn.close()

    return MockDB(conn)


# --- Hypothesis strategies ---

# Valid filename characters (lowercase, no dots except .qmd.md suffix)
_filename_chars = st.sampled_from("abcdefghijklmnopqrstuvwxyz0123456789-_")
_filename_stem = st.text(_filename_chars, min_size=1, max_size=15).filter(
    lambda s: s[0].isalpha()
)

# Generate a .qmd.md filename (not readme or index — those are handled separately)
_qmdc_filename = _filename_stem.map(lambda s: f"{s}.qmd.md").filter(
    lambda f: f != "readme.qmd.md" and f != "index.qmd.md"
)

# Namespace ID: lowercase alpha with optional hyphens/underscores
_ns_id = st.text(
    st.sampled_from("abcdefghijklmnopqrstuvwxyz-_"),
    min_size=2,
    max_size=10,
).filter(lambda s: s[0].isalpha() and s[-1].isalpha())

# Namespace label: non-empty title string
_ns_label = st.text(
    st.sampled_from("ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz "),
    min_size=1,
    max_size=20,
).map(str.strip).filter(lambda s: len(s) > 0)


@st.composite
def workspace_structure(draw):
    """Generate a valid workspace structure for testing navigation.

    Returns (root_files, namespaces) where:
    - root_files: list of root-level .qmd.md filenames (always includes readme.qmd.md)
    - namespaces: dict of ns_id -> (label, files) where files always includes readme.qmd.md
    """
    # Root files: always have readme.qmd.md, plus 0-3 others
    extra_root = draw(st.lists(_qmdc_filename, min_size=0, max_size=3, unique=True))
    root_files = ["readme.qmd.md"] + extra_root

    # Namespaces: 1-4 namespaces, each with readme + 0-4 other files
    ns_count = draw(st.integers(min_value=1, max_value=4))
    ns_ids = draw(
        st.lists(_ns_id, min_size=ns_count, max_size=ns_count, unique=True)
    )

    namespaces = {}
    for ns_id in ns_ids:
        label = draw(_ns_label)
        extra_files = draw(st.lists(_qmdc_filename, min_size=0, max_size=4, unique=True))
        ns_files = ["readme.qmd.md"] + extra_files
        namespaces[ns_id] = (label, ns_files)

    return root_files, namespaces


# --- Property tests with fixture-based DB ---


class TestNavPropertyWithFixture:
    """Property tests using the mock_workspace_db fixture."""

    def test_all_files_appear_exactly_once(self, mock_workspace_db):
        """Every .qmd.md file appears exactly once in the nav as a .md path.

        **Validates: Requirements 9.1, 9.2**
        """
        nav = generate_nav(mock_workspace_db)
        all_paths = extract_all_paths(nav)

        # Get all distinct files from the DB
        files = [
            r["__file"]
            for r in mock_workspace_db.query("SELECT DISTINCT __file FROM objects")
        ]
        expected_paths = sorted(_to_nav_path(f) for f in files)

        assert sorted(all_paths) == expected_paths

    def test_root_files_not_nested(self, mock_workspace_db):
        """Root files appear at the top level, not inside sections.

        **Validates: Requirements 9.2**
        """
        nav = generate_nav(mock_workspace_db)
        top_paths = extract_top_level_paths(nav)

        # readme.md is a root file (readme.qmd.md → index.md)
        assert "index.md" in top_paths

    def test_namespace_files_in_sections(self, mock_workspace_db):
        """Files within namespace directories appear inside section groups.

        **Validates: Requirements 9.2, 9.3**
        """
        nav = generate_nav(mock_workspace_db)
        all_section_paths = extract_all_section_paths(nav)

        assert "storage/index.md" in all_section_paths
        assert "storage/tables.md" in all_section_paths
        assert "api/index.md" in all_section_paths
        assert "api/endpoints.md" in all_section_paths

    def test_readme_first_in_sections(self, mock_workspace_db):
        """Readme files appear first within their section.

        **Validates: Requirements 9.4**
        """
        nav = generate_nav(mock_workspace_db)
        sections = extract_section_items(nav)

        for _title, paths in sections:
            if len(paths) > 0:
                # First path should be a readme
                assert paths[0].endswith("index.md")

    def test_all_paths_end_with_md(self, mock_workspace_db):
        """All output paths end with .md (not .qmd.md).

        **Validates: Requirements 9.2**
        """
        nav = generate_nav(mock_workspace_db)
        all_paths = extract_all_paths(nav)

        for path in all_paths:
            assert path.endswith(".md"), f"Path does not end with .md: {path}"
            assert ".qmd.md" not in path, f"Path contains .qmd.md: {path}"

    def test_section_titles_non_empty(self, mock_workspace_db):
        """Section titles are non-empty strings.

        **Validates: Requirements 9.3**
        """
        nav = generate_nav(mock_workspace_db)
        titles = extract_section_titles(nav)

        for title in titles:
            assert isinstance(title, str)
            assert len(title.strip()) > 0

    def test_section_titles_match_namespace_labels(self, mock_workspace_db):
        """Section titles match namespace __label values.

        **Validates: Requirements 2.3, 9.3**
        """
        nav = generate_nav(mock_workspace_db)

        # The sample workspace has namespaces "storage" (label "Storage Layer")
        # and "api" (label "API Layer")
        storage_title = extract_section_title_for_path(nav, "storage/index.md")
        api_title = extract_section_title_for_path(nav, "api/index.md")

        assert storage_title == "Storage Layer"
        assert api_title == "API Layer"

    def test_page_titles_match_object_labels(self, mock_workspace_db):
        """Page titles match top-level object __label values.

        **Validates: Requirements 9.3, 9.4**
        """
        nav = generate_nav(mock_workspace_db)
        page_titles = extract_page_titles(nav)

        # The sample workspace has known labels from conftest.py
        # storage/tables.qmd.md has top-level object "Tables" (__Document)
        # api/endpoints.qmd.md has top-level object "Endpoints" (__Document)
        # But __Document is a system type (starts with __), so it may be filtered.
        # The actual labels come from the first non-system level-1 object.
        # Let's verify what the DB actually has:
        file_labels = {
            r["__file"]: r["__label"]
            for r in mock_workspace_db.query(
                "SELECT __file, __label FROM objects "
                "WHERE CAST(__level AS INTEGER) = 1 "
                "AND __kind NOT GLOB '__*' AND __label IS NOT NULL"
            )
        }

        for file_path, expected_label in file_labels.items():
            md_path = _to_nav_path(file_path)
            if md_path in page_titles:
                assert page_titles[md_path] == expected_label, (
                    f"Page title for {md_path} is '{page_titles[md_path]}', "
                    f"expected '{expected_label}'"
                )

    def test_pages_alphabetical_after_readme(self, mock_workspace_db):
        """Non-readme pages are ordered alphabetically after readme.

        **Validates: Requirements 9.4**
        """
        nav = generate_nav(mock_workspace_db)
        section_paths = extract_section_paths_ordered(nav)

        for title, paths in section_paths.items():
            if len(paths) <= 1:
                continue
            # First should be readme
            assert paths[0].endswith("index.md"), (
                f"Section '{title}': first item is {paths[0]}, expected index.md"
            )
            # Remaining should be alphabetically sorted
            non_readme = paths[1:]
            assert non_readme == sorted(non_readme), (
                f"Section '{title}': non-readme pages not alphabetical: {non_readme}"
            )


# --- Property tests with Hypothesis-generated workspaces ---


class TestNavPropertyHypothesis:
    """Property tests using Hypothesis-generated workspace structures."""

    @given(ws=workspace_structure())
    @settings(max_examples=50, deadline=2000)
    def test_all_files_appear_exactly_once(self, ws):
        """Every .qmd.md file appears exactly once in the nav as a .md path.

        **Validates: Requirements 9.1, 9.2**
        """
        root_files, namespaces = ws
        db = build_mock_db(root_files, namespaces)
        try:
            nav = generate_nav(db)
            all_paths = extract_all_paths(nav)

            # Build expected paths (readme.qmd.md → index.md per MkDocs convention)
            expected = set()
            for f in root_files:
                expected.add(_to_nav_path(f))
            for ns_id, (_label, ns_files) in namespaces.items():
                for f in ns_files:
                    expected.add(_to_nav_path(f"{ns_id}/{f}"))

            assert set(all_paths) == expected, (
                f"Mismatch: nav has {set(all_paths) - expected} extra, "
                f"missing {expected - set(all_paths)}"
            )
            # Each path appears exactly once
            assert len(all_paths) == len(set(all_paths)), "Duplicate paths in nav"
        finally:
            db.close()

    @given(ws=workspace_structure())
    @settings(max_examples=50, deadline=2000)
    def test_root_files_at_top_level(self, ws):
        """Root files appear at the top level (not nested in sections).

        **Validates: Requirements 9.2**
        """
        root_files, namespaces = ws
        db = build_mock_db(root_files, namespaces)
        try:
            nav = generate_nav(db)
            top_paths = extract_top_level_paths(nav)
            section_paths = extract_all_section_paths(nav)

            # All root files should be at top level
            for f in root_files:
                md_path = _to_nav_path(f)
                assert md_path in top_paths, (
                    f"Root file {md_path} not at top level"
                )
                assert md_path not in section_paths, (
                    f"Root file {md_path} found inside a section"
                )
        finally:
            db.close()

    @given(ws=workspace_structure())
    @settings(max_examples=50, deadline=2000)
    def test_namespace_files_in_sections(self, ws):
        """Files within namespace directories appear inside section groups.

        **Validates: Requirements 9.2, 9.3**
        """
        root_files, namespaces = ws
        db = build_mock_db(root_files, namespaces)
        try:
            nav = generate_nav(db)
            top_paths = extract_top_level_paths(nav)
            section_paths = extract_all_section_paths(nav)

            # All namespace files should be in sections, not at top level
            for ns_id, (_label, ns_files) in namespaces.items():
                for f in ns_files:
                    md_path = _to_nav_path(f"{ns_id}/{f}")
                    assert md_path in section_paths, (
                        f"Namespace file {md_path} not in any section"
                    )
                    assert md_path not in top_paths, (
                        f"Namespace file {md_path} found at top level"
                    )
        finally:
            db.close()

    @given(ws=workspace_structure())
    @settings(max_examples=50, deadline=2000)
    def test_readme_first_in_sections(self, ws):
        """Readme files appear first within their section.

        **Validates: Requirements 9.4**
        """
        root_files, namespaces = ws
        db = build_mock_db(root_files, namespaces)
        try:
            nav = generate_nav(db)
            sections = extract_section_items(nav)

            for title, paths in sections:
                if len(paths) > 0:
                    # First path in each section should be a readme
                    assert paths[0].endswith("index.md"), (
                        f"Section '{title}' first item is {paths[0]}, expected index.md"
                    )
        finally:
            db.close()

    @given(ws=workspace_structure())
    @settings(max_examples=50, deadline=2000)
    def test_all_paths_end_with_md(self, ws):
        """All output paths end with .md (not .qmd.md).

        **Validates: Requirements 9.2**
        """
        root_files, namespaces = ws
        db = build_mock_db(root_files, namespaces)
        try:
            nav = generate_nav(db)
            all_paths = extract_all_paths(nav)

            for path in all_paths:
                assert path.endswith(".md"), f"Path does not end with .md: {path}"
                assert ".qmd.md" not in path, f"Path contains .qmd.md: {path}"
        finally:
            db.close()

    @given(ws=workspace_structure())
    @settings(max_examples=50, deadline=2000)
    def test_section_titles_non_empty(self, ws):
        """Section titles are non-empty strings.

        **Validates: Requirements 9.3**
        """
        root_files, namespaces = ws
        db = build_mock_db(root_files, namespaces)
        try:
            nav = generate_nav(db)
            titles = extract_section_titles(nav)

            assert len(titles) > 0, "Expected at least one section"
            for title in titles:
                assert isinstance(title, str), f"Title is not a string: {title!r}"
                assert len(title.strip()) > 0, f"Title is empty or whitespace: {title!r}"
        finally:
            db.close()

    @given(ws=workspace_structure())
    @settings(max_examples=50, deadline=2000)
    def test_section_titles_match_namespace_labels(self, ws):
        """Section titles match namespace __label (or directory name title-cased).

        **Validates: Requirements 2.3, 9.3**
        """
        root_files, namespaces = ws
        db = build_mock_db(root_files, namespaces)
        try:
            nav = generate_nav(db)

            for ns_id, (ns_label, ns_files) in namespaces.items():
                # Find the section containing this namespace's readme
                readme_path = f"{ns_id}/index.md"
                section_title = extract_section_title_for_path(nav, readme_path)

                assert section_title is not None, (
                    f"No section found containing {readme_path}"
                )

                # If namespace has a label, section title should match it
                if ns_label and ns_label.strip():
                    assert section_title == ns_label, (
                        f"Section title for namespace '{ns_id}' is '{section_title}', "
                        f"expected namespace label '{ns_label}'"
                    )
                else:
                    # Fallback: directory name title-cased
                    expected = ns_id.replace("-", " ").replace("_", " ").title()
                    assert section_title == expected, (
                        f"Section title for namespace '{ns_id}' is '{section_title}', "
                        f"expected derived title '{expected}'"
                    )
        finally:
            db.close()

    @given(ws=workspace_structure())
    @settings(max_examples=50, deadline=2000)
    def test_page_titles_match_object_labels(self, ws):
        """Page titles match top-level object __label (or filename title-cased).

        **Validates: Requirements 9.3, 9.4**
        """
        root_files, namespaces = ws
        db = build_mock_db(root_files, namespaces)
        try:
            nav = generate_nav(db)
            page_titles = extract_page_titles(nav)

            # Check namespace files have correct titles
            for ns_id, (_label, ns_files) in namespaces.items():
                for f in ns_files:
                    md_path = _to_nav_path(f"{ns_id}/{f}")
                    assert md_path in page_titles, (
                        f"Path {md_path} not found in nav page titles"
                    )
                    actual_title = page_titles[md_path]

                    # The mock DB assigns labels based on filename stem title-cased
                    # (matching the build_mock_db logic above)
                    file_path = f"{ns_id}/{f}"
                    # Query the DB for the expected label
                    rows = db.query(
                        "SELECT __label FROM objects "
                        "WHERE __file = ? AND CAST(__level AS INTEGER) = 1 "
                        "AND __kind NOT GLOB '__*' AND __label IS NOT NULL",
                        params=(file_path,),
                    )
                    if rows:
                        expected_title = rows[0]["__label"]
                        assert actual_title == expected_title, (
                            f"Page title for {md_path} is '{actual_title}', "
                            f"expected '{expected_title}' from DB"
                        )
        finally:
            db.close()

    @given(ws=workspace_structure())
    @settings(max_examples=50, deadline=2000)
    def test_pages_alphabetical_after_readme(self, ws):
        """Non-readme pages are ordered alphabetically after readme.

        **Validates: Requirements 9.4**
        """
        root_files, namespaces = ws
        db = build_mock_db(root_files, namespaces)
        try:
            nav = generate_nav(db)
            section_paths = extract_section_paths_ordered(nav)

            for title, paths in section_paths.items():
                if len(paths) <= 1:
                    continue
                # First should be readme
                assert paths[0].endswith("index.md"), (
                    f"Section '{title}': first item is {paths[0]}, expected index.md"
                )
                # Remaining should be alphabetically sorted by path
                non_readme = paths[1:]
                assert non_readme == sorted(non_readme), (
                    f"Section '{title}': non-readme pages not alphabetical: {non_readme}"
                )
        finally:
            db.close()


class TestNavPropertyFallbacks:
    """Tests for fallback behavior when labels are missing."""

    def test_section_title_fallback_no_label(self):
        """When namespace has no __label, section title is directory name title-cased.

        **Validates: Requirements 9.3**
        """
        # Build a mock DB where namespace has NULL/empty label
        db = build_mock_db(
            root_files=["readme.qmd.md"],
            namespaces={"my-service": ("", ["readme.qmd.md", "config.qmd.md"])},
        )
        try:
            nav = generate_nav(db)
            section_title = extract_section_title_for_path(nav, "my-service/index.md")
            # "my-service" -> "My Service" (hyphens to spaces, title-cased)
            assert section_title == "My Service"
        finally:
            db.close()

    def test_page_title_fallback_no_label(self):
        """When page has no top-level object __label, title is filename title-cased.

        **Validates: Requirements 9.4**
        """
        # Build a mock DB where files have no level-1 non-system objects
        conn = sqlite3.connect(":memory:")
        conn.execute("PRAGMA journal_mode = OFF")
        columns = [
            "__global_id", "__id", "__kind", "__label", "__file",
            "__line", "__level", "__workspace", "__namespace", "data",
        ]
        col_defs = ", ".join(f'"{c}" TEXT' for c in columns)
        conn.execute(f"CREATE TABLE objects ({col_defs})")
        conn.execute(
            "CREATE TABLE edges (source_id TEXT, source_field TEXT, "
            "target_id TEXT, edge_type TEXT, __workspace TEXT)"
        )

        rows = [
            ["ws", "ws", "__Workspace", "Test", "readme.qmd.md", "1", "1", None, None, "{}"],
            ["ns1", "ns1", "__Namespace", "Docs", "docs/readme.qmd.md", "1", "1", "ws", None, "{}"],
            # A file with only a level-2 object (no level-1 non-system object)
            ["sub_obj", "sub_obj", "Section", "Some Section",
             "docs/my-feature.qmd.md", "5", "2", "ws", "ns1", "{}"],
        ]
        placeholders = ", ".join("?" * len(columns))
        conn.executemany(f"INSERT INTO objects VALUES ({placeholders})", rows)
        conn.executescript("""
            CREATE INDEX idx_obj_file ON objects(__file);
            CREATE INDEX idx_obj_id ON objects(__id);
            CREATE INDEX idx_obj_gid ON objects(__global_id);
            CREATE INDEX idx_obj_kind ON objects(__kind);
            CREATE INDEX idx_obj_ns ON objects(__namespace);
            CREATE INDEX idx_obj_level ON objects(__level);
            CREATE INDEX idx_edge_src ON edges(source_id);
            CREATE INDEX idx_edge_tgt ON edges(target_id);
        """)

        class MockDB:
            def __init__(self, connection):
                self.conn = connection

            def query(self, sql, params=()):
                cur = self.conn.execute(sql, params)
                cols = [d[0] for d in cur.description]
                return [dict(zip(cols, row)) for row in cur.fetchall()]

            def close(self):
                self.conn.close()

        db = MockDB(conn)
        try:
            nav = generate_nav(db)
            page_titles = extract_page_titles(nav)

            # "docs/my-feature.qmd.md" -> "docs/my-feature.md"
            # Since no level-1 non-system object exists, title should be
            # derived from filename: "my-feature" -> "My Feature"
            assert "docs/my-feature.md" in page_titles
            assert page_titles["docs/my-feature.md"] == "My Feature"
        finally:
            db.close()
