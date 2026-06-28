"""QMDC Workspace - Multi-file parsing with cross-file references."""

import fnmatch
import os
import re
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

from .parser import is_inside_backticks, parse

_WORKSPACE_MARKER_RE = re.compile(r"\[\[[^\]]+:\s*__Workspace\]\]")
_REF_FULL_RE = re.compile(r"\[\[#([^\]]+)\]\]")
_REF_INNER_RE = re.compile(r"\[\[#([^\]]+)\]\]")  # same as _REF_FULL_RE (kept for clarity)
_HEADING_DEF_RE = re.compile(r"^\s*#+\s+.*\[\[([^\]]+)\]\]")


def _build_definition_line_index(content: str) -> tuple[dict[tuple[str, str], int], dict[str, int]]:
    """
    Build an index of object definition lines by scanning headings once.

    Returns:
        - by_id_kind: (id, kind) -> line
        - by_id: id -> line (for definitions without explicit kind)
    """
    by_id_kind: dict[tuple[str, str], int] = {}
    by_id: dict[str, int] = {}

    for i, line in enumerate(content.splitlines(), 1):
        m = _HEADING_DEF_RE.match(line)
        if not m:
            continue

        inner = m.group(1).strip()
        if not inner:
            continue

        # Definition formats:
        # - [[id:Kind]]
        # - [[id]]
        if ":" in inner:
            obj_id, obj_kind = inner.split(":", 1)
            obj_id = obj_id.strip()
            obj_kind = obj_kind.strip()
            if obj_id and obj_kind:
                by_id_kind.setdefault((obj_id, obj_kind), i)
                by_id.setdefault(obj_id, i)
        else:
            by_id.setdefault(inner, i)

    return by_id_kind, by_id


@dataclass
class WorkspaceError:
    """Validation error in workspace."""

    type: str  # broken_link, duplicate_id, ambiguous_reference
    message: str
    file: str | None = None
    line: int | None = None
    object_id: str | None = None
    field_name: str | None = None
    reference: str | None = None
    candidates: list[str] | None = None
    severity: str = "error"  # error, warning


@dataclass
class WorkspaceResult:
    """Result of workspace parsing."""

    root: str
    workspace_id: str | None
    files: list[str]
    objects: list[dict[str, Any]]
    index: dict[str, Any] = field(default_factory=dict)
    errors: list[WorkspaceError] = field(default_factory=list)


def _extract_namespace_id(namespace_ref: str) -> str:
    """Extract namespace ID - now just returns the value as-is (plain ID format)."""
    return namespace_ref or ""


def find_workspace_root(start_path: str) -> str | None:
    """
    Find workspace root by searching for readme.qmd.md with __Workspace object.

    Args:
        start_path: Starting directory or file path

    Returns:
        Absolute path to workspace root directory, or None if not found
    """
    path = Path(start_path).resolve()

    # If it's a file, start from its directory
    if path.is_file():
        path = path.parent

    # Search up the tree
    while path != path.parent:
        readme = path / "readme.qmd.md"
        if readme.exists():
            content = readme.read_text(encoding="utf-8")
            # Check if it contains __Workspace kind (shared marker regex, allows
            # optional whitespace after the colon, e.g. `[[id: __Workspace]]`).
            if _WORKSPACE_MARKER_RE.search(content):
                return str(path)
        path = path.parent

    return None


def find_nested_workspace_roots(root_path: str) -> list[Path]:
    """
    Find all nested workspace roots within a directory.
    Respects .qmdcignore patterns.

    Returns:
        List of absolute paths to directories containing [[id:__Workspace]].
    """
    root = Path(root_path).resolve()
    roots: list[Path] = []
    ignore_patterns = load_qmdcignore(root)

    # Use os.walk so we can prune ignored directories early (rglob can't).
    for dirpath, dirnames, filenames in os.walk(root, topdown=True):
        dir_path = Path(dirpath)

        # Prune ignored directories
        pruned: list[str] = []
        for d in dirnames:
            d_path = dir_path / d
            # Use a dummy child to make subtree ignores match directories reliably.
            if is_ignored(d_path / "__dummy__", root, ignore_patterns):
                pruned.append(d)
        for d in pruned:
            dirnames.remove(d)

        if "readme.qmd.md" not in filenames:
            continue

        readme = dir_path / "readme.qmd.md"

        # Skip root readme
        if readme.parent == root:
            continue

        if is_ignored(readme, root, ignore_patterns):
            continue

        content = readme.read_text(encoding="utf-8")
        if _WORKSPACE_MARKER_RE.search(content):
            roots.append(readme.parent)
            # No need to descend into a nested workspace root for discovery
            dirnames[:] = []

    return roots


def scan_workspace(root_path: str, exclude_nested: bool = True) -> list[str]:
    """
    Scan workspace directory for all *.qmd.md files.
    Respects .qmdcignore patterns.

    Args:
        root_path: Workspace root directory
        exclude_nested: If True, exclude files from nested workspaces

    Returns:
        List of relative file paths
    """
    root = Path(root_path).resolve()
    ignore_patterns = load_qmdcignore(root)

    files, _nested_roots = _scan_workspace_files_and_nested_roots(
        root=root, ignore_patterns=ignore_patterns, exclude_nested=exclude_nested
    )

    return files


