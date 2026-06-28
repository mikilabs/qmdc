"""Property-based tests for graph sidebar computation.

**Validates: Requirements 3.8, 6.1, 6.2, 6.3, 6.4**

Property 6: Breadcrumb Hierarchy
For any page in a namespace, the breadcrumb trail SHALL contain exactly three
segments: workspace label, namespace label (or derived title), and file label —
in that order. For root-level pages (no namespace), the breadcrumb SHALL contain
workspace label and file label.

Property 7: Sibling Listing Completeness
For any page in a namespace directory, the siblings section SHALL list all files
in the same directory (including the current file marked with is_current=True),
each with a non-empty label, and no file from a different directory.
"""

from __future__ import annotations

import sqlite3
from pathlib import PurePosixPath

import hypothesis.strategies as st
from hypothesis import HealthCheck, given, settings

from qmdc_mkdocs.graph import _get_siblings, compute_graph_context


# --- Source files available in the mock_workspace_db fixture ---

# These match the SAMPLE_OBJECTS_JSON in conftest.py
FIXTURE_SOURCE_FILES = [
    "readme.qmd.md",
    "storage/readme.qmd.md",
    "storage/tables.qmd.md",
    "api/readme.qmd.md",
    "api/endpoints.qmd.md",
]

# Strategy for selecting a source file from the fixture
source_file_strategy = st.sampled_from(FIXTURE_SOURCE_FILES)


# --- Mock DB builder for Hypothesis ---


