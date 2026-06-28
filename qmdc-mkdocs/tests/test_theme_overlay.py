"""Tests for the `.mkdocs_theme/` user theme overlay (safe precedence)."""

from __future__ import annotations

from pathlib import Path

from qmdc_mkdocs.cli import _scaffold_overrides


def test_user_theme_assets_are_added(tmp_path: Path):
    """Files in `.mkdocs_theme/` that don't collide with plugin files are shipped."""
    ws = tmp_path / "ws"
    (ws / ".mkdocs_theme" / "css").mkdir(parents=True)
    (ws / ".mkdocs_theme" / "css" / "brand.css").write_text("body{color:#FE4810}", encoding="utf-8")

    tmpdir = tmp_path / "build"
    tmpdir.mkdir()
    _scaffold_overrides(tmpdir, ws)

    brand = tmpdir / "overrides" / "css" / "brand.css"
    assert brand.exists()
    assert brand.read_text(encoding="utf-8") == "body{color:#FE4810}"
    # Plugin files still scaffolded alongside the user additions.
    assert (tmpdir / "overrides" / "main.html").exists()


def test_plugin_wins_over_user_override(tmp_path: Path):
    """A user file that collides with a plugin file is overwritten by the plugin."""
    ws = tmp_path / "ws"
    (ws / ".mkdocs_theme").mkdir(parents=True)
    (ws / ".mkdocs_theme" / "main.html").write_text("HIJACKED", encoding="utf-8")

    tmpdir = tmp_path / "build"
    tmpdir.mkdir()
    _scaffold_overrides(tmpdir, ws)

    # The plugin's main.html must take precedence — QMDC features cannot be disabled.
    assert (tmpdir / "overrides" / "main.html").read_text(encoding="utf-8") != "HIJACKED"


def test_no_user_theme_is_fine(tmp_path: Path):
    """A workspace without `.mkdocs_theme/` still scaffolds plugin overrides."""
    ws = tmp_path / "ws"
    ws.mkdir()

    tmpdir = tmp_path / "build"
    tmpdir.mkdir()
    _scaffold_overrides(tmpdir, ws)

    assert (tmpdir / "overrides" / "main.html").exists()


def test_workspace_arg_optional(tmp_path: Path):
    """Back-compat: _scaffold_overrides works without a workspace argument."""
    tmpdir = tmp_path / "build"
    tmpdir.mkdir()
    _scaffold_overrides(tmpdir)

    assert (tmpdir / "overrides").is_dir()
    assert (tmpdir / "overrides" / "main.html").exists()