def _scan_workspace_files_and_nested_roots(
    root: Path,
    ignore_patterns: list[str],
    exclude_nested: bool,
) -> tuple[list[str], list[Path]]:
    """
    Scan workspace directory for all *.qmd.md files and discover nested workspaces.
    Uses os.walk so we can prune ignored directories early.

    Returns:
        (files, nested_workspace_roots)
    """
    files: list[str] = []
    nested_roots: set[Path] = set()

    # Walk once and prune ignored directories so big trees (e.g. tasks/**) don't dominate scan time.
    for dirpath, dirnames, filenames in os.walk(root, topdown=True):
        dir_path = Path(dirpath)

        # If we are inside a nested workspace and exclude_nested is enabled, prune immediately.
        if exclude_nested and any(dir_path.is_relative_to(nr) for nr in nested_roots):
            dirnames[:] = []
            continue

        # Prune ignored directories early
        pruned: list[str] = []
        for d in dirnames:
            d_path = dir_path / d
            if is_ignored(d_path / "__dummy__", root, ignore_patterns):
                pruned.append(d)
        for d in pruned:
            dirnames.remove(d)

        # Detect nested workspace roots (directory readme contains __Workspace)
        if exclude_nested and dir_path != root and "readme.qmd.md" in filenames:
            readme = dir_path / "readme.qmd.md"
            if not is_ignored(readme, root, ignore_patterns):
                content = readme.read_text(encoding="utf-8")
                if _WORKSPACE_MARKER_RE.search(content):
                    nested_roots.add(dir_path)
                    # Exclude nested workspace directory entirely (including its readme)
                    dirnames[:] = []
                    continue

        for filename in filenames:
            if not filename.endswith(".qmd.md"):
                continue
            path = dir_path / filename

            # Check .qmdcignore before processing
            if is_ignored(path, root, ignore_patterns):
                continue

            rel_path = path.relative_to(root)
            files.append(str(rel_path))

    # Sort for deterministic order (readme.qmd.md first in each directory)
    def sort_key(f: str) -> tuple[str, int, str]:
        parts = Path(f).parts
        dir_path = "/".join(parts[:-1]) if len(parts) > 1 else ""
        filename = parts[-1]
        # readme.qmd.md comes first (priority 0), others alphabetically (priority 1)
        priority = 0 if filename == "readme.qmd.md" else 1
        return (dir_path, priority, filename)

    return (sorted(files, key=sort_key), sorted(nested_roots))


def _find_workspace_object(objects: list[dict[str, Any]]) -> dict[str, Any] | None:
    """Find __Workspace object in parsed objects."""
    for obj in objects:
        if obj.get("__kind") == "__Workspace":
            return obj
    return None


def _find_namespace_object(objects: list[dict[str, Any]]) -> dict[str, Any] | None:
    """Find __Namespace object in parsed objects."""
    for obj in objects:
        if obj.get("__kind") == "__Namespace":
            return obj
    return None


def _get_line_number(content: str, obj: dict[str, Any]) -> int:
    """
    Get line number where object is defined.

    Searches for the heading with object's __id and __kind.
    """
    obj_id = obj.get("__id", "")
    obj_kind = obj.get("__kind", "")

    by_id_kind, by_id = _build_definition_line_index(content)
    if obj_id and obj_kind:
        hit = by_id_kind.get((obj_id, obj_kind))
        if isinstance(hit, int):
            return hit
    if obj_id:
        hit = by_id.get(obj_id)
        if isinstance(hit, int):
            return hit

    return 1  # Default to line 1 if not found


