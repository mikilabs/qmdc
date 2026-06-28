"""Property-based tests for SQL block table rendering.

**Validates: Requirements 3.4**

Property 4: SQL Block to Markdown Table
For any valid SQL query that returns N rows with M columns, `_render_markdown_table`
produces a table with exactly N+2 lines (header + separator + N data rows).
Each row has exactly M pipe-delimited cells. The header row contains the column names.
The separator row contains `---` for each column. Empty result sets produce `*No results*`.
"""

from __future__ import annotations

from hypothesis import given, settings, assume
from hypothesis import strategies as st

from qmdc_mkdocs.sql_blocks import _render_markdown_table, process_sql_blocks


# --- Strategies ---

# Generate column names: non-empty strings without pipe characters or newlines
column_name_st = st.text(
    alphabet=st.characters(
        whitelist_categories=("L", "N", "Pd"),
        whitelist_characters="_",
    ),
    min_size=1,
    max_size=20,
).filter(lambda s: "|" not in s and "\n" not in s)

# Generate cell values: strings without pipe characters or newlines
cell_value_st = st.text(
    alphabet=st.characters(
        whitelist_categories=("L", "N", "P", "S", "Z"),
        blacklist_characters="|\n\r",
    ),
    min_size=0,
    max_size=50,
)


@st.composite
def rows_strategy(draw):
    """Generate a list of row dicts with consistent column names and string values."""
    num_cols = draw(st.integers(min_value=1, max_value=8))
    num_rows = draw(st.integers(min_value=1, max_value=20))

    columns = draw(
        st.lists(column_name_st, min_size=num_cols, max_size=num_cols, unique=True)
    )

    rows = []
    for _ in range(num_rows):
        row = {}
        for col in columns:
            row[col] = draw(cell_value_st)
        rows.append(row)

    return rows


# --- Property Tests ---


class TestSqlBlockToMarkdownTable:
    """Property 4: SQL Block to Markdown Table.

    **Validates: Requirements 3.4**
    """

    @given(rows=rows_strategy())
    @settings(max_examples=200)
    def test_table_has_correct_line_count(self, rows):
        """For N rows with M columns, the rendered table has exactly N+2 lines
        (header + separator + N data rows).

        **Validates: Requirements 3.4**
        """
        result = _render_markdown_table(rows)
        lines = result.split("\n")

        expected_lines = len(rows) + 2  # header + separator + data rows
        assert len(lines) == expected_lines, (
            f"Expected {expected_lines} lines for {len(rows)} rows, got {len(lines)}"
        )

    @given(rows=rows_strategy())
    @settings(max_examples=200)
    def test_each_row_has_correct_cell_count(self, rows):
        """Each row (header, separator, data) has exactly M pipe-delimited cells.

        **Validates: Requirements 3.4**
        """
        result = _render_markdown_table(rows)
        lines = result.split("\n")
        num_cols = len(rows[0].keys())

        for i, line in enumerate(lines):
            # Each line format: "| cell1 | cell2 | ... | cellM |"
            # Splitting by "|" gives: ["", " cell1 ", " cell2 ", ..., " cellM ", ""]
            # So the number of cells = number of "|" separators - 1
            # Or: strip leading/trailing "|", split by "|" → M cells
            assert line.startswith("|"), f"Line {i} doesn't start with '|': {line}"
            assert line.endswith("|"), f"Line {i} doesn't end with '|': {line}"

            inner = line[1:-1]  # strip outer pipes
            cells = inner.split("|")
            assert len(cells) == num_cols, (
                f"Line {i} has {len(cells)} cells, expected {num_cols}. Line: {line}"
            )

    @given(rows=rows_strategy())
    @settings(max_examples=200)
    def test_header_contains_column_names(self, rows):
        """The header row contains the column names from the input dicts.

        **Validates: Requirements 3.4**
        """
        result = _render_markdown_table(rows)
        header_line = result.split("\n")[0]
        columns = list(rows[0].keys())

        # Extract cells from header
        inner = header_line[1:-1]
        header_cells = [c.strip() for c in inner.split("|")]

        assert header_cells == columns, (
            f"Header cells {header_cells} don't match columns {columns}"
        )

    @given(rows=rows_strategy())
    @settings(max_examples=200)
    def test_separator_contains_dashes(self, rows):
        """The separator row (line 2) contains `---` for each column.

        **Validates: Requirements 3.4**
        """
        result = _render_markdown_table(rows)
        separator_line = result.split("\n")[1]
        num_cols = len(rows[0].keys())

        inner = separator_line[1:-1]
        sep_cells = [c.strip() for c in inner.split("|")]

        assert len(sep_cells) == num_cols
        for cell in sep_cells:
            assert cell == "---", (
                f"Separator cell should be '---', got '{cell}'"
            )

    def test_empty_rows_in_process_sql_blocks(self, mock_workspace_db):
        """Empty result sets from SQL queries produce `*No results*`.

        **Validates: Requirements 3.4**
        """
        # Use a query that returns no results
        content = "```table\nsql: SELECT * FROM objects WHERE __id = 'nonexistent_xyz'\n```"
        result = process_sql_blocks(content, mock_workspace_db, "readme.qmd.md")
        assert result == "*No results*"

    def test_render_markdown_table_empty_list(self):
        """_render_markdown_table with empty list returns empty string.

        **Validates: Requirements 3.4**
        """
        result = _render_markdown_table([])
        assert result == ""
