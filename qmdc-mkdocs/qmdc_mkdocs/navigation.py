"""Navigation tree generation from workspace structure."""

from __future__ import annotations

from pathlib import Path, PurePosixPath
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from .database import WorkspaceData


def generate_nav(db: WorkspaceData, namespace_prefix: str | None = None) -> list:
    """Generate MkDocs nav structure from workspace data.

    Groups files by directory, orders readme first then alphabetical,
    and derives section/page titles from namespace labels or directory names.
    Respects .qmdc-mkdocs.ignore patterns (same as converter).

    If namespace_prefix is set, only includes files under that prefix and
    strips the prefix from output paths.
    """
    from .ignore import is_ignored, load_siteignore

    files = [
        r["__file"]
        for r in db.query("SELECT DISTINCT __file FROM objects ORDER BY __file")
    ]

    # Filter out ignored files (before prefix strip — matches full paths)
    if hasattr(db, "result") and hasattr(db.result, "root"):
        ignore_patterns = load_siteignore(Path(db.result.root))
        files = [f for f in files if not is_ignored(f, ignore_patterns)]

    # Filter by namespace prefix and strip it from paths
    if namespace_prefix:
        prefix = namespace_prefix + "/"
        files = [f[len(prefix):] for f in files if f.startswith(prefix)]

    # Filter again after stripping (patterns may match relative paths)
    if hasattr(db, "result") and hasattr(db.result, "root"):
        files = [f for f in files if not is_ignored(f, ignore_patterns)]

    # Remove empty paths
    files = [f for f in files if f]

    namespaces = {
        r["__id"]: r["__label"]
        for r in db.query(
            "SELECT __id, __label FROM objects WHERE __kind = '__Namespace'"
        )
    }

    file_labels = {
        r["__file"]: r["__label"]
        for r in db.query(
            "SELECT __file, __label FROM objects "
            "WHERE CAST(__level AS INTEGER) = 1 "
            "AND __kind NOT GLOB '__*' AND __label IS NOT NULL"
        )
    }

    # Build nested nav tree from directory structure
    nav = _build_nav_tree(files, namespaces, file_labels)

    return nav


def _build_nav_tree(
    files: list[str],
    namespaces: dict[str, str],
    file_labels: dict[str, str],
) -> list:
    """Build a nested MkDocs nav tree from file paths.

    Groups files by their full directory path, creating nested sections
    for subdirectories. Each directory becomes a section with its files
    as children.
    """
    # Group files by their immediate parent directory
    dir_files: dict[str, list[str]] = {}  # dir_path -> [filenames in that dir]
    subdirs: dict[str, set[str]] = {}  # dir_path -> {child dir names}

    for f in files:
        parts = PurePosixPath(f).parts
        if len(parts) <= 1:
            dir_files.setdefault("", []).append(f)
        else:
            parent = str(PurePosixPath(*parts[:-1]))
            dir_files.setdefault(parent, []).append(f)
            # Register parent chain
            for i in range(1, len(parts) - 1):
                ancestor = str(PurePosixPath(*parts[:i]))
                child = parts[i]
                subdirs.setdefault(ancestor, set()).add(child)
            # Top-level dirs
            if len(parts) > 1:
                subdirs.setdefault("", set()).add(parts[0])

    def build_section(dir_path: str) -> list:
        """Recursively build nav items for a directory."""
        items = []

        # Files directly in this directory (readme/index first, then alphabetical)
        direct_files = dir_files.get(dir_path, [])
        direct_files.sort(key=lambda f: (
            0 if PurePosixPath(f).name == "readme.qmd.md" else 1,
            f,
        ))
        for f in direct_files:
            title = _derive_page_title(f, file_labels)
            md_path = _qmdc_to_nav_path(f)
            items.append({title: md_path})

        # Subdirectories as nested sections
        child_dirs = sorted(subdirs.get(dir_path, set()))
        for child in child_dirs:
            child_path = f"{dir_path}/{child}" if dir_path else child
            child_title = _derive_section_title(child, namespaces)
            child_items = build_section(child_path)
            if child_items:
                items.append({child_title: child_items})

        return items

    # Build from root
    return build_section("")


def _derive_section_title(
    directory: str, namespaces: dict[str, str]
) -> str:
    """Get section title from namespace __label or derive from directory name."""
    # The namespace ID is the first path component
    ns_id = directory.split("/")[0] if directory else ""
    if ns_id in namespaces and namespaces[ns_id]:
        return namespaces[ns_id]
    return _title_from_dirname(ns_id)


def _title_from_dirname(name: str) -> str:
    """Convert directory name to title: hyphens/underscores → spaces, title case."""
    return name.replace("-", " ").replace("_", " ").title()


def _derive_page_title(file: str, file_labels: dict[str, str]) -> str:
    """Get page title from top-level object __label or derive from filename."""
    if file in file_labels:
        return file_labels[file]
    stem = PurePosixPath(file).stem
    if stem.endswith(".qmd"):
        stem = stem[:-4]
    return stem.replace("-", " ").replace("_", " ").title()


def _qmdc_to_nav_path(source_file: str) -> str:
    """Convert a workspace-relative .qmd.md path to its nav path.

    readme.qmd.md → index.md (MkDocs convention for directory index pages)
    other.qmd.md → other.md
    """
    md_path = source_file.replace(".qmd.md", ".md")
    parts = PurePosixPath(md_path).parts
    if parts and parts[-1] == "readme.md":
        return str(PurePosixPath(*parts[:-1], "index.md")) if len(parts) > 1 else "index.md"
    return md_path