def parse_workspace(root_path: str) -> WorkspaceResult:
    """
    Parse entire workspace.

    Args:
        root_path: Workspace root directory

    Returns:
        WorkspaceResult with all objects, index, and errors
    """
    root = Path(root_path).resolve()
    ignore_patterns = load_qmdcignore(root)
    files, nested_workspace_roots = _scan_workspace_files_and_nested_roots(
        root=root, ignore_patterns=ignore_patterns, exclude_nested=True
    )

    # Check for nested workspaces (this is an error)
    nested_workspace_errors: list[WorkspaceError] = []

    for nested_root in nested_workspace_roots:
        nested_readme = nested_root / "readme.qmd.md"
        rel_path = str(nested_readme.relative_to(root))
        content = nested_readme.read_text(encoding="utf-8")
        objects = parse(content, format="standard")
        ws_obj = _find_workspace_object(objects)

        if ws_obj:
            ws_line = ws_obj.get("__line")
            nested_workspace_errors.append(
                WorkspaceError(
                    type="nested_workspace",
                    message=f"Nested workspace '{ws_obj.get('__id')}' found inside workspace. "
                    "Workspaces cannot be nested.",
                    file=rel_path,
                    line=ws_line if isinstance(ws_line, int) else _get_line_number(content, ws_obj),
                    object_id=ws_obj.get("__id"),
                    severity="error",
                )
            )

    all_objects: list[dict[str, Any]] = []
    workspace_id: str | None = None
    workspace_ref: str | None = None

    # First pass: find workspace and namespace definitions
    namespace_map: dict[str, str] = {}  # dir_path -> namespace_id

    for file_path in files:
        full_path = root / file_path
        file_dir = str(Path(file_path).parent)
        if file_dir == ".":
            file_dir = ""

        # Check if this is a readme.qmd.md file (in any directory)
        is_readme = Path(file_path).name == "readme.qmd.md"

        # Check for __Workspace in readme
        if is_readme:
            content = full_path.read_text(encoding="utf-8")
            objects = parse(content, format="standard")

            ws_obj = _find_workspace_object(objects)
            if ws_obj:
                workspace_id = ws_obj.get("__id")
                workspace_ref = workspace_id  # Store plain ID without [[#...]]

            ns_obj = _find_namespace_object(objects)
            if ns_obj:
                namespace_map[file_dir] = ns_obj.get("__id")
        else:
            # Check for __Workspace in non-readme file (this is an error),
            # but only if marker exists.
            content = full_path.read_text(encoding="utf-8")
            if "__Workspace" in content and _WORKSPACE_MARKER_RE.search(content):
                objects = parse(content, format="full")
                ws_obj = _find_workspace_object(objects)
                if ws_obj:
                    ws_id = ws_obj.get("__id", "")
                    ws_line = ws_obj.get("__line")
                    # Check .qmdcignore before adding error
                    if not is_ignored(full_path, root, ignore_patterns):
                        nested_workspace_errors.append(
                            WorkspaceError(
                                type="workspace_in_wrong_file",
                                message=(
                                    f"Workspace '{ws_id}' must be defined in readme.qmd.md, "
                                    f"not in '{file_path}'."
                                ),
                                file=file_path,
                                line=(
                                    ws_line
                                    if isinstance(ws_line, int)
                                    else _get_line_number(content, ws_obj)
                                ),
                                object_id=ws_id,
                            )
                        )

    # Second pass: parse all files with full metadata (including __references)
    namespace_for_dir: dict[str, str | None] = {}
    for file_path in files:
        full_path = root / file_path
        content = full_path.read_text(encoding="utf-8")
        by_id_kind, by_id = _build_definition_line_index(content)
        objects = parse(content, format="full")

        file_dir = str(Path(file_path).parent)
        if file_dir == ".":
            file_dir = ""

        # Find namespace for this file's directory
        if file_dir in namespace_for_dir:
            namespace_id = namespace_for_dir[file_dir]
        else:
            namespace_id: str | None = None
            check_dir = file_dir
            while check_dir:
                if check_dir in namespace_map:
                    namespace_id = namespace_map[check_dir]
                    break
                # Go up one directory
                check_dir = str(Path(check_dir).parent)
                if check_dir == ".":
                    check_dir = ""
                    break

            # Also check current directory
            if namespace_id is None and file_dir in namespace_map:
                namespace_id = namespace_map[file_dir]
            namespace_for_dir[file_dir] = namespace_id

        # Add metadata to each object
        for obj in objects:
            obj["__file"] = file_path
            # parse(..., format="full") already provides __line using tokenizer offsets.
            # Only fallback to expensive regex scan if missing.
            if not isinstance(obj.get("__line"), int):
                obj_id = obj.get("__id", "")
                obj_kind = obj.get("__kind", "")
                line = None
                if obj_id and obj_kind:
                    line = by_id_kind.get((obj_id, obj_kind))
                if line is None and obj_id:
                    line = by_id.get(obj_id)
                obj["__line"] = int(line) if isinstance(line, int) else 1

            # Add workspace reference (except for __Workspace itself)
            if obj.get("__kind") != "__Workspace" and workspace_ref:
                obj["__workspace"] = workspace_ref

            # Add namespace reference (except for __Namespace itself and root objects)
            # Namespace is determined from file directory (already determined above)
            if obj.get("__kind") not in ("__Workspace", "__Namespace") and namespace_id:
                obj["__namespace"] = namespace_id  # Store plain ID
            elif obj.get("__kind") == "__Namespace" and workspace_ref:
                obj["__workspace"] = workspace_ref

        # Filter out __Workspace objects from non-readme files
        is_readme = Path(file_path).name == "readme.qmd.md"
        if not is_readme:
            objects = [obj for obj in objects if obj.get("__kind") != "__Workspace"]

        # Extract __ParsingError objects and convert to WorkspaceError
        parsing_error_objs = [obj for obj in objects if obj.get("__kind") == "__ParsingError"]
        regular_objects = [obj for obj in objects if obj.get("__kind") != "__ParsingError"]

        for err_obj in parsing_error_objs:
            err_type = err_obj.get("type", "unknown")
            # Build message from all non-system fields
            detail_parts = []
            for k, v in err_obj.items():
                if k.startswith("__") or k in ("type", "line"):
                    continue
                detail_parts.append(f"{k}: {v}")
            detail = ", ".join(detail_parts) if detail_parts else ""
            msg = f"{err_type}: {detail}" if detail else err_type

            nested_workspace_errors.append(
                WorkspaceError(
                    type=err_type,
                    message=msg,
                    file=file_path,
                    line=err_obj.get("line"),
                    reference=err_obj.get("reference"),
                    object_id=err_obj.get("object"),
                    field_name=err_obj.get("field"),
                )
            )

        all_objects.extend(regular_objects)

    # If no explicit workspace found but we have QMD.md files, create virtual workspace
    # BUT: Don't create virtual workspace if there's a workspace_in_wrong_file error
    has_wrong_file_error = any(e.type == "workspace_in_wrong_file" for e in nested_workspace_errors)

    if workspace_id is None and files and not has_wrong_file_error:
        # Use folder name as workspace ID
        virtual_ws_id = root.name or "workspace"

        workspace_id = virtual_ws_id
        workspace_ref = virtual_ws_id

        # Create __Workspace object for virtual workspace
        ws_obj = {
            "__id": virtual_ws_id,
            "__kind": "__Workspace",
            "__file": "",
            "__line": 1,
            "name": virtual_ws_id,
        }

        # Add __Workspace object to all_objects (at the beginning)
        all_objects.insert(0, ws_obj)

        # Update all existing objects to have __workspace field
        for obj in all_objects:
            kind = obj.get("__kind", "")
            # Add __workspace to all objects except __Workspace itself
            if kind != "__Workspace":
                obj["__workspace"] = virtual_ws_id

    # Build index
    index = build_index(all_objects)

    # Validate
    validation_errors = validate_workspace(all_objects, index, root_path=str(root))
    errors = nested_workspace_errors + validation_errors

    return WorkspaceResult(
        root=str(root),
        workspace_id=workspace_id,
        files=files,
        objects=all_objects,
        index=index,
        errors=errors,
    )


def build_index(objects: list[dict[str, Any]]) -> dict[str, Any]:
    """
    Build workspace index for fast lookups.

    Returns:
        {
            "by_id": {"id": [obj1, obj2, ...]},  # objects with same id
            "by_global_id": {"namespace:Kind:id": obj},  # unique global id
            "by_kind": {"Kind": [obj1, obj2, ...]},
            "by_file": {"file.qmd.md": [obj1, obj2, ...]},
            "by_namespace": {"namespace": [obj1, obj2, ...]},
            "by_local_id": {"local_id": [obj1, obj2, ...]}  # objects with same __local_id
        }
    """
    by_id: dict[str, list[dict[str, Any]]] = {}
    by_global_id: dict[str, dict[str, Any]] = {}
    by_kind: dict[str, list[dict[str, Any]]] = {}
    by_file: dict[str, list[dict[str, Any]]] = {}
    by_namespace: dict[str, list[dict[str, Any]]] = {}
    by_local_id: dict[str, list[dict[str, Any]]] = {}

    for obj in objects:
        obj_id = obj.get("__id")
        obj_kind = obj.get("__kind", "")
        obj_file = obj.get("__file", "")
        obj_namespace = obj.get("__namespace")

        # Skip internal system objects, but index user-facing system kinds
        # __Workspace, __Namespace, __Document, __Object are user-facing and should be indexable
        user_facing_system_kinds = ("__Workspace", "__Namespace", "__Document", "__Object")
        if obj_kind.startswith("__") and obj_kind not in user_facing_system_kinds:
            continue

        if obj_id:
            by_id.setdefault(obj_id, []).append(obj)

            # Global ID: namespace:Kind:id
            ns_id = obj_namespace or ""  # Already plain ID

            global_id = f"{ns_id}:{obj_kind}:{obj_id}" if ns_id else f":{obj_kind}:{obj_id}"
            by_global_id[global_id] = obj

        if obj_kind:
            by_kind.setdefault(obj_kind, []).append(obj)

        if obj_file:
            by_file.setdefault(obj_file, []).append(obj)

        if obj_namespace:
            ns_key = obj_namespace
            by_namespace.setdefault(ns_key, []).append(obj)

        # Index by __local_id for fallback resolution
        local_id = obj.get("__local_id")
        if local_id:
            by_local_id.setdefault(local_id, []).append(obj)

    return {
        "by_id": by_id,
        "by_global_id": by_global_id,
        "by_kind": by_kind,
        "by_file": by_file,
        "by_namespace": by_namespace,
        "by_local_id": by_local_id,
    }


