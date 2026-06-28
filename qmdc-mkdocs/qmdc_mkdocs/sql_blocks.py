"""Dynamic SQL block execution and Markdown table rendering.

Executes ```table fenced code blocks against the workspace SQLite DB.
Uses regex to find code fences (stable markdown syntax) and YAML to parse
block content (matching the VS Code extension's parseBlockContent approach).
"""

from __future__ import annotations

import json
import re
from typing import TYPE_CHECKING

import yaml

if TYPE_CHECKING:
    from .database import WorkspaceData

# Regex for ```table fenced code blocks (stable markdown syntax)
TABLE_BLOCK_PATTERN = re.compile(r"```table\n([\s\S]*?)```", re.MULTILINE)


def process_sql_blocks(content: str, ws_data: WorkspaceData, source_file: str) -> str:
    """Replace ```table blocks with rendered Markdown tables.

    Algorithm (matches VS Code extension):
    1. Find all ```table blocks via regex
    2. Process matches in reverse order (so earlier indices stay valid)
    3. Parse block body as YAML (supports multiline sql: | syntax)
    4. If query: present — look up Query object's sql field in DB
    5. If sql: present — use inline SQL directly
    6. If scope: workspace (default) — filter by current workspace
    7. Execute SQL, render as Markdown table or error div

    Args:
        content: Full file content as a string.
        ws_data: WorkspaceData with DB for SQL execution.
        source_file: Workspace-relative path of the source file.

    Returns:
        Content with ```table blocks replaced by rendered tables or errors.
    """
    matches = list(TABLE_BLOCK_PATTERN.finditer(content))
    if not matches:
        return content

    # Process in reverse order so earlier indices stay valid
    result = content
    for match in reversed(matches):
        block_body = match.group(1).strip()
        parsed = _parse_block_content(block_body)

        sql = _resolve_sql(parsed, ws_data)
        if sql is None:
            replacement = _render_error("No 'sql' or 'query' specified in table block")
        else:
            # Apply workspace scope filtering (default)
            scope = parsed.get("scope", "workspace")
            if scope == "workspace":
                sql = _apply_workspace_scope(sql, ws_data)

            try:
                rows = ws_data.query(sql)
            except Exception as e:
                replacement = _render_error(f"SQL error: {e}")
            else:
                replacement = "*No results*" if not rows else _render_markdown_table(rows)

        # Replace the match in the content using indices
        start = match.start()
        end = match.end()
        result = result[:start] + replacement + result[end:]

    return result


def _parse_block_content(body: str) -> dict:
    """Parse block body as YAML, with regex fallback.

    Matches the VS Code extension's parseBlockContent approach:
    1. Try YAML parsing first (supports multiline sql: | syntax)
    2. Fall back to line-by-line regex parsing if YAML fails

    Args:
        body: The content between ```table and ``` markers.

    Returns:
        Dict with parsed keys (sql, query, scope).
    """
    # Try YAML parsing first
    try:
        parsed = yaml.safe_load(body)
        if parsed and isinstance(parsed, dict):
            return parsed
    except yaml.YAMLError:
        pass

    # Fallback: line-by-line regex parsing
    params: dict[str, str] = {}
    for line in body.splitlines():
        if ":" in line:
            key, _, value = line.partition(":")
            params[key.strip()] = value.strip()
    return params


def _resolve_sql(params: dict, ws_data: WorkspaceData) -> str | None:
    """Get SQL from inline 'sql' param or by looking up a Query object.

    Args:
        params: Parsed block parameters.
        ws_data: WorkspaceData for Query object lookup.

    Returns:
        SQL string or None if neither sql nor query is specified.
    """
    if "sql" in params:
        sql = params["sql"]
        return sql.strip() if isinstance(sql, str) else str(sql)

    if "query" in params:
        ref = params["query"]
        if not isinstance(ref, str):
            return None

        # Strip [[ # and ]] markers to get the object ID
        obj_id = ref.strip("[]#").strip()

        # Look up Query object's sql field via the data column
        rows = ws_data.query(
            "SELECT data FROM objects WHERE __id = ? AND __kind = 'Query'",
            params=(obj_id,),
        )
        if rows:
            data_str = rows[0].get("data")
            if data_str:
                try:
                    data = json.loads(data_str) if isinstance(data_str, str) else data_str
                    if isinstance(data, dict) and "sql" in data:
                        return data["sql"]
                except (json.JSONDecodeError, TypeError):
                    pass

        # Fallback: try the sql column directly (older schema where sql is a top-level column)
        try:
            rows = ws_data.query(
                "SELECT sql FROM objects WHERE __id = ? AND __kind = 'Query'",
                params=(obj_id,),
            )
            if rows and rows[0].get("sql"):
                return rows[0]["sql"]
        except Exception:
            pass

    return None


def _apply_workspace_scope(sql: str, ws_data: WorkspaceData) -> str:
    """Wrap SQL to filter by current workspace when scope is 'workspace'.

    Wraps the query as ``SELECT * FROM (<sql>) WHERE __workspace = '<id>' OR
    __workspace IS NULL``.

    Limitation: this subquery wrapping assumes ``<sql>`` is a plain ``SELECT``
    whose result set exposes a ``__workspace`` column. It does not support
    leading CTEs (``WITH ...``), top-level ``UNION``, or projections that drop
    ``__workspace``; for those, author the block with ``scope: all`` and filter
    explicitly inside the query.

    Args:
        sql: The original SQL query.
        ws_data: WorkspaceData for workspace ID lookup.

    Returns:
        SQL wrapped with workspace filter, or original if no workspace found.
    """
    ws_rows = ws_data.query(
        "SELECT __id FROM objects WHERE __kind = '__Workspace' LIMIT 1"
    )
    if ws_rows:
        ws_ref = ws_rows[0]["__id"]
        # Use parameterized-safe approach: since we can't pass params through
        # the wrapping SQL (it's composed dynamically), we validate the workspace
        # ID contains only safe characters (alphanumeric, underscore, hyphen).
        if not all(c.isalnum() or c in "_-" for c in ws_ref):
            return sql  # Unsafe workspace ID, skip filtering
        return (
            f"SELECT * FROM ({sql}) "
            f"WHERE __workspace = '{ws_ref}' OR __workspace IS NULL"
        )
    return sql


def _render_markdown_table(rows: list[dict]) -> str:
    """Render query results as a pipe-delimited Markdown table.

    Args:
        rows: List of row dicts from SQL query.

    Returns:
        Markdown table string with header, separator, and data rows.
    """
    if not rows:
        return ""
    cols = list(rows[0].keys())
    header = "| " + " | ".join(cols) + " |"
    separator = "| " + " | ".join("---" for _ in cols) + " |"
    body_lines = []
    for row in rows:
        cells = [str(row.get(c, "") or "") for c in cols]
        body_lines.append("| " + " | ".join(cells) + " |")
    return "\n".join([header, separator, *body_lines])


def _render_error(message: str) -> str:
    """Render an error message as a styled div.

    Args:
        message: Error description to display.

    Returns:
        HTML div with error styling.
    """
    from html import escape as html_escape

    return f'\n<div class="sql-error">⚠️ {html_escape(message)}</div>\n'