def build_sibling_mock_db(
    directories: dict[str, list[str]],
) -> object:
    """Build a mock WorkspaceDB with files distributed across directories.

    Args:
        directories: dict of directory_path -> list of filenames in that directory.
                     Use "" for root-level files.
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

    # Add namespace objects and files for each directory
    for directory, files in directories.items():
        if directory == "":
            # Root-level files
            for f in files:
                obj_id = f.replace(".qmd.md", "")
                label = obj_id.replace("_", " ").replace("-", " ").title()
                rows.append([
                    obj_id, obj_id, "Doc", label,
                    f, "3", "2", "ws", None, "{}",
                ])
        else:
            # Namespace object
            ns_id = directory.split("/")[0]
            rows.append([
                ns_id, ns_id, "__Namespace", ns_id.title(),
                f"{directory}/readme.qmd.md", "1", "1", "ws", None, "{}",
            ])
            # Files in this directory
            for f in files:
                file_path = f"{directory}/{f}"
                obj_id = f"{ns_id}::{f.replace('.qmd.md', '')}"
                label = f.replace(".qmd.md", "").replace("_", " ").replace("-", " ").title()
                rows.append([
                    obj_id, obj_id, "Page", label,
                    file_path, "3", "2", "ws", ns_id, "{}",
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

_filename_chars = st.sampled_from("abcdefghijklmnopqrstuvwxyz0123456789_")
_filename_stem = st.text(_filename_chars, min_size=1, max_size=12).filter(
    lambda s: s[0].isalpha()
)
_qmdc_filename = _filename_stem.map(lambda s: f"{s}.qmd.md").filter(
    lambda f: f != "readme.qmd.md"
)

_dir_name = st.text(
    st.sampled_from("abcdefghijklmnopqrstuvwxyz_"),
    min_size=2,
    max_size=8,
).filter(lambda s: s[0].isalpha() and s[-1].isalpha())


@st.composite
def workspace_with_siblings(draw):
    """Generate a workspace structure with multiple files per directory.

    Returns (directories, all_file_paths) where:
    - directories: dict of dir_path -> list of filenames
    - all_file_paths: list of all full file paths in the workspace
    """
    # Generate 2-4 directories, each with 2-5 files
    dir_count = draw(st.integers(min_value=2, max_value=4))
    dir_names = draw(
        st.lists(_dir_name, min_size=dir_count, max_size=dir_count, unique=True)
    )

    directories: dict[str, list[str]] = {}
    all_file_paths: list[str] = []

    # Root always has readme
    root_extras = draw(st.lists(_qmdc_filename, min_size=0, max_size=2, unique=True))
    root_files = ["readme.qmd.md"] + root_extras
    directories[""] = root_files
    all_file_paths.extend(root_files)

    # Each directory gets readme + extra files
    for dir_name in dir_names:
        extra_files = draw(
            st.lists(_qmdc_filename, min_size=1, max_size=4, unique=True)
        )
        dir_files = ["readme.qmd.md"] + extra_files
        directories[dir_name] = dir_files
        all_file_paths.extend(f"{dir_name}/{f}" for f in dir_files)

    return directories, all_file_paths


# --- Property tests using the mock_workspace_db fixture ---


class TestSiblingListingWithFixture:
    """Property 7: Sibling Listing Completeness (fixture-based).

    **Validates: Requirements 6.4**
    """

    def test_current_file_in_siblings_with_is_current(self, mock_workspace_db):
        """The current file always appears in the siblings list with is_current=True.

        **Validates: Requirements 6.4**
        """
        for source_file in FIXTURE_SOURCE_FILES:
            siblings = _get_siblings(source_file, mock_workspace_db)
            current_entries = [s for s in siblings if s.is_current]
            assert len(current_entries) == 1, (
                f"Expected exactly 1 is_current entry for '{source_file}', "
                f"got {len(current_entries)}"
            )
            assert current_entries[0].file == source_file

    def test_all_siblings_in_same_directory(self, mock_workspace_db):
        """All siblings are in the same directory as the current file.

        **Validates: Requirements 6.4**
        """
        for source_file in FIXTURE_SOURCE_FILES:
            siblings = _get_siblings(source_file, mock_workspace_db)
            source_dir = str(PurePosixPath(source_file).parent)

            for sib in siblings:
                sib_dir = str(PurePosixPath(sib.file).parent)
                assert sib_dir == source_dir, (
                    f"Sibling '{sib.file}' (dir='{sib_dir}') is not in the same "
                    f"directory as source '{source_file}' (dir='{source_dir}')"
                )

    def test_no_file_from_different_directory(self, mock_workspace_db):
        """No file from a different directory appears in the siblings list.

        **Validates: Requirements 6.4**
        """
        for source_file in FIXTURE_SOURCE_FILES:
            siblings = _get_siblings(source_file, mock_workspace_db)
            source_dir = str(PurePosixPath(source_file).parent)

            sibling_files = {s.file for s in siblings}
            # Get all files from the DB
            all_files = [
                r["__file"]
                for r in mock_workspace_db.query(
                    "SELECT DISTINCT __file FROM objects"
                )
            ]
            # Files from other directories should NOT be in siblings
            for f in all_files:
                f_dir = str(PurePosixPath(f).parent)
                if f_dir != source_dir:
                    assert f not in sibling_files, (
                        f"File '{f}' from directory '{f_dir}' should not appear "
                        f"in siblings of '{source_file}' (dir='{source_dir}')"
                    )

    def test_every_file_in_same_directory_appears(self, mock_workspace_db):
        """Every file in the same directory (according to the DB) appears in siblings.

        **Validates: Requirements 6.4**
        """
        for source_file in FIXTURE_SOURCE_FILES:
            siblings = _get_siblings(source_file, mock_workspace_db)
            sibling_files = {s.file for s in siblings}
            source_dir = str(PurePosixPath(source_file).parent)

            # Get all files in the same directory from the DB
            all_files = [
                r["__file"]
                for r in mock_workspace_db.query(
                    "SELECT DISTINCT __file FROM objects"
                )
            ]
            same_dir_files = {
                f for f in all_files
                if str(PurePosixPath(f).parent) == source_dir
            }

            assert same_dir_files == sibling_files, (
                f"For source '{source_file}': expected siblings {same_dir_files}, "
                f"got {sibling_files}. "
                f"Missing: {same_dir_files - sibling_files}, "
                f"Extra: {sibling_files - same_dir_files}"
            )

    def test_each_sibling_has_non_empty_label(self, mock_workspace_db):
        """Each sibling has a non-empty label.

        **Validates: Requirements 6.4**
        """
        for source_file in FIXTURE_SOURCE_FILES:
            siblings = _get_siblings(source_file, mock_workspace_db)
            for sib in siblings:
                assert sib.label, (
                    f"Sibling '{sib.file}' has empty label for source '{source_file}'"
                )
                assert isinstance(sib.label, str)
                assert len(sib.label.strip()) > 0, (
                    f"Sibling '{sib.file}' has whitespace-only label: '{sib.label}'"
                )


# --- Property tests with Hypothesis-generated workspaces ---


class TestSiblingListingHypothesis:
    """Property 7: Sibling Listing Completeness (Hypothesis-generated).

    **Validates: Requirements 6.4**
    """

    @given(ws=workspace_with_siblings())
    @settings(max_examples=50, deadline=2000)
    def test_current_file_always_in_siblings(self, ws):
        """The current file always appears in the siblings list with is_current=True.

        **Validates: Requirements 6.4**
        """
        directories, all_file_paths = ws
        db = build_sibling_mock_db(directories)
        try:
            for source_file in all_file_paths:
                siblings = _get_siblings(source_file, db)
                current_entries = [s for s in siblings if s.is_current]
                assert len(current_entries) == 1, (
                    f"Expected exactly 1 is_current for '{source_file}', "
                    f"got {len(current_entries)}"
                )
                assert current_entries[0].file == source_file
        finally:
            db.close()

    @given(ws=workspace_with_siblings())
    @settings(max_examples=50, deadline=2000)
    def test_all_siblings_in_same_directory(self, ws):
        """All siblings are in the same directory as the current file.

        **Validates: Requirements 6.4**
        """
        directories, all_file_paths = ws
        db = build_sibling_mock_db(directories)
        try:
            for source_file in all_file_paths:
                siblings = _get_siblings(source_file, db)
                source_dir = str(PurePosixPath(source_file).parent)

                for sib in siblings:
                    sib_dir = str(PurePosixPath(sib.file).parent)
                    assert sib_dir == source_dir, (
                        f"Sibling '{sib.file}' not in same dir as '{source_file}'"
                    )
        finally:
            db.close()

    @given(ws=workspace_with_siblings())
    @settings(max_examples=50, deadline=2000)
    def test_no_file_from_different_directory(self, ws):
        """No file from a different directory appears in the siblings list.

        **Validates: Requirements 6.4**
        """
        directories, all_file_paths = ws
        db = build_sibling_mock_db(directories)
        try:
            for source_file in all_file_paths:
                siblings = _get_siblings(source_file, db)
                source_dir = str(PurePosixPath(source_file).parent)
                sibling_files = {s.file for s in siblings}

                # No file from a different directory should be present
                for other_file in all_file_paths:
                    other_dir = str(PurePosixPath(other_file).parent)
                    if other_dir != source_dir:
                        assert other_file not in sibling_files, (
                            f"File '{other_file}' from dir '{other_dir}' "
                            f"in siblings of '{source_file}' (dir='{source_dir}')"
                        )
        finally:
            db.close()

    @given(ws=workspace_with_siblings())
    @settings(max_examples=50, deadline=2000)
    def test_every_same_dir_file_in_siblings(self, ws):
        """Every file in the same directory appears in the siblings list.

        **Validates: Requirements 6.4**
        """
        directories, all_file_paths = ws
        db = build_sibling_mock_db(directories)
        try:
            for source_file in all_file_paths:
                siblings = _get_siblings(source_file, db)
                sibling_files = {s.file for s in siblings}
                source_dir = str(PurePosixPath(source_file).parent)

                # All files in the same directory should be in siblings
                same_dir_files = {
                    f for f in all_file_paths
                    if str(PurePosixPath(f).parent) == source_dir
                }

                assert same_dir_files == sibling_files, (
                    f"For '{source_file}': "
                    f"missing={same_dir_files - sibling_files}, "
                    f"extra={sibling_files - same_dir_files}"
                )
        finally:
            db.close()

    @given(ws=workspace_with_siblings())
    @settings(max_examples=50, deadline=2000)
    def test_each_sibling_has_non_empty_label(self, ws):
        """Each sibling has a non-empty label.

        **Validates: Requirements 6.4**
        """
        directories, all_file_paths = ws
        db = build_sibling_mock_db(directories)
        try:
            for source_file in all_file_paths:
                siblings = _get_siblings(source_file, db)
                for sib in siblings:
                    assert sib.label, (
                        f"Sibling '{sib.file}' has empty label"
                    )
                    assert isinstance(sib.label, str)
                    assert len(sib.label.strip()) > 0, (
                        f"Sibling '{sib.file}' has whitespace-only label"
                    )
        finally:
            db.close()


_edge_source_file_strategy = st.sampled_from(FIXTURE_SOURCE_FILES)


class TestGraphEdgeCorrectness:
    """Property 5: Graph Sidebar Edge Correctness.

    **Validates: Requirements 3.8, 6.2, 6.3**

    For any source file in the workspace, compute_graph_context SHALL produce
    edges where:
    - Every edge in links_to connects the current page to a DIFFERENT page (no self-edges)
    - Every edge in linked_from connects a DIFFERENT page to the current page (no self-edges)
    - All edges reference non-system objects (kind does not start with `__`)
    - Edge items have non-empty edge_type, obj_id, label, kind, and file fields
    - The union of links_to targets and linked_from sources covers all cross-page edges
      in the DB for that file
    """

    @given(source_file=_edge_source_file_strategy)
    @settings(max_examples=50, suppress_health_check=[HealthCheck.function_scoped_fixture])
    def test_links_to_no_self_edges(self, source_file, mock_workspace_db):
        """Every edge in links_to connects the current page to a DIFFERENT page.

        **Validates: Requirements 6.2**
        """
        ctx = compute_graph_context(source_file, mock_workspace_db)

        for edge in ctx.links_to:
            assert edge.file != source_file, (
                f"Self-edge found in links_to: edge to '{edge.obj_id}' "
                f"(file={edge.file}) from source '{source_file}'"
            )

    @given(source_file=_edge_source_file_strategy)
    @settings(max_examples=50, suppress_health_check=[HealthCheck.function_scoped_fixture])
    def test_linked_from_no_self_edges(self, source_file, mock_workspace_db):
        """Every edge in linked_from connects a DIFFERENT page to the current page.

        **Validates: Requirements 6.3**
        """
        ctx = compute_graph_context(source_file, mock_workspace_db)

        for edge in ctx.linked_from:
            assert edge.file != source_file, (
                f"Self-edge found in linked_from: edge from '{edge.obj_id}' "
                f"(file={edge.file}) to source '{source_file}'"
            )

    @given(source_file=_edge_source_file_strategy)
    @settings(max_examples=50, suppress_health_check=[HealthCheck.function_scoped_fixture])
    def test_edges_reference_non_system_objects(self, source_file, mock_workspace_db):
        """All edges reference non-system objects (kind does not start with `__`).

        **Validates: Requirements 3.8, 6.2, 6.3**
        """
        ctx = compute_graph_context(source_file, mock_workspace_db)

        for edge in ctx.links_to:
            assert not edge.kind.startswith("__"), (
                f"System object in links_to: '{edge.obj_id}' has kind '{edge.kind}'"
            )

        for edge in ctx.linked_from:
            assert not edge.kind.startswith("__"), (
                f"System object in linked_from: '{edge.obj_id}' has kind '{edge.kind}'"
            )

    @given(source_file=_edge_source_file_strategy)
    @settings(max_examples=50, suppress_health_check=[HealthCheck.function_scoped_fixture])
    def test_edge_items_have_non_empty_fields(self, source_file, mock_workspace_db):
        """Edge items have non-empty edge_type, obj_id, label, kind, and file fields.

        **Validates: Requirements 6.2, 6.3**
        """
        ctx = compute_graph_context(source_file, mock_workspace_db)

        all_edges = ctx.links_to + ctx.linked_from
        for edge in all_edges:
            assert edge.edge_type, (
                f"Empty edge_type for edge to/from '{edge.obj_id}'"
            )
            assert edge.obj_id, (
                f"Empty obj_id for edge with type '{edge.edge_type}'"
            )
            assert edge.label, (
                f"Empty label for edge '{edge.obj_id}'"
            )
            assert edge.kind, (
                f"Empty kind for edge '{edge.obj_id}'"
            )
            assert edge.file, (
                f"Empty file for edge '{edge.obj_id}'"
            )

    @given(source_file=_edge_source_file_strategy)
    @settings(max_examples=50, suppress_health_check=[HealthCheck.function_scoped_fixture])
    def test_edges_cover_all_cross_page_edges(self, source_file, mock_workspace_db):
        """The union of links_to targets and linked_from sources covers all
        cross-page edges in the DB for that file.

        **Validates: Requirements 3.8, 6.2, 6.3**
        """
        ctx = compute_graph_context(source_file, mock_workspace_db)

        # Query all outgoing cross-page edges from DB directly
        # (source in this file, target in another file, target is non-system)
        outgoing_rows = mock_workspace_db.query(
            """
            SELECT DISTINCT e.edge_type, t.__id, t.__file
            FROM edges e
            JOIN objects s ON e.source_id = s.__global_id
            JOIN objects t ON e.target_id = t.__global_id
            WHERE s.__file = ? AND t.__file != ? AND t.__kind NOT GLOB '__*'
            """,
            params=(source_file, source_file),
        )

        # Query all incoming cross-page edges from DB directly
        # (target in this file, source in another file, source is non-system)
        incoming_rows = mock_workspace_db.query(
            """
            SELECT DISTINCT e.edge_type, s.__id, s.__file
            FROM edges e
            JOIN objects s ON e.source_id = s.__global_id
            JOIN objects t ON e.target_id = t.__global_id
            WHERE t.__file = ? AND s.__file != ? AND s.__kind NOT GLOB '__*'
            """,
            params=(source_file, source_file),
        )

        # Verify links_to covers all outgoing edges
        links_to_set = {(e.edge_type, e.obj_id, e.file) for e in ctx.links_to}
        for row in outgoing_rows:
            key = (row["edge_type"], row["__id"], row["__file"])
            assert key in links_to_set, (
                f"Outgoing edge {key} from DB not found in links_to for '{source_file}'. "
                f"links_to has: {links_to_set}"
            )

        # Verify linked_from covers all incoming edges
        linked_from_set = {(e.edge_type, e.obj_id, e.file) for e in ctx.linked_from}
        for row in incoming_rows:
            key = (row["edge_type"], row["__id"], row["__file"])
            assert key in linked_from_set, (
                f"Incoming edge {key} from DB not found in linked_from for '{source_file}'. "
                f"linked_from has: {linked_from_set}"
            )


# --- Property 6: Breadcrumb Hierarchy ---


def build_breadcrumb_mock_db(
    workspace_label: str,
    namespaces: dict[str, str],
    files: list[tuple[str, str, str | None]],
) -> object:
    """Build a mock WorkspaceDB for breadcrumb testing.

    Args:
        workspace_label: Label for the __Workspace object.
        namespaces: dict of namespace_id -> namespace_label.
        files: list of (file_path, file_label, namespace_id_or_None) tuples.
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
        "ws", "ws", "__Workspace", workspace_label,
        "readme.qmd.md", "1", "1", None, None, "{}",
    ])

    # Namespace objects
    for ns_id, ns_label in namespaces.items():
        rows.append([
            ns_id, ns_id, "__Namespace", ns_label,
            f"{ns_id}/readme.qmd.md", "1", "1", "ws", None, "{}",
        ])

    # File objects (non-system objects representing page content)
    for file_path, file_label, ns_id in files:
        obj_id = file_path.replace("/", "_").replace(".qmd.md", "")
        rows.append([
            obj_id, obj_id, "Page", file_label,
            file_path, "3", "2", "ws", ns_id, "{}",
        ])

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
            self.objects_by_file = {}

        def query(self, sql, params=()):
            cur = self.conn.execute(sql, params)
            cols = [d[0] for d in cur.description]
            return [dict(zip(cols, row)) for row in cur.fetchall()]

        def close(self):
            self.conn.close()

    return MockDB(conn)


