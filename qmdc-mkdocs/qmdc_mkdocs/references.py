"""[[#id]] resolution algorithm — position-based replacement using parser data.

Uses __references[] from the parser (with exact line/col positions) to replace
references with Markdown links or broken-link spans. No regex scanning of content.
"""

from __future__ import annotations

from dataclasses import dataclass
from html import escape as html_escape
from pathlib import PurePosixPath
from typing import TYPE_CHECKING, Any

from .paths import qmdc_to_md_path

if TYPE_CHECKING:
    from .database import WorkspaceData


@dataclass
class Replacement:
    """A text replacement at a specific position."""

    line: int  # 1-based
    start_col: int  # 0-based
    end_col: int  # 0-based, exclusive
    new_text: str


def obj_label(obj: dict[str, Any]) -> str:
    """The display label for an object: its ``__label`` or, failing that, ``__id``."""
    return obj.get("__label") or obj["__id"]


def broken_link_span(text: str) -> str:
    """A non-navigable dead-link span (no href), HTML-escaping the visible text.

    Used both for genuinely broken references and for references whose target
    page is excluded from the site. Escaping mirrors syntax.py, which escapes
    every id/kind it emits into HTML.
    """
    return f'<span class="broken-link">{html_escape(text)}</span>'


def resolve_references(
    lines: list[str],
    file_objects: list[dict[str, Any]],
    source_file: str,
    ws_data: WorkspaceData,
    ignore_patterns: list[str] | None = None,
    namespace_prefix: str | None = None,
) -> list[str]:
    """Replace [[#ref]] patterns using parser position data.

    Args:
        lines: Raw file content as list of lines.
        file_objects: Objects from parser for this file (with __references).
        source_file: Workspace-relative path (e.g. 'api/endpoints.qmd.md').
        ws_data: WorkspaceData with DB for target lookup.
        ignore_patterns: Optional .qmdc-mkdocs.ignore patterns. A reference whose
            resolved target page is excluded renders as a non-navigable dead link
            (broken-link span) instead of a Markdown link — its page isn't built.
        namespace_prefix: When building a single namespace, the prefix being
            built (e.g. 'tracking'); used so exclusion matches the same paths the
            converter drops (see ignore.is_excluded).

    Returns:
        Modified lines with references replaced by Markdown links or broken spans.
    """
    from .ignore import is_excluded

    replacements: list[Replacement] = []

    # Build set of lines inside EXAMPLE code fences (don't resolve refs there)
    # Regular code fences DO have refs resolved (per QMD.md spec)
    fence_lines: set[int] = set()  # 1-based line numbers
    in_example_fence = False
    for i, line_text in enumerate(lines):
        stripped = line_text.strip()
        if stripped.startswith("```"):
            if in_example_fence:
                # Closing an example fence
                in_example_fence = False
                fence_lines.add(i + 1)
            elif "example" in stripped:
                # Opening an example fence
                in_example_fence = True
                fence_lines.add(i + 1)
        elif in_example_fence:
            fence_lines.add(i + 1)

    for obj in file_objects:
        refs = obj.get("__references", [])
        if not refs:
            continue

        source_global_id = _compute_global_id(obj)

        for ref in refs:
            line = ref["line"]  # 1-based
            start = ref["start_col"]  # 0-based
            end = ref["end_col"]  # 0-based, exclusive
            raw = ref["raw"]
            target_str = ref["target"]  # e.g. "#storage:users"

            # Skip references inside code fences
            if line in fence_lines:
                continue

            # Look up target via edges table first, then fallback to direct lookup
            target_obj = _find_target(source_global_id, target_str, ws_data, source_file)

            if target_obj is None:
                # Broken reference (escape the raw [[#...]] text)
                new_text = broken_link_span(raw)
            elif is_excluded(target_obj["__file"], ignore_patterns or [], namespace_prefix):
                # Target object exists but its page is excluded from the site.
                # Render a non-navigable dead link (label only) so it doesn't
                # 404 / warn in MkDocs, but the reader still sees what it meant.
                new_text = broken_link_span(obj_label(target_obj))
            else:
                target_file = target_obj["__file"]
                # Anchor is the object's full __id (dot-composed for nested
                # objects), matching the heading anchor emitted by syntax.py.
                target_id = target_obj["__id"]
                label = obj_label(target_obj)
                rel_path = _compute_relative_path(source_file, target_file)
                new_text = f"[{label}]({rel_path}#{target_id})"

            replacements.append(Replacement(line, start, end, new_text))

    # Sort back-to-front (later positions first) so replacements don't shift earlier ones
    replacements.sort(key=lambda r: (r.line, r.start_col), reverse=True)

    # Deduplicate: if multiple replacements target the same position, keep only the first
    seen_positions: set[tuple[int, int]] = set()
    unique_replacements: list[Replacement] = []
    for rep in replacements:
        key = (rep.line, rep.start_col)
        if key not in seen_positions:
            seen_positions.add(key)
            unique_replacements.append(rep)
    replacements = unique_replacements

    # Apply replacements
    result = list(lines)
    for rep in replacements:
        idx = rep.line - 1  # Convert to 0-based
        if 0 <= idx < len(result):
            line_text = result[idx]
            result[idx] = line_text[: rep.start_col] + rep.new_text + line_text[rep.end_col :]

    return result