def _strip_backticks(s: str) -> str:
    """Remove content inside backticks (inline code) to avoid extracting escaped refs."""
    return re.sub(r"`[^`]*`", "", s)


def _extract_references(obj: dict[str, Any]) -> list[tuple[str, str]]:
    """
    Extract all [[#...]] references from object fields.
    References inside backticks are ignored (escaped).

    Returns:
        List of (field_name, reference) tuples
    """
    refs: list[tuple[str, str]] = []

    def extract_from_value(field_name: str, value: Any) -> None:
        if isinstance(value, str):
            # Strip backtick content before extracting refs
            stripped_value = _strip_backticks(value)
            # Find all [[#...]] references
            for match in _REF_FULL_RE.finditer(stripped_value):
                refs.append((field_name, match.group(0)))
        elif isinstance(value, list):
            for item in value:
                extract_from_value(field_name, item)
        elif isinstance(value, dict):
            for k, v in value.items():
                extract_from_value(f"{field_name}.{k}", v)

    for key, value in obj.items():
        if not key.startswith("__"):
            extract_from_value(key, value)

    return refs


def _parse_reference(ref: str) -> tuple[str | None, str | None, str]:
    """
    Parse reference like [[#ns:Kind:id]] or [[#id]].

    Returns:
        (namespace, kind, id)
    """
    # Remove [[# and ]]
    match = _REF_INNER_RE.match(ref)
    if not match:
        return None, None, ref

    inner = match.group(1)
    parts = inner.split(":")

    if len(parts) == 3:
        return parts[0], parts[1], parts[2]
    elif len(parts) == 2:
        # Could be Kind:id or namespace:id
        # Assume Kind:id if first part looks like a Kind (capitalized)
        if parts[0][0].isupper():
            return None, parts[0], parts[1]
        else:
            return parts[0], None, parts[1]
    else:
        return None, None, parts[0]


def resolve_reference(
    ref: str,
    from_obj: dict[str, Any],
    index: dict[str, Any],
) -> dict[str, Any] | list[dict[str, Any]] | None:
    """
    Resolve a reference to target object(s).

    Args:
        ref: Reference like [[#id]] or [[#ns:Kind:id]]
        from_obj: Object containing the reference (for context)
        index: Workspace index

    Returns:
        Target object, list of candidates (ambiguous), or None (broken)
    """
    ns, kind, obj_id = _parse_reference(ref)

    by_id = index.get("by_id", {})
    by_global_id = index.get("by_global_id", {})

    # If fully qualified, use global_id lookup
    if ns and kind:
        global_id = f"{ns}:{kind}:{obj_id}"
        return by_global_id.get(global_id)

    # Get all objects with this id
    candidates = by_id.get(obj_id, [])

    if not candidates:
        return None  # Broken link

    # Filter by kind if specified
    if kind:
        candidates = [c for c in candidates if c.get("__kind") == kind]

    # Filter by namespace if specified
    if ns:
        candidates = [c for c in candidates if c.get("__namespace") == ns]  # Plain ID comparison

    if len(candidates) == 1:
        return candidates[0]
    elif len(candidates) > 1:
        return candidates  # Ambiguous
    else:
        return None  # Broken link


def _extract_id_from_reference(target: str) -> str:
    """
    Extract the actual ID from a reference target.
    Handles formats like: #id, Kind:id, namespace:id, namespace:Kind:id
    """
    # Remove [[# and ]]
    match = _REF_INNER_RE.match(target)
    if not match:
        return target

    inner = match.group(1)
    parts = inner.split(":")

    # Return the last part (the ID)
    return parts[-1] if parts else inner