# Hypothesis strategies for breadcrumb testing

_label_chars = st.sampled_from("abcdefghijklmnopqrstuvwxyz ")
_non_empty_label = st.text(_label_chars, min_size=2, max_size=20).map(str.strip).filter(
    lambda s: len(s) >= 2
)

_ns_id_chars = st.sampled_from("abcdefghijklmnopqrstuvwxyz_")
_ns_id = st.text(_ns_id_chars, min_size=2, max_size=10).filter(
    lambda s: s[0].isalpha() and s[-1].isalpha()
)

_filename_chars = st.sampled_from("abcdefghijklmnopqrstuvwxyz0123456789_")
_filename_stem = st.text(_filename_chars, min_size=1, max_size=12).filter(
    lambda s: s[0].isalpha()
)


@st.composite
def breadcrumb_workspace(draw):
    """Generate a workspace with a mix of root-level and namespaced files.

    Returns (workspace_label, namespaces, files) where:
    - workspace_label: str
    - namespaces: dict of ns_id -> ns_label
    - files: list of (file_path, file_label, ns_id_or_None)
    """
    workspace_label = draw(_non_empty_label)

    # Generate 1-3 namespaces
    ns_count = draw(st.integers(min_value=1, max_value=3))
    ns_ids = draw(st.lists(_ns_id, min_size=ns_count, max_size=ns_count, unique=True))
    ns_labels = draw(
        st.lists(_non_empty_label, min_size=ns_count, max_size=ns_count)
    )
    namespaces = dict(zip(ns_ids, ns_labels))

    files: list[tuple[str, str, str | None]] = []

    # Root-level files (no namespace)
    root_count = draw(st.integers(min_value=1, max_value=3))
    root_stems = draw(
        st.lists(_filename_stem, min_size=root_count, max_size=root_count, unique=True)
    )
    for stem in root_stems:
        file_path = f"{stem}.qmd.md"
        file_label = draw(_non_empty_label)
        files.append((file_path, file_label, None))

    # Namespaced files
    for ns_id in ns_ids:
        file_count = draw(st.integers(min_value=1, max_value=3))
        file_stems = draw(
            st.lists(_filename_stem, min_size=file_count, max_size=file_count, unique=True)
        )
        for stem in file_stems:
            file_path = f"{ns_id}/{stem}.qmd.md"
            file_label = draw(_non_empty_label)
            files.append((file_path, file_label, ns_id))

    return workspace_label, namespaces, files


