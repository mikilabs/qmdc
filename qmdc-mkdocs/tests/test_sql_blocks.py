"""Unit tests for qmdc_mkdocs.sql_blocks module."""

import json
import sqlite3

import pytest

from qmdc_mkdocs.sql_blocks import (
    _apply_workspace_scope,
    _parse_block_content,
    _render_error,
    _render_markdown_table,
    _resolve_sql,
    process_sql_blocks,
)


# --- Fixture: Mock WorkspaceData with a Query object ---


@pytest.fixture
def ws_data_with_query():
    """Create a mock WorkspaceData that includes a Query object with sql in data JSON."""
    conn = sqlite3.connect(":memory:")
    conn.execute("PRAGMA journal_mode = OFF")

    # Objects table — Query objects store their SQL in the JSON `data` column
    conn.execute("""
        CREATE TABLE objects (
            "__global_id" TEXT,
            "__id" TEXT,
            "__kind" TEXT,
            "__label" TEXT,
            "__file" TEXT,
            "__line" TEXT,
            "__level" TEXT,
            "__workspace" TEXT,
            "__namespace" TEXT,
            "data" TEXT
        )
    """)
    conn.executemany(
        'INSERT INTO objects VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)',
        [
            ("myproject", "myproject", "__Workspace", "My Project",
             "readme.qmd.md", "1", "1", None, None, "{}"),
            ("get_tables", "get_tables", "Query", "Get Tables",
             "queries.qmd.md", "3", "2", "myproject", None,
             json.dumps({"sql": "SELECT __id, __label FROM objects WHERE __kind = 'Table'"})),
            ("storage::users", "users", "Table", "Users",
             "storage/tables.qmd.md", "3", "2", "myproject", "storage", "{}"),
            ("storage::orders", "orders", "Table", "Orders",
             "storage/tables.qmd.md", "10", "2", "myproject", "storage", "{}"),
        ],
    )

    # Edges table (empty is fine for these tests)
    conn.execute("""
        CREATE TABLE edges (
            "source_id" TEXT,
            "source_field" TEXT,
            "target_id" TEXT,
            "edge_type" TEXT,
            "__workspace" TEXT
        )
    """)

    conn.executescript("""
        CREATE INDEX idx_obj_id ON objects(__id);
        CREATE INDEX idx_obj_kind ON objects(__kind);
    """)

    class MockWorkspaceData:
        def __init__(self, connection):
            self.conn = connection

        def query(self, sql, params=()):
            cur = self.conn.execute(sql, params)
            cols = [d[0] for d in cur.description]
            return [dict(zip(cols, row)) for row in cur.fetchall()]

        def close(self):
            self.conn.close()

    ws = MockWorkspaceData(conn)
    yield ws
    ws.close()


# --- Tests for _parse_block_content (YAML-first parsing) ---


class TestParseBlockContent:
    """Tests for _parse_block_content with YAML and fallback parsing."""

    def test_simple_sql_yaml(self):
        """Parses a simple sql: line as YAML."""
        body = "sql: SELECT __id FROM objects"
        result = _parse_block_content(body)
        assert result["sql"] == "SELECT __id FROM objects"

    def test_multiple_keys_yaml(self):
        """Parses multiple YAML keys."""
        body = "sql: SELECT * FROM objects\nscope: workspace"
        result = _parse_block_content(body)
        assert result["sql"] == "SELECT * FROM objects"
        assert result["scope"] == "workspace"

    def test_query_reference_yaml(self):
        """Parses a query: [[#id]] reference via YAML."""
        body = "query: '[[#get_tables]]'\nscope: all"
        result = _parse_block_content(body)
        assert result["query"] == "[[#get_tables]]"
        assert result["scope"] == "all"

    def test_multiline_sql_yaml_pipe(self):
        """Parses multiline sql: | YAML syntax."""
        body = "sql: |\n  SELECT __id, __kind\n  FROM objects\n  WHERE __kind = 'Table'\nscope: all"
        result = _parse_block_content(body)
        assert "SELECT __id, __kind" in result["sql"]
        assert "FROM objects" in result["sql"]
        assert "WHERE __kind = 'Table'" in result["sql"]
        assert result["scope"] == "all"

    def test_empty_body(self):
        """Returns empty dict for empty body."""
        result = _parse_block_content("")
        assert result == {} or result is None

    def test_fallback_to_regex_on_invalid_yaml(self):
        """Falls back to line parsing when YAML is invalid."""
        # This is invalid YAML (unquoted colon in value) but parseable line-by-line
        body = "sql: SELECT * FROM objects WHERE __id = 'ns:id'"
        result = _parse_block_content(body)
        # YAML may parse this fine or fall back — either way sql should be present
        assert "sql" in result

    def test_whitespace_handling(self):
        """YAML handles whitespace naturally."""
        body = "sql: SELECT 1\nscope: all"
        result = _parse_block_content(body)
        assert result["sql"] == "SELECT 1"
        assert result["scope"] == "all"

    def test_query_without_quotes(self):
        """Query reference without quotes — YAML may parse differently."""
        body = "query: get_tables\nscope: workspace"
        result = _parse_block_content(body)
        assert result["query"] == "get_tables"
        assert result["scope"] == "workspace"


