"""Graph sidebar data computation — edges, siblings, breadcrumb."""

from __future__ import annotations

from dataclasses import dataclass
from pathlib import PurePosixPath
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from .database import WorkspaceData


@dataclass
class EdgeItem:
    """A single edge in the links-to or linked-from list."""

    edge_type: str
    obj_id: str
    label: str
    kind: str
    file: str


@dataclass
class SiblingItem:
    """A sibling file in the same directory."""

    file: str
    label: str
    is_current: bool


@dataclass
class GraphContext:
    """Full graph context for a page's sidebar."""

    workspace_label: str
    namespace_label: str | None
    file_label: str
    links_to: list[EdgeItem]
    linked_from: list[EdgeItem]
    siblings: list[SiblingItem]
    toc: list[dict]  # [{id, label, level}]


def compute_graph_context(
    source_file: str, ws_data: WorkspaceData, ignore_patterns: list[str] | None = None,
    hidden_kinds: list[str] | None = None,
    namespace_prefix: str | None = None,
) -> GraphContext:
    """Compute full graph context for a page.

    Args:
        source_file: Workspace-relative path (e.g. 'api/endpoints.qmd.md').
        ws_data: WorkspaceData with DB for SQL queries and objects_by_file for TOC.
        ignore_patterns: Optional list of .qmdc-mkdocs.ignore patterns to filter edges.
        hidden_kinds: Optional list of Kind names to exclude from sidebar edges.
        namespace_prefix: When building a single namespace, the prefix being built;
            edges/siblings pointing at excluded pages are dropped using the same
            rule as page generation (ignore.is_excluded).

    Returns:
        GraphContext with breadcrumb, edges, siblings, and TOC data.
    """
    from .ignore import is_excluded

    # Breadcrumb components
    ws_label = _get_workspace_label(ws_data)
    ns_label = _get_namespace_label(source_file, ws_data)
    file_label = _get_file_label(source_file, ws_data)

    # Outgoing edges (links-to): objects in THIS file reference objects in OTHER files
    links_to_rows = ws_data.query(
        """
        SELECT DISTINCT e.edge_type, t.__id, t.__label, t.__kind, t.__file
        FROM edges e
        JOIN objects s ON e.source_id = s.__global_id
        JOIN objects t ON e.target_id = t.__global_id
        WHERE s.__file = ? AND t.__file != ? AND t.__kind NOT GLOB '__*'
        ORDER BY e.edge_type, t.__label
        """,
        params=(source_file, source_file),
    )

    # Incoming edges (linked-from): objects in OTHER files reference objects in THIS file
    linked_from_rows = ws_data.query(
        """
        SELECT DISTINCT e.edge_type, s.__id, s.__label, s.__kind, s.__file
        FROM edges e
        JOIN objects s ON e.source_id = s.__global_id
        JOIN objects t ON e.target_id = t.__global_id
        WHERE t.__file = ? AND s.__file != ? AND s.__kind NOT GLOB '__*'
        ORDER BY e.edge_type, s.__label
        """,
        params=(source_file, source_file),
    )

    # Siblings: other files in same directory
    siblings = _get_siblings(source_file, ws_data)

    # TOC: headings on this page (from objects_by_file, level >= 2, non-system)
    toc = _get_toc(source_file, ws_data)

    # Filter out edges pointing to/from excluded pages (same rule as page gen)
    if ignore_patterns:
        links_to_rows = [
            r for r in links_to_rows
            if not is_excluded(r["__file"], ignore_patterns, namespace_prefix)
        ]
        linked_from_rows = [
            r for r in linked_from_rows
            if not is_excluded(r["__file"], ignore_patterns, namespace_prefix)
        ]
        siblings = [
            s for s in siblings
            if not is_excluded(s.file, ignore_patterns, namespace_prefix)
        ]

    # Filter out hidden kinds from edges
    if hidden_kinds:
        links_to_rows = [r for r in links_to_rows if r["__kind"] not in hidden_kinds]
        linked_from_rows = [r for r in linked_from_rows if r["__kind"] not in hidden_kinds]

    return GraphContext(
        workspace_label=ws_label,
        namespace_label=ns_label,
        file_label=file_label,
        links_to=[
            EdgeItem(
                edge_type=r["edge_type"],
                obj_id=r["__id"],
                label=r["__label"] or r["__id"],
                kind=r["__kind"],
                file=r["__file"],
            )
            for r in links_to_rows
        ],
        linked_from=[
            EdgeItem(
                edge_type=r["edge_type"],
                obj_id=r["__id"],
                label=r["__label"] or r["__id"],
                kind=r["__kind"],
                file=r["__file"],
            )
            for r in linked_from_rows
        ],
        siblings=siblings,
        toc=toc,
    )


def _get_workspace_label(ws_data: WorkspaceData) -> str:
    """Query the __Workspace object's __label."""
    rows = ws_data.query(
        "SELECT __label FROM objects WHERE __kind = '__Workspace' LIMIT 1"
    )
    if rows and rows[0]["__label"]:
        return rows[0]["__label"]
    return "Workspace"