class TestBreadcrumbHierarchyFixture:
    """Property 6: Breadcrumb Hierarchy (fixture-based).

    **Validates: Requirements 6.1**

    For any page in a namespace, the breadcrumb trail SHALL contain exactly
    three segments: workspace label, namespace label, and file label — in that
    order. For root-level pages (no namespace), the breadcrumb SHALL contain
    workspace label and file label.
    """

    def test_root_page_has_two_segments(self, mock_workspace_db):
        """Root-level pages have breadcrumb: workspace_label + file_label (no namespace).

        **Validates: Requirements 6.1**
        """
        ctx = compute_graph_context("readme.qmd.md", mock_workspace_db)

        # workspace_label is non-empty
        assert ctx.workspace_label
        assert isinstance(ctx.workspace_label, str)
        assert len(ctx.workspace_label.strip()) > 0

        # namespace_label is None for root-level pages
        assert ctx.namespace_label is None

        # file_label is non-empty
        assert ctx.file_label
        assert isinstance(ctx.file_label, str)
        assert len(ctx.file_label.strip()) > 0

    def test_namespaced_page_has_three_segments(self, mock_workspace_db):
        """Namespaced pages have breadcrumb: workspace_label + namespace_label + file_label.

        **Validates: Requirements 6.1**
        """
        namespaced_files = [
            "storage/readme.qmd.md",
            "storage/tables.qmd.md",
            "api/readme.qmd.md",
            "api/endpoints.qmd.md",
        ]

        for source_file in namespaced_files:
            ctx = compute_graph_context(source_file, mock_workspace_db)

            # workspace_label is non-empty
            assert ctx.workspace_label, (
                f"Empty workspace_label for '{source_file}'"
            )

            # namespace_label is non-None and non-empty for namespaced pages
            assert ctx.namespace_label is not None, (
                f"namespace_label is None for namespaced page '{source_file}'"
            )
            assert len(ctx.namespace_label.strip()) > 0, (
                f"namespace_label is empty for '{source_file}'"
            )

            # file_label is non-empty
            assert ctx.file_label, (
                f"Empty file_label for '{source_file}'"
            )

    def test_breadcrumb_order_is_workspace_namespace_file(self, mock_workspace_db):
        """Breadcrumb segments are in order: workspace → namespace → file.

        **Validates: Requirements 6.1**
        """
        ctx = compute_graph_context("storage/tables.qmd.md", mock_workspace_db)

        # The workspace label should match the __Workspace object
        ws_rows = mock_workspace_db.query(
            "SELECT __label FROM objects WHERE __kind = '__Workspace' LIMIT 1"
        )
        expected_ws_label = ws_rows[0]["__label"] if ws_rows else "Workspace"
        assert ctx.workspace_label == expected_ws_label

        # The namespace label should match the __Namespace object for 'storage'
        ns_rows = mock_workspace_db.query(
            "SELECT __label FROM objects WHERE __kind = '__Namespace' AND __id = 'storage'"
        )
        expected_ns_label = ns_rows[0]["__label"] if ns_rows else None
        assert ctx.namespace_label == expected_ns_label

        # The file label should be the top-level non-system object's label
        assert ctx.file_label
        assert isinstance(ctx.file_label, str)

    def test_workspace_label_consistent_across_all_pages(self, mock_workspace_db):
        """The workspace_label is the same for all pages in the workspace.

        **Validates: Requirements 6.1**
        """
        all_files = FIXTURE_SOURCE_FILES
        labels = set()
        for source_file in all_files:
            ctx = compute_graph_context(source_file, mock_workspace_db)
            labels.add(ctx.workspace_label)

        assert len(labels) == 1, (
            f"Expected consistent workspace_label across all pages, got: {labels}"
        )


