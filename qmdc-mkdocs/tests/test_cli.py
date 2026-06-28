"""Smoke tests for the CLI entry point."""

from __future__ import annotations

from pathlib import Path

from click.testing import CliRunner

from qmdc_mkdocs.cli import cli


def test_cli_help():
    """CLI --help works and lists all subcommands."""
    runner = CliRunner()
    result = runner.invoke(cli, ["--help"])
    assert result.exit_code == 0
    assert "init" in result.output
    assert "build" in result.output
    assert "serve" in result.output


def test_cli_no_subcommand():
    """CLI without subcommand shows usage (Click returns exit code 2 for missing subcommand)."""
    runner = CliRunner()
    result = runner.invoke(cli, [])
    # Click groups exit with code 2 when no subcommand is given
    assert result.exit_code == 2
    assert "Usage" in result.output


def test_init_creates_mkdocs_yml(sample_workspace):
    """init command creates mkdocs.yml when absent."""
    runner = CliRunner()
    result = runner.invoke(cli, ["--workspace", str(sample_workspace), "init"])
    assert result.exit_code == 0, f"Failed: {result.output}"
    assert "Init complete" in result.output

    mkdocs_yml = sample_workspace / "mkdocs.yml"
    assert mkdocs_yml.exists()

    nav_yml = sample_workspace / "nav.yml"
    assert nav_yml.exists()


def test_init_does_not_overwrite_existing_mkdocs_yml(sample_workspace):
    """init command does NOT overwrite existing mkdocs.yml."""
    mkdocs_yml = sample_workspace / "mkdocs.yml"
    mkdocs_yml.write_text("site_name: Custom\n")

    runner = CliRunner()
    result = runner.invoke(cli, ["--workspace", str(sample_workspace), "init"])
    assert result.exit_code == 0

    # Should still have the custom content
    assert "Custom" in mkdocs_yml.read_text()


def test_cli_workspace_option_nonexistent():
    """CLI with nonexistent workspace path fails gracefully."""
    runner = CliRunner()
    result = runner.invoke(cli, ["--workspace", "/nonexistent/path", "init"])
    assert result.exit_code != 0


def test_no_qmdc_binary_check(sample_workspace, monkeypatch):
    """CLI does NOT check for qmdc binary — all data from qmd Python library."""
    import shutil as _shutil

    # Patch shutil.which to always return None (simulating no qmdc in PATH)
    monkeypatch.setattr(_shutil, "which", lambda name: None if name == "qmdc" else _shutil.which(name))

    runner = CliRunner()
    result = runner.invoke(cli, ["--workspace", str(sample_workspace), "init"])
    # Should succeed even without qmdc binary
    assert result.exit_code == 0, f"Failed: {result.output}"
    assert "Init complete" in result.output


def _make_minimal_ws(tmp_path: Path) -> Path:
    """A minimal valid workspace whose build emits MkDocs's own INFO output.

    MkDocs always logs build lines (e.g. "Building documentation",
    "Documentation built") to stderr; `build` must surface them rather than
    swallow them on success (the regression `test_build_surfaces_mkdocs_output`
    guards against).
    """
    ws = tmp_path / "ws"
    ws.mkdir()
    (ws / "readme.qmd.md").write_text(
        "# Build Output WS [[bows: __Workspace]]\n\n"
        "- version: 1.0\n\n"
        "## Overview [[overview: Section]]\n\n"
        "Body text.\n",
        encoding="utf-8",
    )
    return ws


def test_build_surfaces_mkdocs_output(tmp_path: Path):
    """`build` must stream MkDocs's own output, not swallow it on success.

    Previously build ran mkdocs with capture_output=True and only printed on
    failure, so INFO/WARNING diagnostics (e.g. unrecognised plugin options,
    broken links) were invisible — unlike `serve`, which streams them.

    Run the CLI as a real OS subprocess (not Click's CliRunner) so the mkdocs
    child process's stderr is captured the same way a user's terminal sees it.
    """
    import subprocess
    import sys

    ws = _make_minimal_ws(tmp_path)
    out = tmp_path / "site"

    proc = subprocess.run(
        [
            sys.executable,
            "-m",
            "qmdc_mkdocs.cli",
            "--workspace",
            str(ws),
            "--output",
            str(out),
            "build",
        ],
        capture_output=True,
        text=True,
    )
    combined = proc.stdout + proc.stderr
    assert proc.returncode == 0, f"build failed: {combined}"

    # MkDocs always logs these during a build; build must surface them.
    assert "Building documentation" in combined or "Documentation built" in combined