def validate_workspace(
    objects: list[dict[str, Any]],
    index: dict[str, Any],
    root_path: str | None = None,
) -> list[WorkspaceError]:
    """
    Validate workspace for errors.

    Checks:
    - Broken links
    - Duplicate IDs (same id, different files or different kinds)
    - Ambiguous references
    """
    errors: list[WorkspaceError] = []

    # Build a quick ID lookup set for O(1) parent resolution
    all_ids: set[str] = {obj.get("__id", "") for obj in objects}

    # Phase 3: Resolve dot-ID parents
    # Objects with __local_id == __id and "." in __id are dot-ID declarations
    # that need parent resolution from the global object graph
    for obj in objects:
        obj_id = obj.get("__id", "")
        local_id = obj.get("__local_id")
        # Dot-ID detection: __local_id equals __id AND contains a dot
        # (same-file children have __local_id != __id)
        if local_id is None or local_id != obj_id or "." not in obj_id:
            continue
        # Already has a parent (shouldn't happen, but guard)
        if obj.get("__parent"):
            continue
        # Split on last dot to get parent path
        last_dot = obj_id.rfind(".")
        parent_path = obj_id[:last_dot]
        # Look up parent in the ID set
        if parent_path in all_ids:
            obj["__parent"] = f"[[#{parent_path}]]"
        else:
            errors.append(
                WorkspaceError(
                    type="broken_parent",
                    message=f"Parent object '{parent_path}' not found in workspace",
                    file=obj.get("__file"),
                    line=obj.get("__line"),
                    object_id=obj_id,
                    severity="error",
                )
            )

    # Build index of all objects by id, kind, and namespace for validation
    # Format: id -> [(file, kind, namespace, line), ...]
    objects_by_id: dict[str, list[tuple[str, str, str, int]]] = {}
    # Quick lookup: id -> first object with that id (for field-level resolution)
    obj_lookup: dict[str, dict[str, Any]] = {}

    for obj in objects:
        obj_id = obj.get("__id")
        obj_file = obj.get("__file")
        obj_line = obj.get("__line")

        if not obj_id or not obj_file or obj_line is None:
            continue

        obj_kind = obj.get("__kind", "__Object")
        obj_namespace = obj.get("__namespace", "")
        ns_id = _extract_namespace_id(obj_namespace)

        objects_by_id.setdefault(obj_id, []).append((obj_file, obj_kind, ns_id, obj_line))
        if obj_id not in obj_lookup:
            obj_lookup[obj_id] = obj

    # Check for duplicate IDs (same id, different files or same file)
    # Skip system objects (__Document, __TextBlock) as they are auto-generated per file
    for obj_id, locations in objects_by_id.items():
        # Skip system objects with auto-generated IDs
        is_system_object = any(
            kind == "__Document" or kind == "__TextBlock" for _, kind, _, _ in locations
        )
        if is_system_object:
            continue

        if len(locations) > 1:
            # Check if duplicates are in different files
            files = {file for file, _, _, _ in locations}
            if len(files) > 1:
                # Duplicate ID across files
                for file, _kind, _ns, line in locations[1:]:
                    candidates = [f"{f}:{line_num}" for f, _, _, line_num in locations]
                    errors.append(
                        WorkspaceError(
                            type="duplicate_id",
                            message=f"Duplicate ID '{obj_id}' found in multiple files",
                            file=file,
                            line=line,
                            object_id=obj_id,
                            candidates=candidates,
                            severity="error",
                        )
                    )
            else:
                # Same file - check if different kinds
                kinds = {kind for _, kind, _, _ in locations}
                if len(kinds) > 1:
                    # Same ID, different kinds - ambiguous
                    first_kind = locations[0][1]
                    for file, kind, _ns, line in locations[1:]:
                        candidates = [f"{f}:{k}:{line_num}" for f, k, _, line_num in locations]
                        errors.append(
                            WorkspaceError(
                                type="duplicate_id",
                                message=(
                                    f"Duplicate ID '{obj_id}' with different kinds: "
                                    f"{first_kind} and {kind}"
                                ),
                                file=file,
                                line=line,
                                object_id=obj_id,
                                candidates=candidates,
                                severity="error",
                            )
                        )

    # Check for broken links and ambiguous references using __references from objects
    for obj in objects:
        # Get namespace of current object
        obj_namespace = obj.get("__namespace", "")
        obj_ns_id = _extract_namespace_id(obj_namespace)
        # A __Namespace root object has no own __namespace, but it defines a
        # namespace and resolves its references within it (its own __id).
        # Mirror the Rust resolver, which derives the effective namespace from
        # the file directory for such objects.
        if not obj_ns_id and obj.get("__kind") == "__Namespace":
            obj_ns_id = obj.get("__id", "")

        # Get all references from this object using __references field
        refs = obj.get("__references", [])
        if not isinstance(refs, list):
            continue

        for ref_info in refs:
            if not isinstance(ref_info, dict):
                continue

            # Use 'raw' field if available (contains full [[#...]]), otherwise use 'target'
            target = ref_info.get("raw") or ref_info.get("target")
            line = ref_info.get("line")

            if not target or line is None:
                continue

            # If target doesn't have [[#...]], add it
            if not target.startswith("[["):
                target = f"[[{target}]]" if target.startswith("#") else f"[[#{target}]]"

            obj_id = obj.get("__id", "")
            obj_file = obj.get("__file", "")

            # Parse reference target to extract namespace, kind, and id
            ref_ns, ref_kind, ref_id = _parse_reference(target)

            # Find matching objects
            matching_objects = []
            if ref_id in objects_by_id:
                for file, kind, ns, ref_line in objects_by_id[ref_id]:
                    # If reference specifies namespace, must match exactly
                    if ref_ns is not None and ns != ref_ns:
                        continue
                    # If reference specifies kind, must match
                    if ref_kind is not None and kind != ref_kind:
                        continue
                    # If reference doesn't specify namespace, include all candidates
                    matching_objects.append((file, kind, ns, ref_line))

            # If reference doesn't specify namespace, prefer objects in same namespace
            # According to spec: "current namespace first, then other files in the same namespace"
            # Ambiguous only if:
            # 1. Multiple objects in current namespace, OR
            # 2. No objects in current namespace but multiple in other namespaces
            if ref_ns is None:
                if obj_ns_id:
                    # Prefer objects from same namespace
                    same_ns = [
                        (f, k, n, line_num)
                        for f, k, n, line_num in matching_objects
                        if n == obj_ns_id
                    ]
                    resolved_objects = same_ns or matching_objects
                else:
                    # Object is in root namespace - all matching objects are candidates
                    resolved_objects = matching_objects
            else:
                # Reference specifies namespace - use all matching objects
                resolved_objects = matching_objects

            # Check if reference is inside backticks (inline code) - skip validation
            if root_path and obj_file:
                try:
                    file_path = Path(root_path) / obj_file
                    if file_path.exists():
                        file_content = file_path.read_text(encoding="utf-8")
                        file_lines = file_content.splitlines()
                        if line > 0 and line <= len(file_lines):
                            orig_line = file_lines[line - 1]
                            # Find position of reference in line
                            raw_ref = ref_info.get("raw") or target
                            ref_pos = orig_line.find(raw_ref)
                            if ref_pos >= 0:
                                # Check if reference is inside backticks (single or double)
                                # Use is_inside_backticks function which handles double backticks
                                if is_inside_backticks(orig_line, ref_pos):
                                    continue
                                # Also check if reference is between double backticks (``...``)
                                # Find all pairs of double backticks and check if ref is inside
                                double_backtick_pairs = list(re.finditer(r"``", orig_line))
                                skip_validation = False
                                for i in range(0, len(double_backtick_pairs), 2):
                                    if i + 1 < len(double_backtick_pairs):
                                        start_pos = double_backtick_pairs[i].start()
                                        end_pos = double_backtick_pairs[i + 1].start()
                                        if start_pos < ref_pos < end_pos:
                                            # Reference is inside double backticks - skip validation
                                            skip_validation = True
                                            break
                                if skip_validation:
                                    continue
                except (OSError, UnicodeDecodeError):
                    # If we can't read the file, continue with validation
                    pass

            if not resolved_objects:
                # __local_id fallback: try to resolve by __local_id within same namespace
                by_local_id = index.get("by_local_id", {})
                local_candidates = by_local_id.get(ref_id, [])

                # Filter by target namespace (ref_ns if explicit, else source obj namespace)
                target_ns = ref_ns if ref_ns is not None else obj_ns_id
                if target_ns:
                    local_candidates = [
                        c
                        for c in local_candidates
                        if _extract_namespace_id(c.get("__namespace", "")) == target_ns
                    ]
                else:
                    # Root-level: only match other root-level objects
                    local_candidates = [c for c in local_candidates if not c.get("__namespace")]

                if len(local_candidates) == 1:
                    # Resolved via __local_id — no error
                    resolved_objects = [
                        (
                            local_candidates[0].get("__file", ""),
                            local_candidates[0].get("__kind", ""),
                            _extract_namespace_id(local_candidates[0].get("__namespace", "")),
                            local_candidates[0].get("__line", 1),
                        )
                    ]
                elif len(local_candidates) > 1:
                    # Ambiguous by __local_id
                    candidates = []
                    for c in local_candidates:
                        c_ns = _extract_namespace_id(c.get("__namespace", ""))
                        c_kind = c.get("__kind", "")
                        if c_ns:
                            candidates.append(f"{c_ns}:{c_kind}:{c.get('__id', '')}")
                        else:
                            candidates.append(f"{c_kind}:{c.get('__id', '')}")
                    errors.append(
                        WorkspaceError(
                            type="ambiguous_reference",
                            message=(
                                f"Ambiguous reference '{target}'"
                                " - multiple objects match by __local_id"
                            ),
                            file=obj_file,
                            line=line,
                            object_id=obj_id,
                            reference=target,
                            candidates=candidates,
                            severity="error",
                        )
                    )
                    continue  # Skip further processing for this ref
                # else: local_candidates is empty, fall through to existing
                # broken_link / field-ref logic

            if not resolved_objects:
                # Try field-level resolution: if ref_id contains a dot,
                # split on last dot and check if prefix is a valid object
                # AND the field actually exists on that object
                is_field_ref = False
                if "." in ref_id:
                    last_dot = ref_id.rfind(".")
                    obj_prefix = ref_id[:last_dot]
                    field_part = ref_id[last_dot + 1 :]
                    if obj_prefix in objects_by_id:
                        candidate_obj = obj_lookup.get(obj_prefix)
                        if (
                            candidate_obj
                            and field_part in candidate_obj
                            and not field_part.startswith("__")
                        ):
                            is_field_ref = True

                if not is_field_ref:
                    # Check if the object exists in a different namespace
                    # (cross-namespace hint for better error messages)
                    hint = ""
                    by_local_id_map = index.get("by_local_id", {})
                    other_ns_local = by_local_id_map.get(ref_id, [])
                    if other_ns_local:
                        # Filter to objects in OTHER namespaces
                        if obj_ns_id:
                            others = [
                                c
                                for c in other_ns_local
                                if _extract_namespace_id(c.get("__namespace", "")) != obj_ns_id
                            ]
                        else:
                            others = [c for c in other_ns_local if c.get("__namespace")]
                        if others:
                            other_ns = _extract_namespace_id(others[0].get("__namespace", ""))
                            other_id = others[0].get("__id", ref_id)
                            hint = f". Did you mean [[#{other_ns}:{other_id}]]?"

                    if not hint:
                        # Check by __id in other namespaces
                        other_ns_id = objects_by_id.get(ref_id, [])
                        if other_ns_id:
                            if obj_ns_id:
                                others = [
                                    c
                                    for c in other_ns_id
                                    if _extract_namespace_id(c.get("__namespace", "")) != obj_ns_id
                                ]
                            else:
                                others = [c for c in other_ns_id if c.get("__namespace")]
                            if others:
                                other_ns = _extract_namespace_id(others[0].get("__namespace", ""))
                                other_id = others[0].get("__id", ref_id)
                                hint = f". Did you mean [[#{other_ns}:{other_id}]]?"

                    # Broken link - reference not found
                    errors.append(
                        WorkspaceError(
                            type="broken_link",
                            message=f"Object '{ref_id}' not found{hint}",
                            file=obj_file,
                            line=line,
                            object_id=obj_id,
                            reference=target,
                            severity="error",
                        )
                    )
            elif len(resolved_objects) == 1:
                # Object found — check for ambiguous_field_reference
                # If ref_id contains a dot, check if the field-path interpretation
                # also resolves to a scalar field (not a reference to this object)
                if "." in ref_id:
                    last_dot = ref_id.rfind(".")
                    obj_prefix = ref_id[:last_dot]
                    field_part = ref_id[last_dot + 1 :]
                    if obj_prefix in objects_by_id:
                        candidate_obj = obj_lookup.get(obj_prefix)
                        if (
                            candidate_obj
                            and field_part in candidate_obj
                            and not field_part.startswith("__")
                        ):
                            field_val = candidate_obj.get(field_part)
                            # Ambiguous if field value is NOT a reference to the object
                            if field_val != f"[[#{ref_id}]]":
                                field_val_repr = (
                                    repr(field_val)
                                    if len(repr(field_val)) < 40
                                    else repr(field_val)[:37] + "..."
                                )
                                errors.append(
                                    WorkspaceError(
                                        type="ambiguous_field_reference",
                                        message=(
                                            f"Reference '{target}' cannot be unequivocally "
                                            f"resolved to an object or a field"
                                        ),
                                        file=obj_file,
                                        line=line,
                                        object_id=obj_id,
                                        reference=target,
                                        candidates=[
                                            f"object with __id '{ref_id}'",
                                            (
                                                f"field '{field_part}' on object"
                                                f" '{obj_prefix}' (value: {field_val_repr})"
                                            ),
                                        ],
                                        severity="error",
                                    )
                                )
            elif len(resolved_objects) > 1:
                # Ambiguous reference - multiple matching objects
                kinds = {kind for _, kind, _, _ in resolved_objects}
                namespaces = {ns for _, _, ns, _ in resolved_objects}

                is_ambiguous = False
                if ref_kind is not None and ref_ns is not None:
                    is_ambiguous = False  # Fully qualified, should not be ambiguous
                elif len(kinds) > 1:
                    is_ambiguous = True  # Different kinds
                elif len(namespaces) > 1:
                    is_ambiguous = True  # Different namespaces

                if is_ambiguous:
                    candidates = []
                    for _file, kind, ns, _ref_line in resolved_objects:
                        if ns:
                            candidates.append(f"{ns}:{kind}:{ref_id}")
                        else:
                            candidates.append(f"{kind}:{ref_id}")
                    errors.append(
                        WorkspaceError(
                            type="ambiguous_reference",
                            message=f"Ambiguous reference '{target}' - multiple objects match",
                            file=obj_file,
                            line=line,
                            object_id=obj_id,
                            reference=target,
                            candidates=candidates,
                            severity="error",
                        )
                    )

    return errors


