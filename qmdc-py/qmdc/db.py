"""SQLite database module for QMDC workspace queries."""

import json
import re
import sqlite3
from dataclasses import dataclass, field
from typing import Any

# Regex for valid QMD.md field key
_FIELD_KEY_RE = re.compile(r"^[a-zA-Z_][a-zA-Z0-9_]*$")
# Regex for a reference value: [[#...]]
_REF_VALUE_RE = re.compile(r"^\[\[#[^\]]+\]\]$")
# Regex for comma-separated references: [[#a]], [[#b]]
_MULTI_REF_RE = re.compile(r"^\[\[#[^\]]+\]\](?:\s*,\s*\[\[#[^\]]+\]\])+$")


@dataclass
class QueryResult:
    """Result of a SQL query."""

    columns: list[str] = field(default_factory=list)
    rows: list[list[Any]] = field(default_factory=list)

    def to_json(self) -> list[dict[str, Any]]:
        """Convert to JSON representation (list of objects)."""
        return [dict(zip(self.columns, row, strict=False)) for row in self.rows]

    def to_table_string(self) -> str:
        """Format as text table for display (full width, no truncation)."""
        if not self.rows:
            return "(empty result)\n"

        # Calculate column widths based on actual content
        widths = [len(c) for c in self.columns]
        for row in self.rows:
            for i, val in enumerate(row):
                if i < len(widths):
                    val_str = str(val) if val is not None else "NULL"
                    # Normalize whitespace
                    val_str = " ".join(val_str.split())
                    widths[i] = max(widths[i], len(val_str))

        def format_cell(s: str, w: int) -> str:
            """Format cell content, normalizing whitespace and padding to width."""
            clean = " ".join(s.split()).strip()
            return clean.ljust(w)

        output = []

        # Header
        header = [format_cell(c, widths[i]) for i, c in enumerate(self.columns)]
        output.append(" | ".join(header))

        # Separator
        output.append("-+-".join("-" * w for w in widths))

        # Rows
        for row in self.rows:
            row_strs = [
                format_cell(
                    str(v) if v is not None else "NULL", widths[i] if i < len(widths) else 10
                )
                for i, v in enumerate(row)
            ]
            output.append(" | ".join(row_strs))

        return "\n".join(output) + "\n"


