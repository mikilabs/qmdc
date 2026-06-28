"""Workspace DB loading and sqlite3 management.

Uses the qmdc Python parser as a library — no subprocess calls.
"""

from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

from qmdc.db import QmdcDatabase
from qmdc.workspace import WorkspaceResult, parse_workspace


class WorkspaceLoadError(Exception):
    """Raised when a QMDC workspace cannot be loaded (missing path, parse failure,
    or no QMD.md files). Callers (e.g. the CLI) decide how to surface it; library
    consumers can catch it instead of being killed by a SystemExit."""


@dataclass
class WorkspaceData:
    """Holds both structured parser output and SQLite DB for queries."""

    result: WorkspaceResult
    db: QmdcDatabase
    objects_by_file: dict[str, list[dict[str, Any]]] = field(default_factory=dict)

    def query(self, sql: str, params: tuple = ()) -> list[dict]:
        """Execute SQL query and return list of row dicts.

        When params are provided, uses parameterized execution directly
        on the connection (QmdcDatabase.query() doesn't support params).
        """
        if params:
            cursor = self.db.conn.execute(sql, params)
            columns = [desc[0] for desc in cursor.description] if cursor.description else []
            rows = cursor.fetchall()
            return [dict(zip(columns, row, strict=True)) for row in rows]
        qr = self.db.query(sql)
        return [dict(zip(qr.columns, row, strict=True)) for row in qr.rows]

    def close(self) -> None:
        """Close the underlying database."""
        self.db.close()


def load_workspace(workspace: Path) -> WorkspaceData:
    """Parse workspace using qmdc library and load into SQLite.

    This gives us:
    - Structured objects with __references (positions), __line, __positions
    - SQLite DB with objects + edges tables for SQL queries

    Args:
        workspace: Path to the QMDC workspace root directory.

    Returns:
        WorkspaceData with parsed result, DB, and objects grouped by file.

    Raises:
        WorkspaceLoadError: If the path is missing, parsing fails, or there are
            no QMD.md files. The CLI converts this to a clean error exit; library
            callers can catch it.
    """
    if not workspace.is_dir():
        raise WorkspaceLoadError(f"Workspace path does not exist: {workspace}")

    try:
        ws_result = parse_workspace(str(workspace))
    except Exception as exc:
        raise WorkspaceLoadError(f"Failed to parse workspace '{workspace}': {exc}") from exc

    if not ws_result.files:
        raise WorkspaceLoadError(f"No .qmd.md files found in workspace '{workspace}'")

    # Load into SQLite (creates objects + edges tables, resolves references)
    db = QmdcDatabase()
    db.sync_objects(ws_result.objects)

    # Group objects by file for per-file processing
    objects_by_file: dict[str, list[dict[str, Any]]] = {}
    for obj in ws_result.objects:
        f = obj.get("__file", "")
        if f:
            objects_by_file.setdefault(f, []).append(obj)

    return WorkspaceData(
        result=ws_result,
        db=db,
        objects_by_file=objects_by_file,
    )