def workspace_to_json(result: WorkspaceResult) -> dict[str, Any]:
    """Convert WorkspaceResult to JSON-serializable dict."""
    # Output-shape (QMD-59): never emit a bare `workspace: null` when workspaces
    # were actually resolved. Derive workspace id(s) from the resolved objects:
    #   - walk-up/self (single workspace_id set)  -> "workspace": id
    #   - walk-down, exactly one sub-workspace     -> "workspace": that id
    #   - walk-down, multiple sub-workspaces       -> omit "workspace",
    #                                                 add "workspaces": [ids...]
    out: dict[str, Any] = {"root": result.root}

    if result.workspace_id:
        out["workspace"] = result.workspace_id
    else:
        ws_ids = sorted(
            {
                obj.get("__id")
                for obj in result.objects
                if obj.get("__kind") == "__Workspace" and obj.get("__id")
            }
        )
        if len(ws_ids) == 1:
            out["workspace"] = ws_ids[0]
        elif len(ws_ids) > 1:
            out["workspaces"] = ws_ids
        else:
            out["workspace"] = None

    out.update(
        {
            "files": result.files,
            "objects": result.objects,
            "index": {
                "by_global_id": {
                    k: v.get("__id")
                    for k, v in result.index.get("by_global_id", {}).items()  # Plain IDs
                },
                "by_kind": {
                    k: [o.get("__id") for o in v]  # Plain IDs
                    for k, v in result.index.get("by_kind", {}).items()
                },
                "by_file": {
                    k: [o.get("__id") for o in v]  # Plain IDs
                    for k, v in result.index.get("by_file", {}).items()
                },
            },
            "errors": [
                {
                    "type": e.type,
                    "message": e.message,
                    "file": e.file,
                    "line": e.line,
                    "object": e.object_id,
                    "field": e.field_name,
                    "reference": e.reference,
                    "candidates": e.candidates,
                    "severity": e.severity,
                }
                for e in result.errors
            ],
        }
    )
    return out


