"""Property-based tests for file mapping preservation in convert_workspace.

**Validates: Requirements 3.1**

Property 1: File Mapping Preservation
For any set of .qmd.md files in a workspace, convert_workspace produces exactly
one .md file per input file. The output .md file path matches the input path with
.qmd.md replaced by .md. Directory structure is preserved. Hidden directories
(starting with '.') are skipped. _pages/*.md files are copied unchanged.
"""

from __future__ import annotations

import sqlite3
import tempfile
from pathlib import Path

import pytest
from hypothesis import given, settings, assume, HealthCheck
from hypothesis import strategies as st

from qmdc_mkdocs.converter import convert_workspace


# --- Strategies for generating workspace structures ---

# Valid directory/file name characters (safe subset)
_name_chars = st.sampled_from(
    "abcdefghijklmnopqrstuvwxyz0123456789_-"
)

# Generate a valid file stem (1-12 chars, starts with letter)
_file_stem = st.builds(
    lambda first, rest: first + rest,
    st.sampled_from("abcdefghijklmnopqrstuvwxyz"),
    st.text(_name_chars, min_size=0, max_size=11),
)

# Generate a valid directory name (1-10 chars, starts with letter)
_dir_name = st.builds(
    lambda first, rest: first + rest,
    st.sampled_from("abcdefghijklmnopqrstuvwxyz"),
    st.text(_name_chars, min_size=0, max_size=9),
)

# Generate a hidden directory name (starts with '.')
_hidden_dir_name = st.builds(
    lambda name: "." + name,
    _dir_name,
)


@st.composite
def workspace_structure(draw):
    """Generate a random workspace structure with .qmd.md files.

    Returns a dict with:
    - qmdc_files: list of relative paths for .qmd.md files (non-hidden)
    - hidden_files: list of relative paths for .qmd.md files in hidden dirs
    - pages_files: list of filenames for _pages/*.md files
    """
    # Generate 1-5 top-level .qmd.md files
    n_root_files = draw(st.integers(min_value=1, max_value=5))
    root_stems = draw(
        st.lists(_file_stem, min_size=n_root_files, max_size=n_root_files, unique=True)
    )
    qmdc_files = [f"{stem}.qmd.md" for stem in root_stems]

    # Generate 0-3 subdirectories with 1-3 files each
    n_dirs = draw(st.integers(min_value=0, max_value=3))
    dir_names = draw(st.lists(_dir_name, min_size=n_dirs, max_size=n_dirs, unique=True))

    # Ensure dir names don't collide with root file stems
    dir_names = [d for d in dir_names if d not in root_stems]

    for dir_name in dir_names:
        n_files = draw(st.integers(min_value=1, max_value=3))
        stems = draw(st.lists(_file_stem, min_size=n_files, max_size=n_files, unique=True))
        for stem in stems:
            qmdc_files.append(f"{dir_name}/{stem}.qmd.md")

    # Generate 0-2 hidden directories with 1-2 files each
    n_hidden = draw(st.integers(min_value=0, max_value=2))
    hidden_dir_names = draw(
        st.lists(_hidden_dir_name, min_size=n_hidden, max_size=n_hidden, unique=True)
    )
    hidden_files = []
    for hdir in hidden_dir_names:
        n_files = draw(st.integers(min_value=1, max_value=2))
        stems = draw(st.lists(_file_stem, min_size=n_files, max_size=n_files, unique=True))
        for stem in stems:
            hidden_files.append(f"{hdir}/{stem}.qmd.md")

    # Generate 0-2 _pages/*.md files
    n_pages = draw(st.integers(min_value=0, max_value=2))
    pages_stems = draw(st.lists(_file_stem, min_size=n_pages, max_size=n_pages, unique=True))
    pages_files = [f"{stem}.md" for stem in pages_stems]

    return {
        "qmdc_files": qmdc_files,
        "hidden_files": hidden_files,
        "pages_files": pages_files,
    }