class TestBreadcrumbHierarchyHypothesis:
    """Property 6: Breadcrumb Hierarchy (Hypothesis-generated).

    **Validates: Requirements 6.1**

    For any page in a namespace, the breadcrumb trail SHALL contain exactly
    three segments: workspace label, namespace label (or derived title), and
    file label — in that order. For root-level pages (no namespace), the
    breadcrumb SHALL contain workspace label and file label.
    """

    @given(ws=breadcrumb_workspace())
    @settings(max_examples=50, deadline=2000)
    def test_root_pages_have_no_namespace_label(self, ws):
        """Root-level pages (no namespace) have namespace_label=None.

        **Validates: Requirements 6.1**
        """
        workspace_label, namespaces, files = ws
        db = build_breadcrumb_mock_db(workspace_label, namespaces, files)
        try:
            root_files = [(fp, fl, ns) for fp, fl, ns in files if ns is None]
            for file_path, file_label, _ in root_files:
                ctx = compute_graph_context(file_path, db)
                assert ctx.namespace_label is None, (
                    f"Root page '{file_path}' should have namespace_label=None, "
                    f"got '{ctx.namespace_label}'"
                )
        finally:
            db.close()

    @given(ws=breadcrumb_workspace())
    @settings(max_examples=50, deadline=2000)
    def test_namespaced_pages_have_namespace_label(self, ws):
        """Namespaced pages have a non-None, non-empty namespace_label.

        **Validates: Requirements 6.1**
        """
        workspace_label, namespaces, files = ws
        db = build_breadcrumb_mock_db(workspace_label, namespaces, files)
        try:
            ns_files = [(fp, fl, ns) for fp, fl, ns in files if ns is not None]
            for file_path, file_label, ns_id in ns_files:
                ctx = compute_graph_context(file_path, db)
                assert ctx.namespace_label is not None, (
                    f"Namespaced page '{file_path}' (ns={ns_id}) should have "
                    f"non-None namespace_label"
                )
                assert len(ctx.namespace_label.strip()) > 0, (
                    f"Namespaced page '{file_path}' has empty namespace_label"
                )
        finally:
            db.close()

    @given(ws=breadcrumb_workspace())
    @settings(max_examples=50, deadline=2000)
    def test_workspace_label_always_non_empty(self, ws):
        """workspace_label is always non-empty for every page.

        **Validates: Requirements 6.1**
        """
        workspace_label, namespaces, files = ws
        db = build_breadcrumb_mock_db(workspace_label, namespaces, files)
        try:
            for file_path, _, _ in files:
                ctx = compute_graph_context(file_path, db)
                assert ctx.workspace_label, (
                    f"Empty workspace_label for page '{file_path}'"
                )
                assert len(ctx.workspace_label.strip()) > 0, (
                    f"Whitespace-only workspace_label for page '{file_path}'"
                )
        finally:
            db.close()

    @given(ws=breadcrumb_workspace())
    @settings(max_examples=50, deadline=2000)
    def test_file_label_always_non_empty(self, ws):
        """file_label is always non-empty for every page.

        **Validates: Requirements 6.1**
        """
        workspace_label, namespaces, files = ws
        db = build_breadcrumb_mock_db(workspace_label, namespaces, files)
        try:
            for file_path, _, _ in files:
                ctx = compute_graph_context(file_path, db)
                assert ctx.file_label, (
                    f"Empty file_label for page '{file_path}'"
                )
                assert len(ctx.file_label.strip()) > 0, (
                    f"Whitespace-only file_label for page '{file_path}'"
                )
        finally:
            db.close()

    @given(ws=breadcrumb_workspace())
    @settings(max_examples=50, deadline=2000)
    def test_workspace_label_consistent_across_pages(self, ws):
        """workspace_label is the same for all pages in the workspace.

        **Validates: Requirements 6.1**
        """
        workspace_label, namespaces, files = ws
        db = build_breadcrumb_mock_db(workspace_label, namespaces, files)
        try:
            labels = set()
            for file_path, _, _ in files:
                ctx = compute_graph_context(file_path, db)
                labels.add(ctx.workspace_label)
            assert len(labels) == 1, (
                f"Expected consistent workspace_label, got: {labels}"
            )
        finally:
            db.close()

    @given(ws=breadcrumb_workspace())
    @settings(max_examples=50, deadline=2000)
    def test_namespace_label_matches_namespace_object(self, ws):
        """For namespaced pages, namespace_label matches the __Namespace object's label.

        **Validates: Requirements 6.1**
        """
        workspace_label, namespaces, files = ws
        db = build_breadcrumb_mock_db(workspace_label, namespaces, files)
        try:
            ns_files = [(fp, fl, ns) for fp, fl, ns in files if ns is not None]
            for file_path, _, ns_id in ns_files:
                ctx = compute_graph_context(file_path, db)
                expected_label = namespaces[ns_id]
                assert ctx.namespace_label == expected_label, (
                    f"For page '{file_path}' (ns={ns_id}): "
                    f"expected namespace_label='{expected_label}', "
                    f"got '{ctx.namespace_label}'"
                )
        finally:
            db.close()

    @given(ws=breadcrumb_workspace())
    @settings(max_examples=50, deadline=2000)
    def test_breadcrumb_segment_count(self, ws):
        """Root pages have 2 segments, namespaced pages have 3 segments.

        **Validates: Requirements 6.1**
        """
        workspace_label, namespaces, files = ws
        db = build_breadcrumb_mock_db(workspace_label, namespaces, files)
        try:
            for file_path, _, ns_id in files:
                ctx = compute_graph_context(file_path, db)

                # Build the breadcrumb segments list
                segments = [ctx.workspace_label]
                if ctx.namespace_label is not None:
                    segments.append(ctx.namespace_label)
                segments.append(ctx.file_label)

                if ns_id is None:
                    # Root page: exactly 2 segments
                    assert len(segments) == 2, (
                        f"Root page '{file_path}' should have 2 breadcrumb segments, "
                        f"got {len(segments)}: {segments}"
                    )
                else:
                    # Namespaced page: exactly 3 segments
                    assert len(segments) == 3, (
                        f"Namespaced page '{file_path}' should have 3 breadcrumb "
                        f"segments, got {len(segments)}: {segments}"
                    )
        finally:
            db.close()
