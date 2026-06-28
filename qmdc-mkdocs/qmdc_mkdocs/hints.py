"""Semantic hints loading and injection from hints.json."""

from __future__ import annotations

import json
from dataclasses import dataclass
from pathlib import Path
from typing import TYPE_CHECKING, Any

if TYPE_CHECKING:
    pass


@dataclass
class HintEntry:
    """A single semantic hint entry linking to a similar object."""

    label: str
    kind: str | None
    file: str
    score: float
    id: str | None = None  # Target object's local __id (for anchor navigation)


def load_hints(workspace: Path) -> dict[str, list[HintEntry]]:
    """Load hints.json from .qmdc-semantic/ directory. Returns empty dict if absent."""
    hints_path = workspace / ".qmdc-semantic" / "hints.json"
    if not hints_path.exists():
        return {}
    raw = json.loads(hints_path.read_text())
    return {
        key: [HintEntry(**h) for h in entries]
        for key, entries in raw.items()
    }


def get_page_hints(
    source_file: str,
    ws_data: Any,
    all_hints: dict[str, list[HintEntry]],
) -> dict[str, list[HintEntry]]:
    """Get hints relevant to objects on a specific page.

    Matches hints by:
    - Object global_id (e.g. "workspace:namespace:id") → keyed by local __id
    - Field-level keys (e.g. "field:field_id@file.qmd.md") → keyed by field_id

    Args:
        source_file: Workspace-relative path of the current page.
        ws_data: WorkspaceData instance with .query(sql) method.
        all_hints: All loaded hints from load_hints().

    Returns:
        Dict mapping object/field IDs to their hint entries for this page.
    """
    if not all_hints:
        return {}

    # Get objects on this page (exclude system types).
    objects = ws_data.query(
        "SELECT __id, __global_id FROM objects "
        "WHERE __file = ? AND __kind NOT GLOB '__*'",
        params=(source_file,),
    )

    page_hints: dict[str, list[HintEntry]] = {}
    for obj in objects:
        gid = obj["__global_id"]
        if gid in all_hints:
            page_hints[obj["__id"]] = all_hints[gid]

    # Field-level hints for this file
    for key, entries in all_hints.items():
        if key.startswith("field:") and key.endswith(f"@{source_file}"):
            field_id = key.removeprefix("field:").removesuffix(f"@{source_file}")
            page_hints[field_id] = entries

    return page_hints
