"""Test CLI commands via subprocess (real usage)."""

import json
import subprocess
import tempfile
from pathlib import Path

MICROTESTS_DIR = Path(__file__).parent.parent.parent / "tests/parser"
CLI_DIR = Path(__file__).parent.parent


def test_cli_parse_stdin():
    """Test CLI with stdin input."""
    result = subprocess.run(
        ["qmdc", "parse"], input="## Test [[test]]", capture_output=True, text=True, cwd=CLI_DIR
    )

    assert result.returncode == 0, f"stderr: {result.stderr}"
    output = json.loads(result.stdout)
    assert isinstance(output, list), "Output must be an array"
    assert len(output) == 1, "Must be exactly one object"
    assert output[0]["__id"] == "test"


def test_cli_parse_file():
    """Test CLI with file input."""
    qmdc_file = MICROTESTS_DIR / "001-empty-object.qmd.md"

    result = subprocess.run(
        ["qmdc", "parse", "-i", str(qmdc_file)], capture_output=True, text=True, cwd=CLI_DIR
    )

    assert result.returncode == 0, f"stderr: {result.stderr}"
    output = json.loads(result.stdout)
    assert isinstance(output, list), "Output must be an array"
    # 001-empty-object.qmd.md now returns Document + TextBlock
    assert len(output) == 2, "Must be two objects (Document + TextBlock)"
    assert output[0]["__kind"] == "__Document"
    assert output[1]["__kind"] == "__TextBlock"


def test_cli_parse_output_file():
    """Test CLI with file output."""
    with tempfile.TemporaryDirectory() as tmpdir:
        output_file = Path(tmpdir) / "output.json"

        result = subprocess.run(
            ["qmdc", "parse", "-o", str(output_file)],
            input="## Test [[test]]",
            capture_output=True,
            text=True,
            cwd=CLI_DIR,
        )

        assert result.returncode == 0, f"stderr: {result.stderr}"
        assert output_file.exists()

        output = json.loads(output_file.read_text())
        assert isinstance(output, list), "Output must be an array"
        assert len(output) == 1, "Must be exactly one object"
        assert output[0]["__id"] == "test"


def test_cli_all_microtests():
    """Test CLI on first 5 microtests."""
    for i in range(1, 6):
        qmdc_files = list(MICROTESTS_DIR.glob(f"{i:03d}-*.qmd.md"))
        assert len(qmdc_files) == 1, f"Should have exactly one file for test {i:03d}"

        qmdc_file = qmdc_files[0]
        expected_file = qmdc_file.with_suffix("").with_suffix(".expected.json")

        result = subprocess.run(
            ["qmdc", "parse", "-i", str(qmdc_file)], capture_output=True, text=True, cwd=CLI_DIR
        )

        assert result.returncode == 0, f"Test {i:03d} failed: {result.stderr}"

        output = json.loads(result.stdout)
        expected = json.loads(expected_file.read_text())

        assert output == expected, f"Test {i:03d} output mismatch"


def test_cli_rebuild_stdin():
    """Test rebuild command with stdin input - verify round-trip."""
    result = subprocess.run(
        ["qmdc", "rebuild"],
        input='[{"__id": "test", "__label": "Test"}]',
        capture_output=True,
        text=True,
        cwd=CLI_DIR,
    )

    assert result.returncode == 0, f"rebuild failed! stderr: {result.stderr}"
    assert "# Test [[test]]" in result.stdout, "rebuild should generate QMD output"


def test_cli_parse_no_comments():
    """Test CLI --no-comments option."""
    result = subprocess.run(
        ["qmdc", "parse", "--no-comments"],
        input="## Test [[test]]\n\n- name: Alice\n\nThis is a comment.",
        capture_output=True,
        text=True,
        cwd=CLI_DIR,
    )

    assert result.returncode == 0, f"stderr: {result.stderr}"
    output = json.loads(result.stdout)
    for obj in output:
        assert "__comments" not in obj, f"Object {obj.get('__id')} should not have __comments"


def test_cli_parse_no_pretty():
    """Test CLI --no-pretty option (compact JSON)."""
    result = subprocess.run(
        ["qmdc", "parse", "--no-pretty"],
        input="## Test [[test]]",
        capture_output=True,
        text=True,
        cwd=CLI_DIR,
    )

    assert result.returncode == 0, f"stderr: {result.stderr}"
    # Should be valid JSON
    output = json.loads(result.stdout)
    assert isinstance(output, list)
    assert len(output) == 1
    assert output[0]["__id"] == "test"


def test_cli_workspace_parse_spaced_kind():
    """CLI must detect a workspace whose __Workspace kind has a space after the colon.

    `[[id: __Workspace]]` (spaced) is valid QMD and renders identically to the
    unspaced form, so `workspace parse` must find the root and exit 0.
    """
    with tempfile.TemporaryDirectory() as tmpdir:
        ws = Path(tmpdir)
        (ws / "readme.qmd.md").write_text(
            "# Spaced Project [[spaced_proj: __Workspace]]\n\n"
            "- description: workspace with a space after the colon\n\n"
            "## Thing [[thing]]\n\n"
            "- value: 1\n"
        )

        result = subprocess.run(
            ["qmdc", "workspace", "parse", str(ws)],
            capture_output=True,
            text=True,
            cwd=CLI_DIR,
        )

        assert result.returncode == 0, (
            f"spaced __Workspace should be detected, got exit {result.returncode}: {result.stderr}"
        )
        output = json.loads(result.stdout)
        assert output["workspace"] == "spaced_proj"


if __name__ == "__main__":
    import pytest

    pytest.main([__file__, "-v"])
