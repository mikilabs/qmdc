"""Content regeneration via Kiro CLI agent.

Finds ContentGenerator objects in the workspace, checks if sources changed,
and invokes kiro-cli to regenerate content for stale targets.
"""

from __future__ import annotations

import hashlib
import json
import subprocess
from pathlib import Path
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from .database import WorkspaceData


def find_generators(ws_data: WorkspaceData) -> list[dict]:
    """Find all ContentGenerator objects in the workspace."""
    rows = ws_data.query(
        "SELECT __id, __file, __parent, data FROM objects WHERE __kind = 'ContentGenerator'"
    )
    generators = []
    for row in rows:
        data = json.loads(row["data"]) if isinstance(row["data"], str) else row["data"]
        generators.append({
            "id": row["__id"],
            "file": row["__file"],
            "parent": row["__parent"],
            "target": data.get("target", ""),
            "about": data.get("about", []),
            "sources_hash": data.get("sources_hash", ""),
            "data": data,
        })
    return generators


def compute_sources_hash(generator: dict, ws_data: WorkspaceData, workspace: Path) -> str:
    """Compute SHA-256 hash of all source files referenced by about: links."""
    about_refs = generator["about"]
    if isinstance(about_refs, str):
        about_refs = [about_refs]

    # Collect unique source files from about: references
    source_files: set[str] = set()
    for ref in about_refs:
        # Strip [[# and ]] to get object id
        obj_id = ref.strip("[]#").strip()
        rows = ws_data.query(
            "SELECT __file FROM objects WHERE __id = ?", params=(obj_id,)
        )
        for r in rows:
            if r["__file"]:
                source_files.add(r["__file"])

    # Hash the contents of all source files
    hasher = hashlib.sha256()
    for sf in sorted(source_files):
        file_path = workspace / sf
        if file_path.exists():
            hasher.update(file_path.read_bytes())

    return hasher.hexdigest()[:16]


def parse_target(target: str) -> tuple[str, str]:
    """Parse target reference like '[[#quickstart.content]]' into (object_id, field_name)."""
    # Strip [[ # and ]]
    inner = target.strip("[]#").strip()
    if "." in inner:
        parts = inner.rsplit(".", 1)
        return parts[0], parts[1]
    return inner, "content"


def regenerate_file(
    file_path: str,
    workspace: Path,
    ws_data: WorkspaceData,
    dry_run: bool = False,
    force: bool = False,
) -> dict:
    """Regenerate content for a single file containing ContentGenerator objects.

    Returns dict with status info.
    """
    # Find generators in this file
    generators = [
        g for g in find_generators(ws_data) if g["file"] == file_path
    ]

    if not generators:
        return {"file": file_path, "status": "no_generators", "regenerated": False}

    for gen in generators:
        # Check if sources changed
        current_hash = compute_sources_hash(gen, ws_data, workspace)
        stored_hash = gen["sources_hash"]

        if not force and stored_hash == current_hash and stored_hash != "pending":
            return {
                "file": file_path,
                "status": "unchanged",
                "regenerated": False,
                "hash": current_hash,
            }

        if dry_run:
            return {
                "file": file_path,
                "status": "would_regenerate",
                "regenerated": False,
                "old_hash": stored_hash,
                "new_hash": current_hash,
            }

        # Build short prompt for kiro-cli (agent has full instructions in its system prompt)
        target_obj, target_field = parse_target(gen["target"])
        short_prompt = f"Regenerate: {file_path} hash:{current_hash}"

        try:
            # Invoke kiro-cli with the prompt — stream output to terminal.
            #
            # SECURITY NOTE: `--trust-all-tools` runs the content-generator agent
            # with ALL tools auto-approved (file writes, shell, etc.), driven by
            # prompt text that originates from workspace documents. Only run
            # `regenerate` on workspaces you trust. This is a build-time authoring
            # tool, not part of `build`/`serve`, so it never runs during a normal
            # site build.
            result = subprocess.run(
                [
                    "kiro-cli", "chat",
                    "--agent", "content-generator",
                    "--no-interactive",
                    "--trust-all-tools",
                    short_prompt,
                ],
                text=True,
                cwd=str(workspace),
                timeout=180,
            )

            if result.returncode != 0:
                return {
                    "file": file_path,
                    "status": "error",
                    "regenerated": False,
                    "error": f"kiro-cli exited with code {result.returncode}",
                }

            # Agent writes directly to the file — we just verify it worked
            # Re-read the file to check if content was updated
            abs_file = workspace / file_path
            updated_text = abs_file.read_text(encoding="utf-8")
            if current_hash in updated_text:
                return {
                    "file": file_path,
                    "status": "regenerated",
                    "regenerated": True,
                    "hash": current_hash,
                }
            else:
                return {
                    "file": file_path,
                    "status": "regenerated",
                    "regenerated": True,
                    "hash": current_hash,
                    "note": "hash not found in file — agent may not have updated sources_hash",
                }

        except FileNotFoundError:
            return {
                "file": file_path,
                "status": "error",
                "regenerated": False,
                "error": "kiro-cli not found. Install Kiro CLI to use content regeneration.",
            }
        except subprocess.TimeoutExpired:
            return {
                "file": file_path,
                "status": "error",
                "regenerated": False,
                "error": "kiro-cli timed out after 180s",
            }

    return {"file": file_path, "status": "no_action", "regenerated": False}
