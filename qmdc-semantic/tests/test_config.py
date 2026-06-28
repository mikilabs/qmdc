"""Tests for config module."""

from pathlib import Path

import pytest
import yaml

from qmdc_semantic.config import (
    Config,
    load_config,
)


@pytest.mark.unit
class TestConfig:
    """Tests for configuration loading."""

    def test_default_config(self):
        """Test that default config has expected values."""
        config = Config()
        assert config.embedding.provider == "ollama"
        assert config.embedding.model == "qwen3-embedding"
        assert config.chunking.min_text_length == 30
        assert config.chunking.long_field_threshold == 50
        assert config.inferred.similarity_threshold == 0.7

    def test_load_config_no_files(self, tmp_path):
        """Test loading config when no config files exist."""
        config = load_config(tmp_path)
        # Should return defaults
        assert config.embedding.provider == "ollama"
        assert config.chunking.min_text_length == 30

    def test_load_config_workspace(self, tmp_path):
        """Test loading config from workspace."""
        # Create workspace config
        config_dir = tmp_path / ".qmdc-semantic"
        config_dir.mkdir()
        config_file = config_dir / "config.yaml"
        config_file.write_text(
            yaml.dump(
                {
                    "embedding": {
                        "provider": "openrouter",
                        "model": "openai/text-embedding-3-small",
                    },
                    "chunking": {
                        "min_text_length": 20,
                    },
                }
            )
        )

        config = load_config(tmp_path)
        assert config.embedding.provider == "openrouter"
        assert config.embedding.model == "openai/text-embedding-3-small"
        assert config.chunking.min_text_length == 20
        # Default for unchanged
        assert config.chunking.long_field_threshold == 50

    def test_config_priority(self, tmp_path, monkeypatch):
        """Test that workspace config overrides global config."""
        # Create global config
        global_dir = tmp_path / "global"
        global_dir.mkdir()
        global_config = global_dir / ".qmdc-semantic"
        global_config.mkdir()
        (global_config / "config.yaml").write_text(
            yaml.dump(
                {
                    "embedding": {
                        "provider": "ollama",
                        "model": "global-model",
                    },
                }
            )
        )

        # Create workspace config
        workspace = tmp_path / "workspace"
        workspace.mkdir()
        ws_config = workspace / ".qmdc-semantic"
        ws_config.mkdir()
        (ws_config / "config.yaml").write_text(
            yaml.dump(
                {
                    "embedding": {
                        "model": "workspace-model",
                    },
                }
            )
        )

        # Patch home directory
        monkeypatch.setattr(Path, "home", lambda: global_dir)

        config = load_config(workspace)
        # Workspace overrides global
        assert config.embedding.model == "workspace-model"
        # Provider from global (not overridden)
        assert config.embedding.provider == "ollama"