def _make_minimal_db():
    """Create a minimal WorkspaceDB with no objects/edges (sufficient for file mapping test)."""
    conn = sqlite3.connect(":memory:")
    conn.execute("PRAGMA journal_mode = OFF")
    conn.execute("PRAGMA synchronous = OFF")

    # Create objects table with required columns
    conn.execute("""
        CREATE TABLE objects (
            "__global_id" TEXT,
            "__id" TEXT,
            "__kind" TEXT,
            "__label" TEXT,
            "__file" TEXT,
            "__line" TEXT,
            "__level" TEXT,
            "__workspace" TEXT,
            "__namespace" TEXT,
            "data" TEXT
        )
    """)

    # Create edges table
    conn.execute("""
        CREATE TABLE edges (
            "source_id" TEXT,
            "source_field" TEXT,
            "target_id" TEXT,
            "edge_type" TEXT,
            "__workspace" TEXT
        )
    """)

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

    class MinimalDB:
        def __init__(self, connection):
            self.conn = connection

        def query(self, sql, params=()):
            cur = self.conn.execute(sql, params)
            cols = [d[0] for d in cur.description]
            return [dict(zip(cols, row)) for row in cur.fetchall()]

        def close(self):
            self.conn.close()

    return MinimalDB(conn)


def _create_workspace(base_dir: Path, structure: dict) -> Path:
    """Create a workspace directory with the given structure on disk."""
    workspace = base_dir / "workspace"
    workspace.mkdir(parents=True)

    # Create .qmd.md files (non-hidden)
    for rel_path in structure["qmdc_files"]:
        file_path = workspace / rel_path
        file_path.parent.mkdir(parents=True, exist_ok=True)
        file_path.write_text(f"# Content of {rel_path}\n")

    # Create hidden directory files
    for rel_path in structure["hidden_files"]:
        file_path = workspace / rel_path
        file_path.parent.mkdir(parents=True, exist_ok=True)
        file_path.write_text(f"# Hidden {rel_path}\n")

    # Create _pages/*.md files
    if structure["pages_files"]:
        pages_dir = workspace / "_pages"
        pages_dir.mkdir(exist_ok=True)
        for filename in structure["pages_files"]:
            (pages_dir / filename).write_text(f"# Page {filename}\n")

    return workspace


# --- Property Tests ---