# --- Tests for _resolve_sql ---


class TestResolveSql:
    """Tests for _resolve_sql with inline SQL and Query object reference."""

    def test_inline_sql(self, ws_data_with_query):
        """Returns inline sql value directly."""
        params = {"sql": "SELECT __id FROM objects"}
        result = _resolve_sql(params, ws_data_with_query)
        assert result == "SELECT __id FROM objects"

    def test_query_reference_from_data_json(self, ws_data_with_query):
        """Looks up Query object and returns sql from data JSON column."""
        params = {"query": "[[#get_tables]]"}
        result = _resolve_sql(params, ws_data_with_query)
        assert result == "SELECT __id, __label FROM objects WHERE __kind = 'Table'"

    def test_query_reference_bare_id(self, ws_data_with_query):
        """Handles bare ID without [[ ]] markers."""
        params = {"query": "get_tables"}
        result = _resolve_sql(params, ws_data_with_query)
        assert result == "SELECT __id, __label FROM objects WHERE __kind = 'Table'"

    def test_no_sql_or_query(self, ws_data_with_query):
        """Returns None when neither sql nor query is specified."""
        params = {"scope": "workspace"}
        result = _resolve_sql(params, ws_data_with_query)
        assert result is None

    def test_query_not_found(self, ws_data_with_query):
        """Returns None when referenced Query object doesn't exist."""
        params = {"query": "[[#nonexistent_query]]"}
        result = _resolve_sql(params, ws_data_with_query)
        assert result is None

    def test_sql_takes_precedence_over_query(self, ws_data_with_query):
        """When both sql and query are present, sql is used."""
        params = {"sql": "SELECT 1", "query": "[[#get_tables]]"}
        result = _resolve_sql(params, ws_data_with_query)
        assert result == "SELECT 1"

    def test_multiline_sql_stripped(self, ws_data_with_query):
        """Multiline SQL from YAML pipe is stripped of trailing whitespace."""
        params = {"sql": "SELECT __id\nFROM objects\n"}
        result = _resolve_sql(params, ws_data_with_query)
        assert result == "SELECT __id\nFROM objects"


# --- Tests for _render_markdown_table ---


class TestRenderMarkdownTable:
    """Tests for _render_markdown_table with various row counts."""

    def test_empty_rows(self):
        """Returns empty string for empty row list."""
        result = _render_markdown_table([])
        assert result == ""

    def test_single_row(self):
        """Renders a table with one data row."""
        rows = [{"name": "Alice", "age": "30"}]
        result = _render_markdown_table(rows)
        lines = result.split("\n")
        assert len(lines) == 3  # header + separator + 1 row
        assert lines[0] == "| name | age |"
        assert lines[1] == "| --- | --- |"
        assert lines[2] == "| Alice | 30 |"

    def test_multiple_rows(self):
        """Renders a table with multiple data rows."""
        rows = [
            {"id": "1", "name": "Alice"},
            {"id": "2", "name": "Bob"},
            {"id": "3", "name": "Charlie"},
        ]
        result = _render_markdown_table(rows)
        lines = result.split("\n")
        assert len(lines) == 5  # header + separator + 3 rows
        assert "| 1 | Alice |" in lines
        assert "| 2 | Bob |" in lines
        assert "| 3 | Charlie |" in lines

    def test_none_values_rendered_as_empty(self):
        """None values in rows are rendered as empty strings."""
        rows = [{"id": "1", "name": None}]
        result = _render_markdown_table(rows)
        assert "| 1 |  |" in result

    def test_column_order_preserved(self):
        """Column order matches the order of keys in the first row dict."""
        rows = [{"z_col": "z", "a_col": "a", "m_col": "m"}]
        result = _render_markdown_table(rows)
        header = result.split("\n")[0]
        assert header == "| z_col | a_col | m_col |"


# --- Tests for _render_error ---


class TestRenderError:
    """Tests for error rendering."""

    def test_renders_error_div(self):
        """Renders an error message in a styled div."""
        result = _render_error("Something went wrong")
        assert '<div class="sql-error">' in result
        assert "Something went wrong" in result
        assert "⚠️" in result

    def test_error_message_preserved(self):
        """The exact error message is included in the output."""
        msg = "SQL error: no such table: objects"
        result = _render_error(msg)
        assert msg in result


# --- Tests for _apply_workspace_scope ---


class TestApplyWorkspaceScope:
    """Tests for workspace scope filtering."""

    def test_wraps_sql_with_workspace_filter(self, ws_data_with_query):
        """Wraps SQL with workspace filter when workspace exists."""
        sql = "SELECT __id FROM objects"
        result = _apply_workspace_scope(sql, ws_data_with_query)
        assert "SELECT * FROM (" in result
        assert "__workspace = 'myproject'" in result
        assert "__workspace IS NULL" in result


