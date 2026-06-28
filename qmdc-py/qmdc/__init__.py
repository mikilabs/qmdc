"""QMDC Parser - Convert QMD.md to structured JSON."""

__version__ = "1.0.3"

from .parser import parse, rebuild
from .workspace import (
    WorkspaceError,
    WorkspaceResult,
    build_index,
    find_workspace_root,
    parse_all_workspaces,
    parse_workspace,
    resolve_reference,
    resolve_workspace,
    scan_workspace,
    validate_workspace,
    workspace_to_json,
)

__all__ = [
    "parse",
    "rebuild",
    # Workspace
    "WorkspaceError",
    "WorkspaceResult",
    "find_workspace_root",
    "scan_workspace",
    "parse_workspace",
    "parse_all_workspaces",
    "resolve_workspace",
    "build_index",
    "resolve_reference",
    "validate_workspace",
    "workspace_to_json",
]
