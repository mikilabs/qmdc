"""QMDC Workspace - Multi-file parsing with cross-file references."""

import os
import re
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

from .ignore import IgnoreRule, is_ignored, load_qmdcignore
from .parser import is_inside_backticks, parse

_WORKSPACE_MARKER_RE = re.compile(r"\[\[[^\]]+:\s*__Workspace\]\]")

# Mirrors WORKSPACE_SCAN_MAX_DEPTH in the Rust parser (qmdc-rs/src/workspace.rs) and the
# depth documented in docs/mcp/readme.qmd.md. A caller-supplied path is scanned downward at
# most this many directory levels, so pointing a tool at a large checkout cannot turn one
# call into a full-tree crawl. The three implementations must share the number: a marker
# deeper than this is undiscovered everywhere, or the same `-w` invocation succeeds in one
# implementation and is a usage error in another.
WORKSPACE_SCAN_MAX_DEPTH = 5

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
    """Result of workspace parsing.

    ``root`` is the base every ``__file`` is relative to, as a canonical absolute path, or
    ``None`` when that base is virtual: the ``-w`` form, whose workspaces need not share any
    directory, puts each one at its own id instead (QMD-72).

    ``workspaces`` holds one ``{"id", "root", "path"}`` entry per workspace, ordered by
    ``path``: ``root`` is where the workspace really is and ``path`` is where it sits in the
    base (``""`` when it IS the base). A consumer locates any file with one rule: take the
    entry whose ``path`` is the longest prefix of ``__file`` and join its ``root`` with the
    rest.
    """

    root: str | None
    workspace_id: str | None
    files: list[str]
    objects: list[dict[str, Any]]
    index: dict[str, Any] = field(default_factory=dict)
    errors: list[WorkspaceError] = field(default_factory=list)
    workspaces: list[dict[str, str]] = field(default_factory=list)


def _canonical_slash(path: Path | str) -> str:
    """
    Canonical absolute form of ``path`` with ``/`` separators (QMD-72).

    Canonical rather than lexical, because the ``-w`` duplicate check already treats two
    spellings of one directory (a symlink and its target) as the same path; the reported
    root has to agree with that. Mirrors ``canonical_slash`` in the Rust parser.
    """
    return Path(path).resolve().as_posix()


def _single_entry(workspace_id: str | None, root: Path) -> list[dict[str, str]]:
    """The entry for a single-workspace result: the workspace IS the base (``path`` ``""``)."""
    if not workspace_id:
        return []
    return [{"id": workspace_id, "root": _canonical_slash(root), "path": ""}]


def _extract_namespace_id(namespace_ref: str) -> str:
    """Extract namespace ID - now just returns the value as-is (plain ID format)."""
    return namespace_ref or ""


def synthetic_id_stem(file_path: str, namespace_dir: str | None) -> str:
    """The readable, collision-free stem a file's synthesised ids are built from.

    The file's path relative to the directory that declared its namespace (relative to the
    workspace root when it has none), lowercased with every run of other characters folded
    to ``_``.

    A single-file ``parse`` has no workspace and no namespace, so it keeps the counter form it
    has always had - one document cannot collide with itself. Only the workspace layer, which
    is where two files meet and where the graph key ``<workspace>:<namespace>:<id>`` is formed,
    qualifies them.
    """
    rel = file_path
    if namespace_dir and rel.startswith(namespace_dir):
        rel = rel[len(namespace_dir) :].lstrip("/")
    if rel.endswith(".qmd.md"):
        rel = rel[: -len(".qmd.md")]
    out: list[str] = []
    pending_sep = False
    for ch in rel:
        lower = ch.lower()
        if ("a" <= lower <= "z") or lower.isdigit():
            if pending_sep and out:
                out.append("_")
            pending_sep = False
            out.append(lower)
        else:
            pending_sep = True
    return "".join(out)


def _synthetic_id_renames(
    objects: list[dict[str, Any]], file_path: str, namespace_dir: str | None
) -> dict[str, str]:
    """Map each synthesised ``doc_*`` / ``text_*`` id in one file to its path-derived replacement.

    Empty when the stem is unusable, which leaves the ids exactly as the parser produced them.
    """
    stem = synthetic_id_stem(file_path, namespace_dir)
    if not stem:
        return {}
    renames: dict[str, str] = {}
    for obj in objects:
        obj_id = obj.get("__id", "")
        kind = obj.get("__kind")
        if kind == "__Document":
            renames[obj_id] = f"doc_{stem}"
        elif kind == "__TextBlock":
            # `text_<n>` keeps its ordinal: a file can hold several text blocks.
            ordinal = obj_id[len("text_") :] if obj_id.startswith("text_") else obj_id
            renames[obj_id] = f"text_{stem}_{ordinal}"
    return renames


