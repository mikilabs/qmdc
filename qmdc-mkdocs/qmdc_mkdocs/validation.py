"""Workspace validation — uses the qmdc Python parser library (no subprocess).

Validation goes through `qmdc.workspace.parse_workspace`, the same library the
rest of the package uses for data loading. This keeps a single source of truth
for parsing/validation and avoids the environment-dependent behavior of shelling
out to a `qmdc` binary (which may resolve to a different parser implementation).
"""

from __future__ import annotations

import sys
from dataclasses import dataclass
from pathlib import Path


@dataclass
class ValidationError:
    """A validation error formatted for display."""

    type: str
    message: str
    file: str
    line: int | None
    severity: str


def validate_workspace(workspace_or_errors) -> list[ValidationError]:
    """Validate a workspace and print a summary to stderr (non-blocking).

    Accepts either:
    - A `Path` to the workspace root — parses it via `qmdc.workspace.parse_workspace`
      and uses the resulting `.errors`.
    - A sequence of already-parsed error objects (each exposing `type`, `message`,
      `file`, `line`, `severity`) — formats them directly.

    Returns the formatted errors (empty list if none or if the workspace has no
    parseable QMD.md files).
    """
    if isinstance(workspace_or_errors, Path):
        raw_errors = _parse_workspace_errors(workspace_or_errors)
    else:
        raw_errors = list(workspace_or_errors)

    if not raw_errors:
        return []

    errors = [
        ValidationError(
            type=getattr(e, "type", "unknown"),
            message=getattr(e, "message", str(e)),
            file=getattr(e, "file", "") or "",
            line=getattr(e, "line", None),
            severity=getattr(e, "severity", "error"),
        )
        for e in raw_errors
    ]
    _print_error_summary(errors)
    return errors


def _parse_workspace_errors(workspace: Path) -> list:
    """Parse the workspace via the qmdc library and return its raw error objects.

    Returns an empty list if the workspace cannot be parsed (e.g. no QMD.md files);
    validation is non-blocking, so parse failures must not raise.
    """
    from qmdc.workspace import parse_workspace

    try:
        result = parse_workspace(str(workspace))
    except Exception:
        return []

    return list(getattr(result, "errors", []) or [])


def _print_error_summary(errors: list[ValidationError]) -> None:
    """Print formatted error summary to stderr (up to 20 errors)."""
    max_display = 20

    for err in errors[:max_display]:
        loc = f"{err.file}:{err.line}" if err.line else err.file
        print(f"  {loc}: {err.message}", file=sys.stderr)

    if len(errors) > max_display:
        remaining = len(errors) - max_display
        print(f"  ... and {remaining} more errors", file=sys.stderr)