# --- Tests for process_sql_blocks (integration) ---


class TestProcessSqlBlocks:
    """Integration tests for process_sql_blocks with a mock WorkspaceData."""

    def test_inline_sql_renders_table(self, ws_data_with_query):
        """A ```table block with inline SQL renders a Markdown table."""
        content = "```table\nsql: SELECT __id, __kind FROM objects WHERE __kind = 'Table'\nscope: all\n```"
        result = process_sql_blocks(content, ws_data_with_query, "test.qmd.md")
        assert "| __id | __kind |" in result
        assert "| users | Table |" in result
        assert "| orders | Table |" in result

    def test_no_results_renders_message(self, ws_data_with_query):
        """A query returning no rows renders '*No results*'."""
        content = "```table\nsql: SELECT __id FROM objects WHERE __kind = 'NonExistent'\nscope: all\n```"
        result = process_sql_blocks(content, ws_data_with_query, "test.qmd.md")
        assert result == "*No results*"

    def test_invalid_sql_renders_error(self, ws_data_with_query):
        """Invalid SQL renders an error message."""
        content = "```table\nsql: INVALID SQL STATEMENT\nscope: all\n```"
        result = process_sql_blocks(content, ws_data_with_query, "test.qmd.md")
        assert "sql-error" in result
        assert "SQL error" in result

    def test_missing_sql_and_query_renders_error(self, ws_data_with_query):
        """Block with neither sql nor query renders an error."""
        content = "```table\nscope: workspace\n```"
        result = process_sql_blocks(content, ws_data_with_query, "test.qmd.md")
        assert "sql-error" in result
        assert "No" in result and "sql" in result and "query" in result and "specified" in result

    def test_query_reference_resolves(self, ws_data_with_query):
        """A ```table block with query: [[#id]] resolves and executes."""
        content = "```table\nquery: '[[#get_tables]]'\nscope: all\n```"
        result = process_sql_blocks(content, ws_data_with_query, "test.qmd.md")
        assert "| __id | __label |" in result
        assert "| users | Users |" in result
        assert "| orders | Orders |" in result

    def test_surrounding_content_preserved(self, ws_data_with_query):
        """Content before and after the table block is preserved."""
        content = "# Title\n\nSome text.\n\n```table\nsql: SELECT __id FROM objects WHERE __kind = 'Table'\nscope: all\n```\n\nMore text."
        result = process_sql_blocks(content, ws_data_with_query, "test.qmd.md")
        assert result.startswith("# Title\n\nSome text.\n\n")
        assert result.endswith("\n\nMore text.")

    def test_multiple_blocks_processed(self, ws_data_with_query):
        """Multiple ```table blocks in one file are all processed."""
        content = (
            "# First\n\n"
            "```table\nsql: SELECT __id FROM objects WHERE __kind = 'Table'\nscope: all\n```\n\n"
            "# Second\n\n"
            "```table\nsql: SELECT __id FROM objects WHERE __kind = '__Workspace'\nscope: all\n```"
        )
        result = process_sql_blocks(content, ws_data_with_query, "test.qmd.md")
        assert "# First" in result
        assert "# Second" in result
        assert "| users |" in result
        assert "| myproject |" in result

    def test_reverse_order_preserves_indices(self, ws_data_with_query):
        """Processing in reverse order keeps earlier content positions valid."""
        # Two blocks with different-length outputs
        content = (
            "Before first.\n\n"
            "```table\nsql: SELECT __id FROM objects WHERE __kind = '__Workspace'\nscope: all\n```\n\n"
            "Between.\n\n"
            "```table\nsql: SELECT __id FROM objects WHERE __kind = 'Table'\nscope: all\n```\n\n"
            "After last."
        )
        result = process_sql_blocks(content, ws_data_with_query, "test.qmd.md")
        assert "Before first." in result
        assert "Between." in result
        assert "After last." in result
        # Both tables should be rendered
        assert "| myproject |" in result
        assert "| users |" in result

    def test_multiline_sql_yaml_pipe(self, ws_data_with_query):
        """Multiline sql: | YAML syntax is parsed correctly."""
        content = "```table\nsql: |\n  SELECT __id, __kind\n  FROM objects\n  WHERE __kind = 'Table'\nscope: all\n```"
        result = process_sql_blocks(content, ws_data_with_query, "test.qmd.md")
        assert "| __id | __kind |" in result
        assert "| users | Table |" in result

    def test_no_table_blocks_returns_unchanged(self, ws_data_with_query):
        """Content without ```table blocks is returned unchanged."""
        content = "# Hello\n\nSome regular content.\n\n```python\nprint('hi')\n```"
        result = process_sql_blocks(content, ws_data_with_query, "test.qmd.md")
        assert result == content