def _rename_reference(raw: str, renames: dict[str, str]) -> str | None:
    """Rewrite a ``[[#id]]`` reference when its target was renamed."""
    if not (raw.startswith("[[#") and raw.endswith("]]")):
        return None
    new = renames.get(raw[3:-2])
    return f"[[#{new}]]" if new else None


def _rename_synthetic_ids(obj: dict[str, Any], renames: dict[str, str]) -> None:
    """Apply the file's synthesised-id renames to one object.

    Its own id, the ``content`` array a ``__Document`` uses to list what it holds, and the
    ``__container`` every sibling points back with. Author-written references are deliberately
    NOT rewritten - a reference to an id that no longer exists has to surface as a broken link
    rather than be silently retargeted.
    """
    if not renames:
        return
    new_id = renames.get(obj.get("__id", ""))
    if new_id:
        obj["__id"] = new_id
    container = obj.get("__container")
    if isinstance(container, str):
        new_ref = _rename_reference(container, renames)
        if new_ref:
            obj["__container"] = new_ref
    content = obj.get("content")
    if isinstance(content, list):
        for i, item in enumerate(content):
            if isinstance(item, str):
                new_ref = _rename_reference(item, renames)
                if new_ref:
                    content[i] = new_ref


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
            if is_ignored(d_path, root, ignore_patterns, is_dir=True):
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
    ignore_patterns: list[IgnoreRule],
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
            if is_ignored(d_path, root, ignore_patterns, is_dir=True):
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


