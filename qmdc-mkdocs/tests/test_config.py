"""Unit tests for qmdc_mkdocs/config.py."""

from pathlib import Path

import yaml

from qmdc_mkdocs.config import (
    generate_build_config,
    generate_mkdocs_config,
    generate_nav_file,
)

SAMPLE_NAV = [
    {"Home": "readme.md"},
    {"Storage Layer": [{"Users": "storage/tables.md"}]},
]


class TestGenerateMkdocsConfig:
    """Tests for generate_mkdocs_config."""

    def test_creates_file_when_absent(self, tmp_path: Path):
        """generate_mkdocs_config creates mkdocs.yml when it doesn't exist."""
        generate_mkdocs_config(tmp_path, nav=SAMPLE_NAV, site_name="Test Site")

        mkdocs_yml = tmp_path / "mkdocs.yml"
        assert mkdocs_yml.exists()

    def test_does_not_overwrite_existing_file(self, tmp_path: Path):
        """generate_mkdocs_config does NOT overwrite an existing mkdocs.yml."""
        mkdocs_yml = tmp_path / "mkdocs.yml"
        original_content = "site_name: User Config\n"
        mkdocs_yml.write_text(original_content)

        generate_mkdocs_config(tmp_path, nav=SAMPLE_NAV, site_name="New Name")

        assert mkdocs_yml.read_text() == original_content

    def test_generated_yaml_contains_required_keys(self, tmp_path: Path):
        """Generated YAML contains site_name, theme.name, and plugins."""
        generate_mkdocs_config(tmp_path, nav=SAMPLE_NAV, site_name="My Docs")

        config = yaml.safe_load((tmp_path / "mkdocs.yml").read_text())

        assert config["site_name"] == "My Docs"
        assert config["theme"]["name"] == "material"
        assert "qmdc" in config["plugins"]

    def test_generated_yaml_does_not_contain_build_time_keys(self, tmp_path: Path):
        """Generated YAML does NOT contain docs_dir or custom_dir (build-time only)."""
        generate_mkdocs_config(tmp_path, nav=SAMPLE_NAV, site_name="Test")

        config = yaml.safe_load((tmp_path / "mkdocs.yml").read_text())

        assert "docs_dir" not in config
        assert "custom_dir" not in config
        # custom_dir lives inside theme, check there too
        assert "custom_dir" not in config.get("theme", {})

    def test_generated_yaml_includes_nav(self, tmp_path: Path):
        """Generated YAML includes the provided nav structure."""
        generate_mkdocs_config(tmp_path, nav=SAMPLE_NAV, site_name="Test")

        config = yaml.safe_load((tmp_path / "mkdocs.yml").read_text())

        assert config["nav"] == SAMPLE_NAV


class TestGenerateBuildConfig:
    """Tests for generate_build_config."""

    def test_merges_user_config_with_tmpdir_paths(self, tmp_path: Path):
        """generate_build_config reads user config and merges build-time paths."""
        workspace = tmp_path / "workspace"
        workspace.mkdir()
        tmpdir = tmp_path / "build_tmp"
        tmpdir.mkdir()
        output = tmp_path / "output"

        # Write a user config with custom settings
        user_config = {"site_name": "User Site", "theme": {"name": "material"}}
        (workspace / "mkdocs.yml").write_text(yaml.dump(user_config))

        result_path = generate_build_config(workspace, tmpdir, output)

        build_config = yaml.safe_load(result_path.read_text())

        # User settings preserved
        assert build_config["site_name"] == "User Site"
        # Build-time paths injected
        assert "docs_dir" in build_config
        assert "site_dir" in build_config

    def test_sets_docs_dir_site_dir_custom_dir_correctly(self, tmp_path: Path):
        """generate_build_config sets docs_dir, site_dir, custom_dir to correct paths."""
        workspace = tmp_path / "workspace"
        workspace.mkdir()
        tmpdir = tmp_path / "build_tmp"
        tmpdir.mkdir()
        output = tmp_path / "output"

        (workspace / "mkdocs.yml").write_text(yaml.dump({"site_name": "Test"}))

        result_path = generate_build_config(workspace, tmpdir, output)
        build_config = yaml.safe_load(result_path.read_text())

        assert build_config["docs_dir"] == str(tmpdir / "docs")
        assert build_config["site_dir"] == str(output)
        assert build_config["theme"]["custom_dir"] == str(tmpdir / "overrides")

    def test_returns_path_to_temp_config(self, tmp_path: Path):
        """generate_build_config returns path to the generated temp config file."""
        workspace = tmp_path / "workspace"
        workspace.mkdir()
        tmpdir = tmp_path / "build_tmp"
        tmpdir.mkdir()
        output = tmp_path / "output"

        (workspace / "mkdocs.yml").write_text(yaml.dump({"site_name": "Test"}))

        result_path = generate_build_config(workspace, tmpdir, output)

        assert result_path == tmpdir / "mkdocs.yml"
        assert result_path.exists()

    def test_works_without_existing_user_config(self, tmp_path: Path):
        """generate_build_config works even if no user mkdocs.yml exists."""
        workspace = tmp_path / "workspace"
        workspace.mkdir()
        tmpdir = tmp_path / "build_tmp"
        tmpdir.mkdir()
        output = tmp_path / "output"

        result_path = generate_build_config(workspace, tmpdir, output)
        build_config = yaml.safe_load(result_path.read_text())

        assert build_config["docs_dir"] == str(tmpdir / "docs")
        assert build_config["site_dir"] == str(output)


class TestGenerateNavFile:
    """Tests for generate_nav_file."""

    def test_writes_valid_yaml(self, tmp_path: Path):
        """generate_nav_file writes a valid YAML file to workspace root."""
        generate_nav_file(tmp_path, nav=SAMPLE_NAV)

        nav_yml = tmp_path / "nav.yml"
        assert nav_yml.exists()

        # Parse it back — should not raise
        parsed = yaml.safe_load(nav_yml.read_text())
        assert parsed == SAMPLE_NAV

    def test_overwrites_existing_nav_file(self, tmp_path: Path):
        """generate_nav_file overwrites existing nav.yml (it's a reference file)."""
        nav_yml = tmp_path / "nav.yml"
        nav_yml.write_text("old content\n")

        new_nav = [{"Updated": "new.md"}]
        generate_nav_file(tmp_path, nav=new_nav)

        parsed = yaml.safe_load(nav_yml.read_text())
        assert parsed == new_nav