class QmdcDatabase:
    """QMDC SQLite database wrapper."""

    def __init__(self) -> None:
        """Create a new in-memory SQLite database with QMDC schema."""
        self.conn = sqlite3.connect(":memory:")
        self._create_schema()

    def _create_schema(self) -> None:
        """Create the database schema."""
        self.conn.executescript("""
            CREATE TABLE IF NOT EXISTS objects (
                __workspace TEXT NOT NULL,
                __namespace TEXT NOT NULL DEFAULT '',
                __id TEXT NOT NULL,
                __global_id TEXT GENERATED ALWAYS AS (
                    __workspace || ':' ||
                    CASE WHEN __namespace = '' THEN ':' ELSE __namespace || ':' END ||
                    __id
                ) STORED UNIQUE,
                __kind TEXT,
                __label TEXT,
                __local_id TEXT,
                __file TEXT,
                __parent TEXT,
                __line INTEGER,
                __level INTEGER,
                data TEXT,
                PRIMARY KEY (__workspace, __namespace, __id)
            );

            CREATE TABLE IF NOT EXISTS edges (
                source_id TEXT NOT NULL,
                source_field TEXT NOT NULL,
                target_id TEXT NOT NULL,
                edge_type TEXT NOT NULL,
                target_field TEXT NOT NULL DEFAULT '',
                __workspace TEXT,
                UNIQUE(source_id, source_field, target_id, edge_type, target_field),
                FOREIGN KEY (source_id) REFERENCES objects(__global_id),
                FOREIGN KEY (target_id) REFERENCES objects(__global_id)
            );

            CREATE INDEX IF NOT EXISTS idx_objects_kind ON objects(__kind);
            CREATE INDEX IF NOT EXISTS idx_objects_namespace ON objects(__namespace);
            CREATE INDEX IF NOT EXISTS idx_objects_parent ON objects(__parent);
            CREATE INDEX IF NOT EXISTS idx_objects_workspace ON objects(__workspace);
            CREATE INDEX IF NOT EXISTS idx_edges_source ON edges(source_id);
            CREATE INDEX IF NOT EXISTS idx_edges_target ON edges(target_id);
        """)

    def clear(self) -> None:
        """Clear all data."""
        self.conn.execute("DELETE FROM edges")
        self.conn.execute("DELETE FROM objects")
        self.conn.commit()

    def upsert_object(self, obj: dict[str, Any]) -> None:
        """Insert or replace an object."""
        obj_id = obj.get("__id", "")
        kind = obj.get("__kind")
        label = obj.get("__label")
        local_id = obj.get("__local_id")
        namespace = obj.get("__namespace") or ""
        workspace = obj.get("__workspace") or ""
        file = obj.get("__file")
        # Normalize __parent: extract ID from [[#id]] format
        parent = obj.get("__parent")
        if parent and parent.startswith("[[#") and parent.endswith("]]"):
            parent = parent[3:-2]
        line = obj.get("__line")
        level = obj.get("__level")

        # Build data JSON without system fields.
        # Canonical form (cross-parser byte parity): compact separators, raw
        # UTF-8 (no \uXXXX escaping), keys in document insertion order. Numeric
        # literals such as 1.0 keep their float form (`1.0`, not `1`) natively
        # because Python parses them to float; the TS parser must reconstruct
        # this from raw tokens, Rust gets it free from serde_json.
        user_data = {k: v for k, v in obj.items() if not k.startswith("__")}
        data = json.dumps(user_data, separators=(",", ":"), ensure_ascii=False)

        self.conn.execute(
            """
            INSERT OR REPLACE INTO objects
                (__workspace, __namespace, __id, __kind, __label, __local_id,
                 __file, __parent, __line, __level, data)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            """,
            (workspace, namespace, obj_id, kind, label, local_id, file, parent, line, level, data),
        )

    @staticmethod
    def compute_global_id(workspace: str, namespace: str, obj_id: str) -> str:
        """Compute __global_id from workspace, namespace, and id.

        Format: workspace:namespace:id or workspace::id (double colon for empty namespace)
        """
        if not namespace:
            return f"{workspace}::{obj_id}"
        return f"{workspace}:{namespace}:{obj_id}"

    def insert_edge(
        self,
        source_id: str,
        source_field: str,
        target_id: str,
        edge_type: str | None = None,
        target_field: str | None = None,
        workspace_id: str | None = None,
    ) -> None:
        """Insert an edge (reference).

        source_id and target_id should be __global_id values.
        edge_type defaults to source_field if not provided.
        target_field is NULL for whole-object references, non-NULL for field references.
        """
        # Extract workspace from source_id (format: workspace:namespace:id or workspace::id)
        workspace = source_id.split(":")[0] if source_id else (workspace_id or "")
        actual_edge_type = edge_type if edge_type is not None else source_field
        actual_target_field = target_field if target_field is not None else ""
        self.conn.execute(
            "INSERT OR IGNORE INTO edges "
            "(source_id, source_field, target_id, edge_type, target_field, __workspace) "
            "VALUES (?, ?, ?, ?, ?, ?)",
            (source_id, source_field, target_id, actual_edge_type, actual_target_field, workspace),
        )

    def _extract_and_insert_edges(self, source_id: str, obj: dict[str, Any]) -> None:
        """Extract references from object and insert as edges.

        source_id should be __id, not __global_id. This method computes __global_id.
        """
        workspace_id = obj.get("__workspace") or ""
        namespace = obj.get("__namespace") or ""
        source_global_id = self.compute_global_id(workspace_id, namespace, source_id)

        for field_name, value in obj.items():
            if field_name.startswith("__"):
                continue

            self._extract_refs_from_value(
                source_global_id, field_name, value, workspace_id, namespace
            )

    def _resolve_and_insert_edge(
        self,
        source_global_id: str,
        field_name: str,
        target_id: str,
        workspace_id: str,
        namespace: str,
        edge_type: str | None = None,
        target_field: str | None = None,
    ) -> None:
        """Resolve a target reference and insert an edge if the target exists.

        If the full target_id doesn't resolve as an object and contains a dot,
        tries splitting off the last segment as a target_field (field-level reference).
        """
        target_global_id = self._resolve_target_global_id(target_id, workspace_id, namespace)
        if target_global_id:
            self.insert_edge(
                source_global_id,
                field_name,
                target_global_id,
                edge_type=edge_type,
                target_field=target_field,
                workspace_id=workspace_id,
            )
        elif "." in target_id and target_field is None:
            # Try field-level resolution: split on last dot
            last_dot = target_id.rfind(".")
            obj_path = target_id[:last_dot]
            field_part = target_id[last_dot + 1 :]
            obj_global_id = self._resolve_target_global_id(obj_path, workspace_id, namespace)
            if obj_global_id:
                self.insert_edge(
                    source_global_id,
                    field_name,
                    obj_global_id,
                    edge_type=edge_type,
                    target_field=field_part,
                    workspace_id=workspace_id,
                )

    def _extract_refs_from_value(
        self, source_global_id: str, field_name: str, value: Any, workspace_id: str, namespace: str
    ) -> None:
        """Recursively extract references from a value.

        source_global_id should be __global_id, not __id.
        For text fields, also extracts typed edges from preamble lines.
        """
        if isinstance(value, str):
            # Try preamble extraction for text field values
            preamble_edges = self._extract_preamble_refs(value)
            if preamble_edges:
                # Track which targets were handled by preamble to avoid duplicates
                preamble_targets = set()
                for preamble_key, target_id in preamble_edges:
                    self._resolve_and_insert_edge(
                        source_global_id,
                        field_name,
                        target_id,
                        workspace_id,
                        namespace,
                        edge_type=preamble_key,
                    )
                    preamble_targets.add(target_id)
                # Also extract remaining refs from the rest of the text (after preamble)
                for target_id in self._parse_all_references(value):
                    if target_id not in preamble_targets:
                        self._resolve_and_insert_edge(
                            source_global_id,
                            field_name,
                            target_id,
                            workspace_id,
                            namespace,
                        )
            else:
                # No preamble — standard extraction
                for target_id in self._parse_all_references(value):
                    self._resolve_and_insert_edge(
                        source_global_id,
                        field_name,
                        target_id,
                        workspace_id,
                        namespace,
                    )
        elif isinstance(value, list):
            for item in value:
                self._extract_refs_from_value(
                    source_global_id, field_name, item, workspace_id, namespace
                )

    @staticmethod
    def _extract_preamble_refs(text: str) -> list[tuple[str, str]] | None:
        """Extract typed edges from text field preamble.

        A preamble is a markdown list at the start of a text field where ALL items
        are valid `- key: [[#ref]]` fields. If any item is invalid, returns None
        (all-or-nothing).

        The preamble must be separated from the rest of the text by a blank line.

        Returns list of (key, target_id) tuples, or None if no valid preamble.
        """
        if not text or not text.startswith("- "):
            return None

        # Split into preamble block (up to first blank line) and rest
        # A blank line is \n\n (two consecutive newlines)
        parts = text.split("\n\n", 1)
        preamble_block = parts[0]

        lines = preamble_block.split("\n")
        edges: list[tuple[str, str]] = []

        for line in lines:
            line = line.strip()
            if not line:
                continue
            if not line.startswith("- "):
                return None  # Not a list item — invalid preamble

            content = line[2:].strip()
            # Must be "key: value" format
            colon_idx = content.find(":")
            if colon_idx <= 0:
                return None

            key = content[:colon_idx].strip()
            val = content[colon_idx + 1 :].strip()

            # Key must be a valid field key
            if not _FIELD_KEY_RE.match(key):
                return None

            # Value must be a reference or comma-separated references
            if _REF_VALUE_RE.match(val) or _MULTI_REF_RE.match(val):
                ref_ids = re.findall(r"\[\[#([^\]]+)\]\]", val)
                for ref_id in ref_ids:
                    target = ref_id.split(":")[-1]
                    edges.append((key, target))
            else:
                return None  # Value is not a reference — invalid preamble

        return edges if edges else None

    def _resolve_target_global_id(
        self, target_id: str, workspace_id: str, namespace: str
    ) -> str | None:
        """Resolve target __global_id from target __id.

        First tries same workspace/namespace, then searches all workspaces.
        """
        # First try: same workspace and namespace
        candidate = self.compute_global_id(workspace_id, namespace, target_id)
        cursor = self.conn.execute(
            "SELECT 1 FROM objects WHERE __global_id = ? LIMIT 1", (candidate,)
        )
        if cursor.fetchone():
            return candidate

        # Second try: same workspace, any namespace (including empty)
        cursor = self.conn.execute(
            "SELECT __global_id FROM objects WHERE __workspace = ? AND __id = ? LIMIT 1",
            (workspace_id, target_id),
        )
        row = cursor.fetchone()
        if row:
            return row[0]

        # Third try: any workspace
        cursor = self.conn.execute(
            "SELECT __global_id FROM objects WHERE __id = ? LIMIT 1", (target_id,)
        )
        row = cursor.fetchone()
        if row:
            return row[0]

        # Fourth try: __local_id in same namespace
        cursor = self.conn.execute(
            "SELECT __global_id FROM objects"
            " WHERE __local_id = ? AND __workspace = ? AND __namespace = ? LIMIT 2",
            (target_id, workspace_id, namespace),
        )
        rows = cursor.fetchall()
        if len(rows) == 1:
            return rows[0][0]
        # If 0 or >1 matches, return None (ambiguous or not found)

        return None

    def _parse_reference(self, s: str) -> str | None:
        """Parse [[#id]] or [[#namespace:id]] reference, return target id."""
        if s.startswith("[[#") and s.endswith("]]"):
            inner = s[3:-2]
            # Take last part after : as the id
            parts = inner.split(":")
            return parts[-1] if parts else None
        return None

    def _parse_all_references(self, s: str) -> list[str]:
        """Find ALL [[#id]] patterns in the string, return list of target ids."""
        targets = []
        # Match [[#...]] patterns
        pattern = r"\[\[#([^\]]+)\]\]"
        for match in re.finditer(pattern, s):
            inner = match.group(1)
            # Take last part after : as the id
            parts = inner.split(":")
            target_id = parts[-1] if parts else None
            if target_id:
                targets.append(target_id)
        return targets

    def sync_objects(self, objects: list[dict[str, Any]]) -> None:
        """Sync objects from workspace.

        Two passes: first insert all objects, then extract and insert edges.
        This ensures all referenced objects exist before edges are created.
        """
        self.clear()

        # First pass: insert all objects
        for obj in objects:
            self.upsert_object(obj)

        # Second pass: extract and insert edges
        for obj in objects:
            obj_id = obj.get("__id")
            if obj_id:
                self._extract_and_insert_edges(obj_id, obj)

        self.conn.commit()

    def query(self, sql: str) -> QueryResult:
        """Execute a SQL query."""
        sql = sql.strip()

        # Handle dot-commands
        if sql.startswith("."):
            return self._handle_dot_command(sql)

        cursor = self.conn.execute(sql)
        columns = [desc[0] for desc in cursor.description] if cursor.description else []
        rows = [list(row) for row in cursor.fetchall()]
        return QueryResult(columns=columns, rows=rows)

    def _handle_dot_command(self, cmd: str) -> QueryResult:
        """Handle dot-commands (.schema, .tables, etc.)."""
        parts = cmd.split()
        command = parts[0] if parts else ""

        if command == ".tables":
            return self.query("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
        elif command == ".schema":
            table = parts[1] if len(parts) > 1 else None
            if table:
                # Use parameterized query to prevent SQL injection
                cursor = self.conn.execute(
                    "SELECT sql FROM sqlite_master WHERE type='table' AND name=?",
                    (table,),
                )
                columns = [desc[0] for desc in cursor.description] if cursor.description else []
                rows = [list(row) for row in cursor.fetchall()]
                return QueryResult(columns=columns, rows=rows)
            return self.query(
                "SELECT name, sql FROM sqlite_master WHERE type='table' ORDER BY name"
            )
        elif command == ".help":
            return QueryResult(
                columns=["command", "description"],
                rows=[
                    [".tables", "List all tables"],
                    [".schema [table]", "Show table schema"],
                    [".help", "Show this help"],
                ],
            )
        else:
            raise ValueError(f"Unknown command: {command}. Try .help")

    def close(self) -> None:
        """Close the database connection."""
        self.conn.close()


def execute_query(workspace: dict[str, Any], query: str) -> QueryResult:
    """Execute a query against a workspace result.

    The query can be:
    - A SQL query (e.g., "SELECT * FROM objects")
    - A reference to a Query object (e.g., "#get_tables")

    Args:
        workspace: WorkspaceResult dict with 'objects' list
        query: SQL query or "#query_id"

    Returns:
        QueryResult with columns and rows
    """
    db = QmdcDatabase()

    try:
        # Sync objects
        objects = workspace.get("objects", [])
        db.sync_objects(objects)

        # Resolve query
        if query.startswith("#"):
            # Find Query object by ID
            query_id = query[1:]
            query_obj = next(
                (
                    obj
                    for obj in objects
                    if obj.get("__id") == query_id and obj.get("__kind") == "Query"
                ),
                None,
            )
            if not query_obj:
                raise ValueError(f"Query object '{query_id}' not found")
            sql = query_obj.get("sql")
            if not isinstance(sql, str):
                raise ValueError(f"Query object '{query_id}' has no 'sql' field")
        else:
            sql = query

        return db.query(sql)
    finally:
        db.close()