def _get_namespace_label(source_file: str, ws_data: WorkspaceData) -> str | None:
    """Find the namespace for the file's directory, return its __label.

    Returns None if the file is at the workspace root (no namespace).
    """
    directory = str(PurePosixPath(source_file).parent)
    if directory == ".":
        return None

    # The namespace ID is typically the first path component
    ns_id = directory.split("/")[0]
    rows = ws_data.query(
        "SELECT __label FROM objects WHERE __kind = '__Namespace' AND __id = ?",
        params=(ns_id,),
    )
    if rows and rows[0]["__label"]:
        return rows[0]["__label"]
    return None


def _label_from_filename(source_file: str) -> str:
    """Derive a human label from a file path when no object label is available.

    `foo-bar.qmd.md` → `Foo Bar`. Handles the `.qmd.md` double extension
    correctly (PurePosixPath.stem only strips the final `.md`).
    """
    name = PurePosixPath(source_file).name
    # Strip the QMD.md double extension (.qmd.md) or a single .md
    if name.endswith(".qmd.md"):
        stem = name[: -len(".qmd.md")]
    elif name.endswith(".md"):
        stem = name[: -len(".md")]
    else:
        stem = PurePosixPath(name).stem
    return stem.replace("-", " ").replace("_", " ").title()


def _get_file_label(source_file: str, ws_data: WorkspaceData) -> str:
    """Get the top-level non-system object's __label on this file."""
    rows = ws_data.query(
        """
        SELECT __label FROM objects
        WHERE __file = ? AND __kind NOT GLOB '__*'
        ORDER BY CAST(__level AS INTEGER) ASC, CAST(__line AS INTEGER) ASC
        LIMIT 1
        """,
        params=(source_file,),
    )
    if rows and rows[0]["__label"]:
        return rows[0]["__label"]
    # Fallback: derive from filename
    return _label_from_filename(source_file)


def _get_siblings(source_file: str, ws_data: WorkspaceData) -> list[SiblingItem]:
    """Get other files in the same directory as the source file.

    Resolves all sibling labels in a single batch query (the top-level
    non-system object per file) instead of one query per file (avoids N+1).
    """
    directory = str(PurePosixPath(source_file).parent)

    if directory == ".":
        # Root-level files: match files without a directory separator
        file_filter = "__file NOT GLOB '*/*'"
        params: tuple = ()
    else:
        # Files in the same directory: match directory prefix but no deeper nesting
        file_filter = "__file GLOB ? AND __file NOT GLOB ?"
        params = (f"{directory}/*", f"{directory}/*/*")

    # All distinct files in the directory (no kind filter — a file may contain
    # only system objects, e.g. a workspace/namespace readme, and must still
    # appear as a sibling).
    file_rows = ws_data.query(
        f"""
        SELECT DISTINCT __file FROM objects
        WHERE {file_filter} AND __file IS NOT NULL AND __file != ''
        ORDER BY __file
        """,
        params=params,
    )

    # Batch-resolve labels: the first top-level non-system object per file.
    label_rows = ws_data.query(
        f"""
        SELECT __file, __label FROM (
            SELECT __file, __label FROM objects
            WHERE ({file_filter}) AND __kind NOT GLOB '__*'
            ORDER BY CAST(__level AS INTEGER) ASC, CAST(__line AS INTEGER) ASC
        )
        GROUP BY __file
        """,
        params=params,
    )
    labels_by_file = {r["__file"]: r["__label"] for r in label_rows if r["__label"]}

    siblings: list[SiblingItem] = []
    for row in file_rows:
        file_path = row["__file"]
        if not file_path:
            continue
        label = labels_by_file.get(file_path) or _label_from_filename(file_path)
        siblings.append(
            SiblingItem(
                file=file_path,
                label=label,
                is_current=(file_path == source_file),
            )
        )
    return siblings


def _get_toc(source_file: str, ws_data: WorkspaceData) -> list[dict]:
    """Get non-system headings on this page as TOC entries.

    Uses ws_data.objects_by_file for direct access to parser objects.
    Filters to __level >= 2 (excludes page title) and non-system kinds.

    Returns list of dicts with keys: id, label, level.
    """
    objects_by_file = getattr(ws_data, "objects_by_file", None)
    if objects_by_file is None:
        # Fallback to SQL query if objects_by_file not available
        rows = ws_data.query(
            """
            SELECT __id, __label, __level FROM objects
            WHERE __file = ? AND __kind NOT GLOB '__*'
              AND CAST(__level AS INTEGER) >= 2
            ORDER BY CAST(__line AS INTEGER) ASC
            """,
            params=(source_file,),
        )
        return [
            {
                "id": r["__id"],
                "label": r["__label"] or r["__id"],
                "level": int(r["__level"]) if r["__level"] else 2,
            }
            for r in rows
        ]

    file_objects = objects_by_file.get(source_file, [])
    toc: list[dict] = []
    for obj in file_objects:
        kind = obj.get("__kind", "")
        # Skip system kinds (starting with __)
        if kind.startswith("__"):
            continue
        level = obj.get("__level")
        if level is None:
            continue
        level_int = int(level) if not isinstance(level, int) else level
        # Only include level >= 2 (sub-headings, not page title)
        if level_int < 2:
            continue
        toc.append({
            "id": obj.get("__id", ""),
            "label": obj.get("__label") or obj.get("__id", ""),
            "level": level_int,
        })
    return toc