class TestFileMappingPreservation:
    """Property 1: File Mapping Preservation.

    **Validates: Requirements 3.1**
    """

    @given(structure=workspace_structure())
    @settings(max_examples=50)
    def test_one_md_per_qmdc_file(self, structure):
        """For any set of .qmd.md files, convert_workspace produces exactly one
        .md file per input file.

        **Validates: Requirements 3.1**
        """
        with tempfile.TemporaryDirectory() as td:
            base = Path(td)
            workspace = _create_workspace(base, structure)
            tmpdir = base / "build"
            tmpdir.mkdir()

            db = _make_minimal_db()
            try:
                count = convert_workspace(workspace, tmpdir, db, {})

                # Count should equal number of non-hidden .qmd.md files
                assert count == len(structure["qmdc_files"]), (
                    f"Expected {len(structure['qmdc_files'])} pages, got {count}. "
                    f"Files: {structure['qmdc_files']}"
                )

                # Each input file should have exactly one output file
                docs_dir = tmpdir / "docs"
                for rel_path in structure["qmdc_files"]:
                    expected_output = rel_path.replace(".qmd.md", ".md")
                    output_file = docs_dir / expected_output
                    assert output_file.exists(), (
                        f"Expected output file {expected_output} not found for input {rel_path}"
                    )
            finally:
                db.close()

    @given(structure=workspace_structure())
    @settings(max_examples=50)
    def test_output_path_matches_input_with_extension_replaced(self, structure):
        """The output .md file path matches the input path with .qmd.md replaced by .md.

        **Validates: Requirements 3.1**
        """
        with tempfile.TemporaryDirectory() as td:
            base = Path(td)
            workspace = _create_workspace(base, structure)
            tmpdir = base / "build"
            tmpdir.mkdir()

            db = _make_minimal_db()
            try:
                convert_workspace(workspace, tmpdir, db, {})

                docs_dir = tmpdir / "docs"
                # Collect all .md files produced (excluding _pages)
                produced_files = set()
                for md_file in docs_dir.rglob("*.md"):
                    rel = md_file.relative_to(docs_dir)
                    if "_pages" not in rel.parts:
                        produced_files.add(str(rel))

                # Expected files: input paths with .qmd.md → .md
                expected_files = {
                    rel_path.replace(".qmd.md", ".md")
                    for rel_path in structure["qmdc_files"]
                }

                assert produced_files == expected_files, (
                    f"Produced files don't match expected.\n"
                    f"Extra: {produced_files - expected_files}\n"
                    f"Missing: {expected_files - produced_files}"
                )
            finally:
                db.close()

    @given(structure=workspace_structure())
    @settings(max_examples=50)
    def test_directory_structure_preserved(self, structure):
        """Directory structure is preserved (subdirectories in input → same
        subdirectories in output).

        **Validates: Requirements 3.1**
        """
        with tempfile.TemporaryDirectory() as td:
            base = Path(td)
            workspace = _create_workspace(base, structure)
            tmpdir = base / "build"
            tmpdir.mkdir()

            db = _make_minimal_db()
            try:
                convert_workspace(workspace, tmpdir, db, {})

                docs_dir = tmpdir / "docs"
                for rel_path in structure["qmdc_files"]:
                    output_path = rel_path.replace(".qmd.md", ".md")
                    output_file = docs_dir / output_path

                    # The parent directory of the output should match the parent of the input
                    input_parent = Path(rel_path).parent
                    output_parent = output_file.relative_to(docs_dir).parent
                    assert str(input_parent) == str(output_parent), (
                        f"Directory mismatch for {rel_path}: "
                        f"input parent={input_parent}, output parent={output_parent}"
                    )
            finally:
                db.close()

    @given(structure=workspace_structure())
    @settings(max_examples=50)
    def test_hidden_directories_skipped(self, structure):
        """Hidden directories (starting with '.') are skipped.

        **Validates: Requirements 3.1**
        """
        assume(len(structure["hidden_files"]) > 0)

        with tempfile.TemporaryDirectory() as td:
            base = Path(td)
            workspace = _create_workspace(base, structure)
            tmpdir = base / "build"
            tmpdir.mkdir()

            db = _make_minimal_db()
            try:
                convert_workspace(workspace, tmpdir, db, {})

                docs_dir = tmpdir / "docs"
                # No hidden directory files should appear in output
                for rel_path in structure["hidden_files"]:
                    output_path = rel_path.replace(".qmd.md", ".md")
                    output_file = docs_dir / output_path
                    assert not output_file.exists(), (
                        f"Hidden file {rel_path} should not produce output, "
                        f"but {output_path} exists"
                    )

                # Also verify no hidden directories exist in output at all
                if docs_dir.exists():
                    for item in docs_dir.rglob("*"):
                        rel = item.relative_to(docs_dir)
                        for part in rel.parts:
                            assert not part.startswith("."), (
                                f"Hidden directory component found in output: {rel}"
                            )
            finally:
                db.close()

    @given(structure=workspace_structure())
    @settings(max_examples=50)
    def test_pages_copied_unchanged(self, structure):
        """_pages/*.md files are copied unchanged (not transformed).

        **Validates: Requirements 3.1**
        """
        assume(len(structure["pages_files"]) > 0)

        with tempfile.TemporaryDirectory() as td:
            base = Path(td)
            workspace = _create_workspace(base, structure)
            tmpdir = base / "build"
            tmpdir.mkdir()

            db = _make_minimal_db()
            try:
                convert_workspace(workspace, tmpdir, db, {})

                docs_dir = tmpdir / "docs"
                pages_out = docs_dir / "_pages"

                # Each _pages file should be copied unchanged
                for filename in structure["pages_files"]:
                    source_file = workspace / "_pages" / filename
                    output_file = pages_out / filename

                    assert output_file.exists(), (
                        f"_pages/{filename} should be copied to output"
                    )

                    # Content should be identical (not transformed)
                    assert output_file.read_text() == source_file.read_text(), (
                        f"_pages/{filename} content was modified during copy"
                    )
            finally:
                db.close()