def _find_target(
    source_global_id: str,
    target_str: str,
    ws_data: WorkspaceData,
    source_file: str = "",
) -> dict[str, Any] | None:
    """Find target object for a reference using the edges table or direct lookup.

    The edges table has already-resolved edges. We look for an edge from this
    source object to a target matching the reference.
    """
    # Strip leading # from target
    ref_id = target_str.lstrip("#")

    # Parse reference parts to extract the target object ID
    parts = ref_id.split(":")
    target_id = parts[-1]  # Last part is always the ID

    # Try edges table first (most reliable — parser already resolved).
    # Match the target by __id OR __local_id: hierarchical (dot-id) objects have
    # __id = "parent.leaf" but are referenced by their __local_id ("leaf").
    if source_global_id:
        rows = ws_data.query(
            "SELECT t.__file, t.__id, t.__label FROM edges e "
            "JOIN objects t ON e.target_id = t.__global_id "
            "WHERE e.source_id = ? AND (t.__id = ? OR t.__local_id = ?)",
            params=(source_global_id, target_id, target_id),
        )
        if rows:
            return rows[0]

    # Fallback: direct object lookup (for cases where edge wasn't created)
    return _direct_lookup(parts, ws_data, source_file)


def _direct_lookup(
    parts: list[str],
    ws_data: WorkspaceData,
    source_file: str = "",
) -> dict[str, Any] | None:
    """Resolve a reference using the parser's built-in resolution logic.

    Delegates to QmdcDatabase._resolve_target_global_id which handles:
    1. Same workspace/namespace by __id
    2. Same workspace, any namespace by __id
    3. Any workspace by __id
    4. __local_id fallback (for hierarchical IDs)

    For multi-part references (namespace:id, Kind:id, namespace:Kind:id),
    reconstructs the target_id and uses the same resolution.
    """
    db = ws_data.db

    # Get workspace_id
    ws_rows = ws_data.query(
        "SELECT __id FROM objects WHERE __kind = '__Workspace' LIMIT 1"
    )
    workspace_id = ws_rows[0]["__id"] if ws_rows else ""

    # Derive source namespace from source_file (first path component if it's a namespace)
    source_namespace = ""
    if source_file and "/" in source_file:
        candidate_ns = source_file.split("/")[0]
        ns_check = ws_data.query(
            "SELECT 1 FROM objects WHERE __id = ? AND __kind = '__Namespace' LIMIT 1",
            params=(candidate_ns,),
        )
        if ns_check:
            source_namespace = candidate_ns

    if len(parts) == 1:
        global_id = db._resolve_target_global_id(parts[0], workspace_id, source_namespace)
        if not global_id:
            # Cross-namespace __local_id fallback. The workspace resolver resolves a
            # bare [[#leaf]] to a hierarchical (dot-id) object by its __local_id even
            # across namespaces; _resolve_target_global_id only does same-namespace
            # __local_id, so replicate the workspace-wide unique-leaf match here.
            rows = ws_data.query(
                "SELECT __file, __id, __label FROM objects "
                "WHERE __workspace = ? AND __local_id = ?",
                params=(workspace_id, parts[0]),
            )
            if len(rows) == 1:
                return rows[0]
    elif len(parts) == 2:
        if parts[0] and parts[0][0].isupper():
            # Kind:id
            rows = ws_data.query(
                "SELECT __file, __id, __label FROM objects "
                "WHERE __kind = ? AND (__id = ? OR __local_id = ?)",
                params=(parts[0], parts[1], parts[1]),
            )
            return rows[0] if len(rows) == 1 else None
        else:
            # namespace:id
            global_id = db._resolve_target_global_id(parts[1], workspace_id, parts[0])
    elif len(parts) == 3:
        # namespace:Kind:id
        rows = ws_data.query(
            "SELECT __file, __id, __label FROM objects "
            "WHERE __namespace = ? AND __kind = ? AND (__id = ? OR __local_id = ?)",
            params=(parts[0], parts[1], parts[2], parts[2]),
        )
        return rows[0] if len(rows) == 1 else None
    else:
        return None

    if not global_id:
        return None

    rows = ws_data.query(
        "SELECT __file, __id, __label FROM objects WHERE __global_id = ?",
        params=(global_id,),
    )
    return rows[0] if rows else None


def _compute_global_id(obj: dict[str, Any]) -> str:
    """Compute __global_id for an object (matches QmdcDatabase format).

    Format: workspace:namespace:id or workspace::id (double colon for empty namespace)
    """
    from qmdc.db import QmdcDatabase

    workspace = obj.get("__workspace", "")
    namespace = obj.get("__namespace", "")
    obj_id = obj.get("__id", "")
    return QmdcDatabase.compute_global_id(workspace, namespace, obj_id)


def _compute_relative_path(source_file: str, target_file: str) -> str:
    """Compute a relative **Markdown link** path from source file to target file.

    Both source_file and target_file are workspace-relative paths like
    'storage/tables.qmd.md'. We convert both to their output .md equivalents
    (readme.qmd.md → index.md) and compute the relative path between them.

    This produces FILE-style paths (e.g. ``../index.md``) used in the rendered
    Markdown body, where MkDocs (with ``use_directory_urls``) rewrites ``.md``
    links to their final directory URLs. This is intentionally different from
    ``converter._compute_href``, which emits pre-resolved directory-style URLs
    (e.g. ``../foo/``) for raw HTML in the templates (MkDocs does NOT rewrite
    those). Do not merge the two — they target different consumers.
    """
    # Convert both to output paths (.qmd.md → .md, readme → index)
    source_md = qmdc_to_md_path(source_file)
    target_md = qmdc_to_md_path(target_file)

    source_dir = PurePosixPath(source_md).parent
    target_path = PurePosixPath(target_md)

    # Find common prefix length
    s_parts = source_dir.parts
    t_parts = target_path.parts

    common = 0
    for a, b in zip(s_parts, t_parts, strict=False):
        if a == b:
            common += 1
        else:
            break

    # Go up from source dir, then down to target
    up = len(s_parts) - common
    down = "/".join(t_parts[common:])
    return ("../" * up) + down