def find_all_workspace_dirs(root_path: str) -> list[Path]:
    """
    Find all workspace directories (directories containing readme.qmd.md with __Workspace).

    Returns:
        List of paths to directories containing workspace definition.
    """
    root = Path(root_path).resolve()
    workspace_dirs: list[Path] = []

    for path in root.rglob("readme.qmd.md"):
        content = path.read_text(encoding="utf-8")
        if _WORKSPACE_MARKER_RE.search(content):
            workspace_dirs.append(path.parent)

    return workspace_dirs


def load_qmdcignore(root_path: Path) -> list[str]:
    """
    Load .qmdcignore patterns from root directory.

    Returns:
        List of glob patterns to ignore
    """
    qmdcignore_path = root_path / ".qmdcignore"

    if not qmdcignore_path.exists():
        return []

    patterns = []
    content = qmdcignore_path.read_text(encoding="utf-8")

    for line in content.splitlines():
        line = line.strip()

        # Skip empty lines and comments
        if not line or line.startswith("#"):
            continue

        # If pattern ends with /, replace with /** to match all files within
        pattern = f"{line}**" if line.endswith("/") else line

        patterns.append(pattern)

    return patterns


def is_ignored(path: Path, root_path: Path, patterns: list[str]) -> bool:
    """
    Check if a path should be ignored based on glob patterns.

    Args:
        path: Path to check
        root_path: Root directory for relative path calculation
        patterns: List of glob patterns from .qmdcignore

    Returns:
        True if path matches any ignore pattern
    """
    if not patterns:
        return False

    try:
        rel_path = path.relative_to(root_path)
    except ValueError:
        return False

    # Convert to string with forward slashes for consistent matching
    rel_str = str(rel_path).replace("\\", "/")

    for pattern in patterns:
        # Handle ** pattern - replace with * for fnmatch
        # **/pattern matches pattern at any depth
        # pattern/** matches everything under pattern
        if "**" in pattern:
            # Replace **/ with */ and ** with * for fnmatch
            normalized_pattern = pattern.replace("**/", "*").replace("**", "*")
            if fnmatch.fnmatch(rel_str, normalized_pattern):
                return True
            # Also try matching just the filename
            if fnmatch.fnmatch(path.name, normalized_pattern):
                return True
        else:
            if fnmatch.fnmatch(rel_str, pattern):
                return True
            # Also try matching just the filename
            if fnmatch.fnmatch(path.name, pattern):
                return True

    return False


