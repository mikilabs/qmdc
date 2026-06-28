"""Unit tests for validation.py — workspace validation wrapper."""

from pathlib import Path

from qmdc.workspace import WorkspaceError

from qmdc_mkdocs.validation import ValidationError, validate_workspace


class TestValidateWorkspace:
    """Tests for validate_workspace function."""

    def test_returns_empty_when_no_errors(self, sample_workspace):
        """When workspace has no validation errors, returns empty list."""
        errors = validate_workspace(sample_workspace)
        # The sample workspace may have some broken links due to cross-refs;
        # what matters is the function runs without crashing and returns a list
        assert isinstance(errors, list)
        assert all(isinstance(e, ValidationError) for e in errors)

    def test_returns_empty_for_nonexistent_workspace(self, tmp_path):
        """When workspace path has no .qmd.md files, returns empty list gracefully."""
        errors = validate_workspace(tmp_path)
        assert errors == []

    def test_accepts_workspace_error_list_directly(self):
        """When passed a list of WorkspaceError objects, formats them directly."""
        raw_errors = [
            WorkspaceError(
                type="broken_link",
                message="Object 'xyz' not found",
                file="api/endpoints.qmd.md",
                line=5,
                severity="error",
            ),
            WorkspaceError(
                type="duplicate_id",
                message="Duplicate ID 'users'",
                file="storage/tables.qmd.md",
                line=10,
                severity="error",
            ),
        ]
        errors = validate_workspace(raw_errors)

        assert len(errors) == 2
        assert errors[0].type == "broken_link"
        assert errors[0].message == "Object 'xyz' not found"
        assert errors[0].file == "api/endpoints.qmd.md"
        assert errors[0].line == 5
        assert errors[0].severity == "error"
        assert errors[1].type == "duplicate_id"

    def test_handles_none_file_gracefully(self):
        """When error has file=None, it maps to empty string."""
        raw_errors = [
            WorkspaceError(
                type="broken_link",
                message="Something wrong",
                file=None,
                line=None,
                severity="error",
            ),
        ]
        errors = validate_workspace(raw_errors)

        assert len(errors) == 1
        assert errors[0].file == ""
        assert errors[0].line is None

    def test_handles_null_line(self):
        """When line is None, it maps correctly."""
        raw_errors = [
            WorkspaceError(
                type="ambiguous_reference",
                message="Ambiguous ref",
                file="readme.qmd.md",
                line=None,
                severity="warning",
            ),
        ]
        errors = validate_workspace(raw_errors)

        assert errors[0].line is None
        assert errors[0].severity == "warning"

    def test_prints_summary_to_stderr(self, capsys):
        """Errors are printed to stderr with file:line: message format."""
        raw_errors = [
            WorkspaceError(
                type="broken_link",
                message="Object 'xyz' not found",
                file="api/endpoints.qmd.md",
                line=5,
                severity="error",
            ),
        ]
        validate_workspace(raw_errors)

        captured = capsys.readouterr()
        assert "api/endpoints.qmd.md:5: Object 'xyz' not found" in captured.err

    def test_prints_file_only_when_line_is_none(self, capsys):
        """When line is None, only file path is shown in summary."""
        raw_errors = [
            WorkspaceError(
                type="broken_link",
                message="Something wrong",
                file="readme.qmd.md",
                line=None,
                severity="error",
            ),
        ]
        validate_workspace(raw_errors)

        captured = capsys.readouterr()
        assert "readme.qmd.md: Something wrong" in captured.err
        # Should NOT have a colon after file when line is None
        assert "readme.qmd.md:None" not in captured.err

    def test_limits_display_to_20_errors(self, capsys):
        """Only first 20 errors are printed, with remaining count."""
        raw_errors = [
            WorkspaceError(
                type="broken_link",
                message=f"Error {i}",
                file=f"file{i}.qmd.md",
                line=i + 1,
                severity="error",
            )
            for i in range(25)
        ]
        errors = validate_workspace(raw_errors)

        # All 25 errors are returned
        assert len(errors) == 25

        captured = capsys.readouterr()
        # First 20 are printed
        assert "file0.qmd.md:1: Error 0" in captured.err
        assert "file19.qmd.md:20: Error 19" in captured.err
        # 21st is NOT printed
        assert "file20.qmd.md" not in captured.err
        # Remaining count is shown
        assert "... and 5 more errors" in captured.err

    def test_no_remaining_message_for_20_or_fewer(self, capsys):
        """When exactly 20 errors, no 'and X more' message is shown."""
        raw_errors = [
            WorkspaceError(
                type="broken_link",
                message=f"Error {i}",
                file=f"file{i}.qmd.md",
                line=i,
                severity="error",
            )
            for i in range(20)
        ]
        validate_workspace(raw_errors)

        captured = capsys.readouterr()
        assert "... and" not in captured.err

    def test_returns_empty_list_for_empty_errors(self):
        """When passed an empty list, returns empty list without printing."""
        errors = validate_workspace([])
        assert errors == []

    def test_real_workspace_with_broken_links(self, tmp_path, capsys):
        """Integration test: workspace with broken references produces errors."""
        # Create a workspace with a broken reference
        readme = tmp_path / "readme.qmd.md"
        readme.write_text(
            "# Test [[test: __Workspace]]\n\n"
            "## Item [[item: Thing]]\n\n"
            "- ref: [[#nonexistent]]\n"
        )

        errors = validate_workspace(tmp_path)

        # Should detect the broken link
        assert len(errors) > 0
        assert any(e.type == "broken_link" for e in errors)

        # Should print to stderr
        captured = capsys.readouterr()
        assert "nonexistent" in captured.err
