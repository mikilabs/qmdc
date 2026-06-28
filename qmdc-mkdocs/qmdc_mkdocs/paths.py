"""Shared path conversion utilities for QMDC → MkDocs output paths."""

from pathlib import PurePosixPath


def qmdc_to_md_path(source_file: str) -> str:
    """Convert a workspace-relative .qmd.md path to its output .md path.

    readme.qmd.md → index.md (MkDocs convention for directory index pages)
    other.qmd.md → other.md
    """
    md_path = source_file.replace(".qmd.md", ".md")
    parts = PurePosixPath(md_path).parts
    if parts and parts[-1] == "readme.md":
        return str(PurePosixPath(*parts[:-1], "index.md")) if len(parts) > 1 else "index.md"
    return md_path


def md_to_url_path(md_path: str) -> str:
    """Convert .md file path to directory-style URL path.

    commands.md → commands/ (served as commands/)
    index.md → / (served at root)
    storage/index.md → storage/ (served at storage/)
    storage/tables.md → storage/tables/ (served as storage/tables/)
    """
    if not md_path or md_path == ".":
        return "/"
    p = PurePosixPath(md_path)
    if p.name == "index.md":
        return str(p.parent) + "/" if p.parent.parts else "/"
    else:
        stem = str(p.with_suffix(""))
        return stem + "/"