def parse_all_workspaces(root_path: str) -> WorkspaceResult:
    """
    Parse all workspaces found in a directory tree (non-nested).

    If root_path itself is a workspace, parse only that one.
    If root_path contains multiple workspace directories, parse all of them.
    Respects .qmdcignore patterns at the root level.

    Args:
        root_path: Directory to scan for workspaces

    Returns:
        WorkspaceResult with combined objects from all workspaces
    """
    root = Path(root_path).resolve()

    # Load .qmdcignore patterns
    ignore_patterns = load_qmdcignore(root)

    # Check if root_path itself is a workspace
    root_readme = root / "readme.qmd.md"
    if root_readme.exists() and not is_ignored(root_readme, root, ignore_patterns):
        content = root_readme.read_text(encoding="utf-8")
        if _WORKSPACE_MARKER_RE.search(content):
            # Root is a workspace - use single workspace parsing
            return parse_workspace(str(root))

    # Root is not a workspace - find all workspaces in subdirectories
    all_workspace_dirs = find_all_workspace_dirs(str(root))

    # Filter out ignored workspaces
    workspace_dirs = [
        ws_dir
        for ws_dir in all_workspace_dirs
        if not is_ignored(ws_dir / "readme.qmd.md", root, ignore_patterns)
    ]

    if not workspace_dirs:
        # No explicit workspaces found - check if root has .qmd.md files
        # If yes, treat root as a virtual workspace
        # IMPORTANT: Must respect .qmdcignore when checking for files
        has_qmdc_files = False
        for qmdc_file in root.rglob("*.qmd.md"):
            # Check .qmdcignore before considering file
            if not is_ignored(qmdc_file, root, ignore_patterns):
                # Check max depth (5 levels)
                depth = len(qmdc_file.relative_to(root).parts)
                if depth <= 5:
                    has_qmdc_files = True
                    break

        if has_qmdc_files:
            # Treat root as a virtual workspace
            return parse_workspace(str(root))

        # No workspaces and no QMD.md files - return empty result
        return WorkspaceResult(
            root=str(root),
            workspace_id=None,
            files=[],
            objects=[],
            errors=[],
        )

    # Parse each workspace and combine results
    all_objects: list[dict[str, Any]] = []
    all_files: list[str] = []
    all_errors: list[WorkspaceError] = []

    for ws_dir in workspace_dirs:
        ws_result = parse_workspace(str(ws_dir))

        # Adjust __file paths in objects to be relative to root_path
        for obj in ws_result.objects:
            if "__file" in obj:
                full_path = ws_dir / obj["__file"]
                try:
                    rel_path = full_path.relative_to(root)
                    obj["__file"] = str(rel_path)
                except ValueError:
                    pass

        all_objects.extend(ws_result.objects)

        # Make file paths relative to root_path
        for file in ws_result.files:
            full_path = ws_dir / file
            try:
                rel_path = full_path.relative_to(root)
                all_files.append(str(rel_path))
            except ValueError:
                # File is not relative to root, skip
                pass

        # Adjust error file paths to be relative to root_path
        for error in ws_result.errors:
            if error.file:
                try:
                    full_path = ws_dir / error.file
                    rel_path = full_path.relative_to(root)
                    error.file = str(rel_path)
                except ValueError:
                    pass
            all_errors.append(error)

    # After parsing explicit workspaces, check for orphan .qmd.md files
    # (files outside any workspace directory that should be loaded too)
    orphan_files = []
    for qmdc_file in root.rglob("*.qmd.md"):
        # Exclude files inside explicit workspace directories
        is_inside_workspace = any(qmdc_file.is_relative_to(ws_dir) for ws_dir in workspace_dirs)
        # Apply .qmdcignore filtering
        if not is_inside_workspace and not is_ignored(qmdc_file, root, ignore_patterns):
            orphan_files.append(qmdc_file)

    if orphan_files:
        # Parse orphan files as if they belong to a virtual workspace
        virtual_ws_id = root.name or "workspace"

        for file_path in orphan_files:
            try:
                content = file_path.read_text(encoding="utf-8")
                objects = parse(content, random_seed=666)

                rel_file = str(file_path.relative_to(root))
                is_readme = file_path.name == "readme.qmd.md"

                # Add __file and __workspace metadata to each object
                for obj in objects:
                    # Skip __Workspace objects from non-readme files
                    if not is_readme and obj.get("__kind") == "__Workspace":
                        ws_id = obj.get("__id", "")
                        # Check .qmdcignore before adding error
                        if not is_ignored(file_path, root, ignore_patterns):
                            all_errors.append(
                                WorkspaceError(
                                    type="workspace_in_wrong_file",
                                    message=(
                                        f"Workspace '{ws_id}' must be defined in readme.qmd.md, "
                                        f"not in '{rel_file}'."
                                    ),
                                    file=rel_file,
                                    line=_get_line_number(content, obj),
                                    object_id=ws_id,
                                    severity="error",
                                )
                            )
                        continue  # Skip this object

                    obj["__file"] = rel_file
                    obj["__workspace"] = virtual_ws_id  # Store plain ID
                    all_objects.append(obj)

                all_files.append(rel_file)
            except Exception:
                # Skip files that can't be read
                pass

    return WorkspaceResult(
        root=str(root),
        workspace_id=None,  # Multiple workspaces, no single ID
        files=all_files,
        objects=all_objects,
        errors=all_errors,
    )


def resolve_workspace(path: str) -> WorkspaceResult:
    """Unified workspace resolver (QMD-59).

    Lets the user run workspace parse/validate/query from ANY directory without
    bailing out:

    1. Walk-UP: if ``path`` itself, or any ancestor directory, is a workspace
       (has ``readme.qmd.md`` with ``[[__Workspace]]``), parse that workspace via
       ``parse_workspace``. This preserves genuine nested-workspace detection.
    2. Walk-DOWN: otherwise ``path`` is a non-workspace container; ``parse_all_workspaces``
       resolves each contained sub-workspace independently (union of errors), or
       falls back to a virtual workspace for orphan files.

    The previous CLI behaviour ("No workspace found", exit 1) is removed: a
    non-workspace container now descends into the sub-workspaces it contains.
    """
    root = find_workspace_root(path)
    if root:
        return parse_workspace(root)
    return parse_all_workspaces(path)
