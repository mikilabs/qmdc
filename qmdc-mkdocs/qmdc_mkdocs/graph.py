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
    # Which breadcrumb item is the current page: "workspace" or "namespace" on that
    # object's own readme, otherwise "file".
    current_level: str = "file"
    # Output-relative paths of the workspace and namespace index pages, for breadcrumb
    # links; None when the page is not in the built site or is the current page.
    workspace_page: str | None = None
    namespace_page: str | None = None


# A page is named by its top-level object. System objects are skipped, except the
# __Namespace / __Workspace heading that names a namespace or workspace readme.
_PAGE_OBJECT_FILTER = "(__kind NOT GLOB '__*' OR __kind IN ('__Namespace', '__Workspace'))"


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

    # Breadcrumb ancestors: the files holding the __Workspace and __Namespace objects.
    ws_file = _get_index_file(ws_data, "__Workspace", None)
    ns_file = (
        _get_index_file(ws_data, "__Namespace", _namespace_id(source_file))
        if ns_label is not None
        else None
    )
    if source_file == ws_file:
        current_level = "workspace"
    elif source_file == ns_file:
        current_level = "namespace"
    else:
        current_level = "file"

    def _linkable(index_file: str | None) -> str | None:
        """Output path of an ancestor index page, if it is in the site and not this page."""
        if index_file is None or index_file == source_file:
            return None
        if namespace_prefix is not None:
            prefix = namespace_prefix + "/"
            if not index_file.startswith(prefix):
                return None  # outside the namespace being built
            out = index_file[len(prefix):]
        else:
            out = index_file
        if is_excluded(index_file, ignore_patterns or [], namespace_prefix):
            return None
        return out

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

    # Filter out hidden kinds from edges. An outgoing edge is listed from this
    # page whatever kind its source has, so only the target kind matters there.
    # An incoming edge from a hidden object is credited to the page that holds
    # it (the file's top-level object), once per page and edge type, so both
    # sides of the sidebar show the same page relationship.
    if hidden_kinds:
        links_to_rows = [r for r in links_to_rows if r["__kind"] not in hidden_kinds]
        linked_from_rows = _credit_hidden_sources(linked_from_rows, hidden_kinds, ws_data)

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
        current_level=current_level,
        workspace_page=_linkable(ws_file),
        namespace_page=_linkable(ns_file),
    )


def _get_page_object(source_file: str, ws_data: WorkspaceData) -> dict | None:
    """The top-level non-system object of a file (system objects never credited)."""
    rows = ws_data.query(
        """
        SELECT __id, __label, __kind, __file FROM objects
        WHERE __file = ? AND __kind NOT GLOB '__*'
        ORDER BY CAST(__level AS INTEGER) ASC, CAST(__line AS INTEGER) ASC
        LIMIT 1
        """,
        params=(source_file,),
    )
    return rows[0] if rows else None


def _credit_hidden_sources(
    rows: list[dict], hidden_kinds: list[str], ws_data: WorkspaceData
) -> list[dict]:
    """Replace incoming-edge sources of a hidden kind with their page object.

    Rows keep their order; a page credited twice for the same edge type is
    listed once, and a source whose page object is itself hidden is dropped.
    """
    pages: dict[str, dict | None] = {}
    seen: set[tuple[str, str, str]] = set()
    result: list[dict] = []
    for r in rows:
        if r["__kind"] in hidden_kinds:
            file = r["__file"]
            if file not in pages:
                pages[file] = _get_page_object(file, ws_data)
            page = pages[file]
            if page is None or page["__kind"] in hidden_kinds:
                continue
            r = {"edge_type": r["edge_type"], **page}
        key = (r["edge_type"], r["__id"], r["__file"])
        if key in seen:
            continue
        seen.add(key)
        result.append(r)
    return result


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

    rows = ws_data.query(
        "SELECT __label FROM objects WHERE __kind = '__Namespace' AND __id = ?",
        params=(_namespace_id(source_file),),
    )
    if rows and rows[0]["__label"]:
        return rows[0]["__label"]
    return None


def _namespace_id(source_file: str) -> str:
    """The namespace ID for a file: typically its first path component."""
    return str(PurePosixPath(source_file).parent).split("/")[0]


def _get_index_file(
    ws_data: WorkspaceData, kind: str, obj_id: str | None
) -> str | None:
    """The file holding the __Workspace object, or the __Namespace with ``obj_id``."""
    if obj_id is None:
        rows = ws_data.query(
            "SELECT __file FROM objects WHERE __kind = ? LIMIT 1", params=(kind,)
        )
    else:
        rows = ws_data.query(
            "SELECT __file FROM objects WHERE __kind = ? AND __id = ? LIMIT 1",
            params=(kind, obj_id),
        )
    return rows[0]["__file"] if rows and rows[0]["__file"] else None


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
    """Get the __label of the page's top-level object (see _PAGE_OBJECT_FILTER)."""
    rows = ws_data.query(
        f"""
        SELECT __label FROM objects
        WHERE __file = ? AND {_PAGE_OBJECT_FILTER}
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
    page object per file) instead of one query per file (avoids N+1).
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

    # Batch-resolve labels: the page's top-level object per file (same rule as
    # _get_file_label, see _PAGE_OBJECT_FILTER).
    label_rows = ws_data.query(
        f"""
        SELECT __file, __label FROM (
            SELECT __file, __label FROM objects
            WHERE ({file_filter}) AND {_PAGE_OBJECT_FILTER}
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