def _nested_workspace_message(nested_id: str, nested_rel_dir: str) -> str:
    """Wording of the ``nested_workspace`` report, mirroring ``nested_workspace_message`` in Rust.

    Nesting is reported because the outer scan leaves the inner workspace's files out, so they are
    missing from the graph the caller asked for -- not because the two may never coexist. They may:
    the container form composes both and drops this report, which is how a repository whose root is
    a workspace and which also holds a ``.qmdc`` model workspace is validated. The message states
    the consequence and the remedy rather than claiming nesting is forbidden (QMD-76).

    ``nested_rel_dir`` is the inner workspace's directory relative to the outer root, so the remedy
    is copy-pasteable and carries no absolute path.
    """
    return (
        f"Nested workspace '{nested_id}' inside this workspace: its files are excluded from this "
        f"graph. Validate both together: --with <root> --with <root>/{nested_rel_dir}"
    )


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
        rel_dir = nested_root.relative_to(root).as_posix()
        content = nested_readme.read_text(encoding="utf-8")
        objects = parse(content, format="standard")
        ws_obj = _find_workspace_object(objects)

        if ws_obj:
            ws_line = ws_obj.get("__line")
            nested_workspace_errors.append(
                WorkspaceError(
                    type="nested_workspace",
                    message=_nested_workspace_message(str(ws_obj.get("__id")), rel_dir),
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
    namespace_for_dir: dict[str, tuple[str | None, str | None]] = {}
    for file_path in files:
        full_path = root / file_path
        content = full_path.read_text(encoding="utf-8")
        by_id_kind, by_id = _build_definition_line_index(content)
        objects = parse(content, format="full")

        file_dir = str(Path(file_path).parent)
        if file_dir == ".":
            file_dir = ""

        # Find namespace for this file's directory, and the directory that declared it — the
        # second is what a synthesised id is made relative to.
        if file_dir in namespace_for_dir:
            namespace_id, namespace_dir = namespace_for_dir[file_dir]
        else:
            namespace_id: str | None = None
            namespace_dir: str | None = None
            check_dir = file_dir
            while check_dir:
                if check_dir in namespace_map:
                    namespace_id = namespace_map[check_dir]
                    namespace_dir = check_dir
                    break
                # Go up one directory
                check_dir = str(Path(check_dir).parent)
                if check_dir == ".":
                    check_dir = ""
                    break

            # Also check current directory
            if namespace_id is None and file_dir in namespace_map:
                namespace_id = namespace_map[file_dir]
                namespace_dir = file_dir
            namespace_for_dir[file_dir] = (namespace_id, namespace_dir)

        synthetic_renames = _synthetic_id_renames(objects, file_path, namespace_dir)

        # Add metadata to each object
        for obj in objects:
            _rename_synthetic_ids(obj, synthetic_renames)
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
        root=_canonical_slash(root),
        workspace_id=workspace_id,
        files=files,
        objects=all_objects,
        index=index,
        errors=errors,
        workspaces=_single_entry(workspace_id, root),
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
    Parse a reference target into (workspace, namespace, id).

    QMD-69: a reference target is a right-aligned suffix of the ``__global_id`` grammar
    ``workspace:namespace:id``, with an optional ``.field`` suffix carried inside the id.
    There is no ``Kind`` segment.

    * ``#id`` -> (None, None, id)
    * ``#ns:id`` -> (None, ns, id)
    * ``#ws:ns:id`` -> (ws, ns, id)
    * ``#ws::id`` -> (ws, "", id) -- an EMPTY namespace ELIDES rather than asserting the
      workspace root, so the target may live in any namespace of ``ws``; more than one
      candidate is an ambiguity.
    """
    # Remove [[# and ]]
    match = _REF_INNER_RE.match(ref)
    if not match:
        return None, None, ref

    inner = match.group(1)
    parts = inner.split(":")

    if len(parts) >= 3:
        # workspace:namespace:id -- the id keeps any further colons, as rsplit-style
        # parsing would; the middle segment may be empty (elided namespace).
        return parts[0], parts[1], ":".join(parts[2:])
    elif len(parts) == 2:
        return None, parts[0], parts[1]
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
    file_paths: dict[str, Path] | None = None,
) -> list[WorkspaceError]:
    """
    Validate workspace for errors.

    Checks:
    - Broken links
    - Duplicate IDs (same id, different files or different kinds)
    - Ambiguous references

    A referring object's source line is read from ``file_paths[__file]`` when that map is
    given, else from ``root_path / __file``. The map exists for a composed ``-w`` set
    (QMD-72), whose ``__file`` values are relative to a virtual base and so cannot be joined
    onto any directory.
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

    # Build index of all objects by id, kind, namespace and workspace for validation.
    # Format: id -> [(file, kind, namespace, workspace, line), ...]
    # QMD-69: the workspace is carried so a workspace-qualified reference can be enforced
    # and an unqualified one can be kept inside its own workspace.
    objects_by_id: dict[str, list[tuple[str, str, str, str, int]]] = {}
    # Quick lookup: id -> first object with that id (for field-level resolution)
    # QMD-69: a MULTIMAP, not `id -> first object`. The field-reference check has to pick a
    # candidate that satisfies the reference's qualifiers, and in a composed container the
    # same id legitimately exists in several workspaces.
    obj_lookup: dict[str, list[dict[str, Any]]] = {}

    for obj in objects:
        obj_id = obj.get("__id")
        obj_file = obj.get("__file")
        obj_line = obj.get("__line")

        if not obj_id or not obj_file or obj_line is None:
            continue

        obj_kind = obj.get("__kind", "__Object")
        obj_namespace = obj.get("__namespace", "")
        ns_id = _extract_namespace_id(obj_namespace)

        obj_ws = obj.get("__workspace", "") or ""
        objects_by_id.setdefault(obj_id, []).append((obj_file, obj_kind, ns_id, obj_ws, obj_line))
        obj_lookup.setdefault(obj_id, []).append(obj)

    # Check for duplicate IDs (same id, different files or same file)
    # Skip system objects (__Document, __TextBlock) as they are auto-generated per file
    #
    # QMD-67: duplicate identity is scoped by namespace. Two objects with the same
    # full __id in DIFFERENT namespaces are distinct (distinct rows in the DB, keyed
    # by (__workspace, __namespace, __id), and distinct reference targets). We group
    # by (namespace, id) here. This is a SEPARATE grouping from objects_by_id (bare-id
    # keyed), which must stay bare-id-keyed for reference resolution / __local_id
    # fallback / cross-namespace hints. Dropping __workspace is safe: validation runs
    # per-workspace.
    # Exclude system objects (__Document/__TextBlock/__ParsingError) PER LOCATION while
    # building the duplicate index, rather than skipping a whole (namespace, id) group if
    # any member is a system object. A group-level skip would mask a genuine user-object
    # duplicate whenever a system object happens to share the id in the same namespace.
    # (objects_by_id itself stays complete — reference resolution depends on it.)
    system_kinds = ("__Document", "__TextBlock", "__ParsingError")
    objects_by_ns_id: dict[tuple[str, str], list[tuple[str, str, str, str, int]]] = {}
    for obj_id, locations in objects_by_id.items():
        for loc in locations:
            _file, kind, namespace, _ws, _line = loc
            if kind in system_kinds:
                continue
            objects_by_ns_id.setdefault((namespace, obj_id), []).append(loc)

    for (_namespace, obj_id), locations in objects_by_ns_id.items():
        if len(locations) > 1:
            # Check if duplicates are in different files
            files = {file for file, _, _, _, _ in locations}
            if len(files) > 1:
                # Duplicate ID across files
                for file, _kind, _ns, _ws, line in locations[1:]:
                    candidates = [f"{f}:{line_num}" for f, _, _, _, line_num in locations]
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
                kinds = {kind for _, kind, _, _, _ in locations}
                if len(kinds) > 1:
                    # Same ID, different kinds - ambiguous
                    first_kind = locations[0][1]
                    for file, kind, _ns, _ws, line in locations[1:]:
                        candidates = [f"{f}:{k}:{line_num}" for f, k, _, _, line_num in locations]
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
        # QMD-69: the referring object's own workspace. Empty in a single-workspace parse,
        # where it correctly imposes no filter.
        obj_ws_id = obj.get("__workspace", "") or ""

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

            # Parse reference target to extract workspace, namespace and id
            ref_ws, ref_ns, ref_id = _parse_reference(target)

            # QMD-69 candidate filter. Each qualifier present narrows the search; the
            # WORKSPACE never widens past the referring object's own workspace unless the
            # reference names one.
            matching_objects = []
            if ref_id in objects_by_id:
                for file, kind, ns, ws, ref_line in objects_by_id[ref_id]:
                    if ref_ws is not None:
                        # `ws:...` -- only inside the named workspace.
                        if ws != ref_ws:
                            continue
                    # Unqualified: a cross-workspace reference MUST name its workspace, so
                    # an unqualified one stays local. The empty-workspace case keeps
                    # single-workspace parses working unchanged.
                    elif obj_ws_id and ws and ws != obj_ws_id:
                        continue
                    # An EMPTY namespace segment (`ws::id`) elides rather than
                    # asserting the root namespace: any namespace matches.
                    if ref_ns is not None and ref_ns != "" and ns != ref_ns:
                        continue
                    matching_objects.append((file, kind, ns, ws, ref_line))

            # If reference doesn't specify namespace, prefer objects in same namespace
            # According to spec: "current namespace first, then other files in the same namespace"
            # Ambiguous only if:
            # 1. Multiple objects in current namespace, OR
            # 2. No objects in current namespace but multiple in other namespaces
            if ref_ns is None:
                if obj_ns_id:
                    # Prefer objects from same namespace
                    same_ns = [
                        (f, k, n, w, line_num)
                        for f, k, n, w, line_num in matching_objects
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
            if obj_file and (file_paths is not None or root_path):
                try:
                    if file_paths is not None:
                        file_path = file_paths.get(obj_file)
                    else:
                        file_path = Path(str(root_path)) / obj_file
                    if file_path is not None and file_path.exists():
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

                # QMD-69: honour the reference's WORKSPACE qualifier here too. This path used
                # to ignore it entirely, so `[[#no_such_ws:ns:leaf]]` was accepted.
                local_candidates = [
                    c
                    for c in local_candidates
                    if _workspace_matches(ref_ws, c.get("__workspace", "") or "", obj_ws_id)
                ]

                # The NAMESPACE rule is deliberately NOT the elide/assert rule used by
                # _qualifiers_match: an unqualified reference resolves by __local_id only
                # inside the referring object's OWN namespace (the root namespace when it has
                # none). That is what keeps a bare [[#config]] at the workspace root from
                # reaching gateway.config in the `services` namespace.
                if ref_ns == "":
                    # `ws::id` -- namespace elided: any namespace of the named workspace.
                    pass
                else:
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
                    # QMD-69: the prefix object must itself satisfy the reference's
                    # qualifiers. This escape used to accept any object with a matching id
                    # and field, so a reference naming a workspace that does not exist -- or
                    # one that exists but does not hold the object -- was silently treated as
                    # a field reference and never reported.
                    for candidate_obj in obj_lookup.get(obj_prefix, []):
                        if not _qualifiers_match(
                            ref_ws,
                            ref_ns,
                            candidate_obj.get("__workspace", "") or "",
                            _extract_namespace_id(candidate_obj.get("__namespace", "")),
                            obj_ws_id,
                        ):
                            continue
                        if field_part in candidate_obj and not field_part.startswith("__"):
                            is_field_ref = True
                            break

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
                        # QMD-69: objects_by_id holds TUPLES
                        # (file, kind, namespace, workspace, line) -- not dicts. This block
                        # used to call .get() on them and abort the whole validation run
                        # with `'tuple' object has no attribute 'get'`. It only fired when
                        # a broken link had no by_local_id hint yet the bare id existed
                        # elsewhere, which is exactly what a workspace-qualified reference
                        # produces.
                        other_ns_id = objects_by_id.get(ref_id, [])
                        if other_ns_id:
                            if obj_ns_id:
                                others = [c for c in other_ns_id if c[2] != obj_ns_id]
                            else:
                                others = [c for c in other_ns_id if c[2]]
                            if others:
                                other_ns = others[0][2]
                                hint = f". Did you mean [[#{other_ns}:{ref_id}]]?"

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
                    # QMD-69: same qualifier rule as the field-ref escape above -- the prefix
                    # object considered here must be one the reference could actually name, or
                    # QMDC009 would be raised about an object in a workspace the reference
                    # never mentioned.
                    candidate_obj = next(
                        (
                            c
                            for c in obj_lookup.get(obj_prefix, [])
                            if _qualifiers_match(
                                ref_ws,
                                ref_ns,
                                c.get("__workspace", "") or "",
                                _extract_namespace_id(c.get("__namespace", "")),
                                obj_ws_id,
                            )
                        ),
                        None,
                    )
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
                kinds = {kind for _, kind, _, _, _ in resolved_objects}
                namespaces = {ns for _, _, ns, _, _ in resolved_objects}

                # QMD-69: there is no Kind segment to suppress ambiguity with, and an
                # elided namespace (`ws::id`) explicitly MAY match several namespaces --
                # which is an ambiguity, not a silent pick.
                is_ambiguous = len(kinds) > 1 or len(namespaces) > 1

                if is_ambiguous:
                    candidates = []
                    for _file, kind, ns, _ws, _ref_line in resolved_objects:
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
    # Output shape (QMD-72): one shape for every invocation. "workspaces" is always present
    # -- zero, one or many {id, root, path} entries -- replacing the QMD-59
    # "workspace": id / "workspaces": [ids] / "workspace": null trio a consumer had to tell
    # apart. "root" is the base every __file is relative to, or null when it is virtual (-w).
    out: dict[str, Any] = {"root": result.root, "workspaces": result.workspaces}

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
            # QMD-75: absent values are OMITTED, not emitted as null -- Rust and TypeScript
            # both leave them out, so a consumer reading this parser saw three extra keys.
            "errors": [
                {
                    k: v
                    for k, v in {
                        "type": e.type,
                        "message": e.message,
                        "file": e.file,
                        "line": e.line,
                        # QMD-77 C3: the same keys `workspace validate` uses. This envelope said
                        # `object`/`field` for the very same error, so a consumer reading both
                        # commands had to know two spellings of one field.
                        "objectId": e.object_id,
                        "fieldName": e.field_name,
                        "reference": e.reference,
                        "candidates": e.candidates,
                        "severity": e.severity,
                    }.items()
                    if v is not None
                }
                for e in result.errors
            ],
        }
    )
    return out


def find_all_workspace_dirs(root_path: str) -> list[Path]:
    """
    Find all workspace directories (directories containing readme.qmd.md with __Workspace).

    Unbounded depth. Use :func:`find_workspace_dirs_bounded` where a caller-supplied path
    must not turn into a full-tree crawl.

    Returns:
        List of paths to directories containing workspace definition.
    """
    root = Path(root_path).resolve()
    ignore_patterns = load_qmdcignore(root)
    workspace_dirs: list[Path] = []

    for path in root.rglob("readme.qmd.md"):
        # The root's .qmdcignore steers discovery, as it does in the Rust scan (QMD-73).
        if is_ignored(path, root, ignore_patterns):
            continue
        try:
            content = path.read_text(encoding="utf-8")
        except OSError:
            # An unreadable marker is a marker we cannot see; treat it as absent rather
            # than aborting the whole scan with an OS error. Mirrors the Rust scan, whose
            # `read_to_string` failure falls through the same way.
            continue
        if _WORKSPACE_MARKER_RE.search(content):
            workspace_dirs.append(path.parent)

    return workspace_dirs


def find_workspace_dirs_bounded(root_path: str, max_depth: int) -> list[Path]:
    """
    Depth-bounded variant of :func:`find_all_workspace_dirs` (QMD-72).

    Mirrors ``find_nested_workspace_roots_bounded`` in the Rust parser: the crawl is
    capped at ``max_depth`` directory levels below ``root_path``, so pointing a tool at a
    large checkout cannot turn one call into a full-tree crawl. The cap is part of the
    contract, not an optimisation — a marker deeper than this is NOT discovered, and all
    three implementations must agree on that, or the same ``-w`` invocation is a usage
    error in one and a successful composition in another.

    Returns:
        List of paths to directories containing workspace definition.
    """
    root = Path(root_path).resolve()
    ignore_patterns = load_qmdcignore(root)
    workspace_dirs: list[Path] = []

    for path in root.rglob("readme.qmd.md"):
        # rglob yields the readme itself; its depth below root is what the cap applies to.
        try:
            depth = len(path.relative_to(root).parts)
        except ValueError:
            continue
        if depth > max_depth:
            continue
        # The path's own .qmdcignore steers the scan, as in the Rust scan (QMD-73): without
        # it `-w` on a directory that ignores one of its two workspaces composed in Rust and
        # was refused here as "contains 2 workspaces".
        if is_ignored(path, root, ignore_patterns):
            continue
        try:
            content = path.read_text(encoding="utf-8")
        except OSError:
            continue
        if _WORKSPACE_MARKER_RE.search(content):
            workspace_dirs.append(path.parent)

    return workspace_dirs


def _workspace_matches(ref_workspace: str | None, cand_workspace: str, obj_workspace: str) -> bool:
    """Whether a candidate in ``cand_workspace`` satisfies a reference's WORKSPACE qualifier.

    QMD-69. ``Some(ws)`` binds the match to that workspace; ``None`` keeps it inside
    ``obj_workspace``, because a cross-workspace reference must name its workspace. An empty
    ``obj_workspace`` (single-workspace parse) imposes no constraint, and so does an empty
    ``cand_workspace`` -- that is a workspace ROOT object.

    Mirrors ``workspace_matches`` in the Rust ``core::reference_scan``.
    """
    if ref_workspace is not None:
        return cand_workspace == ref_workspace
    return not obj_workspace or not cand_workspace or cand_workspace == obj_workspace


def _qualifiers_match(
    ref_workspace: str | None,
    ref_namespace: str | None,
    cand_workspace: str,
    cand_namespace: str,
    obj_workspace: str,
) -> bool:
    """Whether a candidate satisfies a reference's qualifiers (QMD-69).

    Used by every path that resolves a reference by its ``__id``: the main candidate filter,
    the field-reference escape and the ``ambiguous_field_reference`` check.

    ``ref_namespace`` is a name for an assertion, ``""`` for the ELIDED form (``ws::id``, no
    namespace constraint), and ``None`` when the reference named none -- which imposes no
    constraint here either, the own-namespace-first preference being the caller's rule.

    The ``__local_id`` fallback deliberately does NOT use this: there an unqualified reference
    is scoped to the referring object's own namespace exactly.
    """
    if not _workspace_matches(ref_workspace, cand_workspace, obj_workspace):
        return False
    if ref_namespace is None or ref_namespace == "":
        return True
    return cand_namespace == ref_namespace


def _is_reference_finding(error_type: str) -> bool:
    """
    Whether a WorkspaceError was produced by the reference resolver.

    QMD-69: these are recomputed over the composed object set when a container holds
    several sibling workspaces, so the per-workspace copies must be discarded first.
    """
    return error_type in ("broken_link", "ambiguous_reference", "ambiguous_field_reference")


class WorkspaceUsageError(ValueError):
    """
    A workspace command was invoked in a way that cannot mean anything (QMD-72).

    Distinct from a parse or validation failure: nothing was wrong with the documents, the
    invocation itself was refused, so the CLI reports it as a usage error (exit 2) rather
    than as a result.
    """


@dataclass
class Composition:
    """What ``compose_workspace_roots`` produces (QMD-72).

    ``file_paths`` maps each ``files`` entry to its real location: ``files`` is relative to
    the result's base, which is virtual in the ``-w`` form and so cannot be joined.
    ``workspaces`` holds one entry per composed root, ordered by ``path``.
    """

    objects: list[dict[str, Any]]
    files: list[str]
    file_paths: dict[str, Path]
    errors: list[WorkspaceError]
    workspaces: list[dict[str, str]]


def compose_workspace_roots(roots: list[Path], base: Path | None) -> Composition:
    """
    Compose an explicit set of workspace ROOTS into one graph (QMD-72).

    This is the one composition primitive: the container form, the CLI's ``-w`` /
    ``--with`` form and any other surface that composes workspaces reach composition
    through it, so none of them can resolve references differently from the others. Each
    root must already be a single workspace; discovery is the caller's job.

    ``base`` decides where each workspace's files appear in ``__file``. A real directory
    that contains every root (the container form) puts them relative to it. ``None`` (the
    ``-w`` form) puts each workspace at its own id in a virtual base: its roots need not
    share any directory, so a real common ancestor can degenerate to ``/`` and would put the
    host's own directory names into ``__file``. The id depends only on the content, so the
    same repositories give the same ``__file`` wherever they are checked out, and ids are
    distinct within a composable set, so the values stay unique. Mirrors ``Mount`` in the
    Rust parser.

    Reference findings are dropped per workspace and recomputed once over the composed set
    by ``rescan_composed_references``, because a workspace validated in isolation cannot see
    its siblings' objects.
    """
    out = Composition(objects=[], files=[], file_paths={}, errors=[], workspaces=[])

    # A workspace inside another one is reported as `nested_workspace` because its files are
    # then missing from the outer workspace's graph. When the inner one is itself a member of
    # this set, nothing is missing: the outer scan already leaves its files out, and they are
    # composed under the inner workspace. The report would contradict the composition the
    # caller asked for (`-w repo -w repo/.qmdc`), so it is dropped -- matched by path, not by
    # id, since the container form does not refuse two members sharing an id (QMD-72).
    members = {Path(r).resolve() for r in roots}

    for ws_dir in roots:
        ws_result = parse_workspace(str(ws_dir))

        # Where this workspace sits in the base. None only when a container member is
        # somehow not under the container, which discovery never produces; its values are
        # then left as they came, the historical behaviour.
        prefix: str | None
        if base is None:
            prefix = ws_result.workspace_id
        else:
            try:
                prefix = ws_dir.relative_to(base).as_posix()
            except ValueError:
                prefix = None
            if prefix == ".":
                prefix = ""

        def relocate(rel: str, prefix: str | None = prefix) -> str | None:
            if prefix is None:
                return None
            return rel if prefix == "" else f"{prefix}/{rel}"

        if ws_result.workspace_id and prefix is not None:
            out.workspaces.append(
                {"id": ws_result.workspace_id, "root": _canonical_slash(ws_dir), "path": prefix}
            )

        for obj in ws_result.objects:
            if "__file" in obj:
                moved = relocate(obj["__file"])
                if moved is not None:
                    obj["__file"] = moved
        out.objects.extend(ws_result.objects)

        for file in ws_result.files:
            moved = relocate(file)
            if moved is not None:
                out.files.append(moved)
                out.file_paths[moved] = ws_dir / file

        for error in ws_result.errors:
            # QMD-69: reference findings are dropped here and recomputed once over the
            # composed object set -- in isolation this workspace could not see its
            # siblings' objects, so any cross-workspace reference looked broken.
            if _is_reference_finding(error.type):
                continue
            if (
                error.type == "nested_workspace"
                and error.file
                and (ws_dir / error.file).parent.resolve() in members
            ):
                continue
            if error.file:
                moved = relocate(error.file)
                if moved is not None:
                    error.file = moved
            out.errors.append(error)

    out.workspaces.sort(key=lambda e: (e["path"], e["id"]))
    return out


def rescan_composed_references(
    objects: list[dict[str, Any]],
    file_paths: dict[str, Path],
    errors: list[WorkspaceError],
) -> None:
    """
    Re-run reference validation over a COMPOSED object set (QMD-69).

    Each workspace was parsed and validated in isolation, so its reference findings were
    computed against an index that could not see the other workspaces' objects: a
    workspace-qualified cross-workspace reference was therefore always reported broken.
    The stale findings are dropped by ``compose_workspace_roots`` and the shared validator
    runs once here over every object in the composed set.

    ``file_paths`` maps each ``__file`` to its real location; the validator reads a
    referring object's source line through it, because under the ``-w`` form that value
    cannot be joined onto any directory (QMD-72).

    Structural findings (duplicate_id, workspace_in_wrong_file, parsing errors) stay
    per-workspace, because identity is workspace-scoped (QMD-67).
    """
    composed_index = build_index(objects)
    errors.extend(
        error
        for error in validate_workspace(objects, composed_index, file_paths=file_paths)
        if _is_reference_finding(error.type)
    )


def _resolve_single_workspace_root(path: str) -> Path:
    """
    Resolve one ``-w`` / ``--with`` path to exactly one workspace root (QMD-72).

    A ``--with`` path names a workspace, not a container: zero and several are both usage
    errors, because the caller asked to compose a specific workspace and the tool must not
    guess which one was meant.

    Raises:
        WorkspaceUsageError: with the usage message to print.
    """
    p = Path(path)
    try:
        exists = p.exists()
    except OSError:
        # Rust's `Path::exists()` reports false on ANY stat error (including EACCES on an
        # unreadable parent); Python's raises for errnos outside its ignore list. Mirror
        # Rust so an inaccessible path is the same usage error everywhere, not a crash.
        exists = False
    if not exists:
        raise WorkspaceUsageError(f"--with path does not exist: {path}")
    resolved = p.resolve()
    readme = resolved / "readme.qmd.md"
    try:
        content = readme.read_text(encoding="utf-8")
    except OSError:
        # Any OS error here means we cannot see a marker, so fall through to the scan and
        # end in the "not a workspace" usage error (exit 2) that the Rust resolver reports
        # for the same input. Letting it escape would exit 1 and break the A2 promise that
        # a rejected `-w` fails identically in all three.
        #
        # Note `readme.exists()` is NOT a safe pre-check: on Python 3.12 it ignores only
        # ENOENT/ENOTDIR/EBADF/ELOOP, so an unreadable parent directory makes the stat
        # raise PermissionError out of `exists()` itself. Measured, not assumed.
        content = ""
    if _WORKSPACE_MARKER_RE.search(content):
        return resolved

    top: list[Path] = []
    for r in find_workspace_dirs_bounded(str(resolved), WORKSPACE_SCAN_MAX_DEPTH):
        if not any(r.is_relative_to(kept) for kept in top):
            top.append(r)
    if len(top) == 1:
        return top[0]
    if not top:
        raise WorkspaceUsageError(
            f"--with path is not a workspace: {path} "
            "(no readme.qmd.md declaring [[id: __Workspace]])"
        )
    raise WorkspaceUsageError(
        f"--with path contains {len(top)} workspaces: {path} (pass each one as its own --with)"
    )


def compose_with_paths(paths: list[str]) -> WorkspaceResult:
    """
    Compose the workspaces named by repeated ``-w`` / ``--with`` (QMD-72).

    Every path is a peer -- the first is not primary -- and each must resolve to exactly
    one workspace. Raises ``WorkspaceUsageError`` rather than returning a wrong answer for the
    shapes that cannot mean anything: a path that is not a workspace, a path holding
    several, the same path twice, and two paths carrying the same workspace id (which would
    make an id ambiguous, so the set could not be composed into one graph).
    """
    roots: list[Path] = []
    for path in paths:
        root = _resolve_single_workspace_root(path)
        if root in roots:
            raise WorkspaceUsageError(f"--with path given twice: {path}")
        roots.append(root)

    composed = compose_workspace_roots(roots, None)

    # Two workspaces carrying the same id cannot be composed: every id in one would
    # collide with the other's, so no reference could resolve to a single object. With
    # each workspace at its own id, their files would also land under one __file prefix.
    seen_ids: list[str] = []
    for obj in composed.objects:
        if obj.get("__kind") == "__Workspace":
            ws_id = obj.get("__id") or ""
            if ws_id:
                if ws_id in seen_ids:
                    raise WorkspaceUsageError(
                        f"--with paths declare the same workspace id '{ws_id}'; "
                        "ids must be distinct to compose"
                    )
                seen_ids.append(ws_id)

    rescan_composed_references(composed.objects, composed.file_paths, composed.errors)

    return WorkspaceResult(
        root=None,  # The base is virtual: each workspace sits at its own id
        workspace_id=None,  # A composed set has no single workspace id
        files=composed.files,
        objects=composed.objects,
        errors=composed.errors,
        workspaces=composed.workspaces,
    )


def resolve_workspace_input(path: str | None, with_paths: list[str]) -> WorkspaceResult:
    """
    Entry point for a workspace-aware CLI command (QMD-72).

    Accepts either the historical positional path or one or more ``-w`` / ``--with``
    paths, and refuses both at once: they answer different questions ("what is near this
    path" versus "which workspaces make up this project"), so silently preferring one
    would answer a question the caller did not ask.

    Raises:
        WorkspaceUsageError: with the usage message to print.
    """
    if with_paths:
        if path is not None:
            raise WorkspaceUsageError(
                "a positional PATH and --with are mutually exclusive; "
                "pass every workspace as --with"
            )
        return compose_with_paths(with_paths)
    return resolve_workspace(path if path is not None else ".")


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

    # Root is not a workspace - find all workspaces in subdirectories. find_all_workspace_dirs
    # already drops a marker the root's .qmdcignore hides, using the same root and the same
    # rules, so there is nothing left for a second filter to remove (QMD-73).
    workspace_dirs = find_all_workspace_dirs(str(root))

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
            root=_canonical_slash(root),
            workspace_id=None,
            files=[],
            objects=[],
            errors=[],
        )

    # Parse each workspace and combine results -- through the one composition primitive.
    composed = compose_workspace_roots(workspace_dirs, root)
    all_objects = composed.objects
    all_files = composed.files
    all_file_paths = composed.file_paths
    all_errors = composed.errors

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
                all_file_paths[rel_file] = file_path
            except Exception:
                # Skip files that can't be read
                pass

    # QMD-69: re-run reference validation over the COMPOSED object set. See
    # ``rescan_composed_references`` for why the per-workspace findings cannot be reused.
    rescan_composed_references(all_objects, all_file_paths, all_errors)

    return WorkspaceResult(
        root=_canonical_slash(root),
        workspace_id=None,  # Multiple workspaces, no single ID
        files=all_files,
        objects=all_objects,
        errors=all_errors,
        workspaces=composed.workspaces,
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
