"""Configuration loading for QMDC Semantic."""

from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

import yaml


@dataclass
class EmbeddingConfig:
    """Embedding provider configuration."""

    provider: str = "ollama"
    model: str = "qwen3-embedding"
    base_url: str = "http://localhost:11434"
    api_key_env: str | None = None
    dimension: int | None = None  # Auto-detected if not specified


@dataclass
class ChunkingConfig:
    """Chunking configuration."""

    min_text_length: int = 30
    long_field_threshold: int = 50
    max_chunk_size: int = 3000  # Split child chunks exceeding this char count


@dataclass
class InferredConfig:
    """Inferred edges configuration."""

    similarity_threshold: float = 0.7
    top_k: int = 50


@dataclass
class SearchConfig:
    """Search configuration."""

    # Chunk types priority for snippet display (first = best)
    snippet_priority: list[str] = field(
        default_factory=lambda: ["solution", "description", "text", "summary", "body", "combined"]
    )
    snippet_max_length: int = 120


@dataclass
class Config:
    """Main configuration."""

    embedding: EmbeddingConfig = field(default_factory=EmbeddingConfig)
    chunking: ChunkingConfig = field(default_factory=ChunkingConfig)
    inferred: InferredConfig = field(default_factory=InferredConfig)
    search: SearchConfig = field(default_factory=SearchConfig)


def _dict_to_config(data: dict[str, Any]) -> Config:
    """Convert dict to Config dataclass."""
    config = Config()

    if "embedding" in data:
        emb = data["embedding"]
        config.embedding = EmbeddingConfig(
            provider=emb.get("provider", "ollama"),
            model=emb.get("model", "qwen3-embedding"),
            base_url=emb.get("base_url", "http://localhost:11434"),
            api_key_env=emb.get("api_key_env"),
            dimension=emb.get("dimension"),
        )

    if "chunking" in data:
        ch = data["chunking"]
        config.chunking = ChunkingConfig(
            min_text_length=ch.get("min_text_length", 30),
            long_field_threshold=ch.get("long_field_threshold", 50),
            max_chunk_size=ch.get("max_chunk_size", 3000),
        )

    if "inferred" in data:
        inf = data["inferred"]
        config.inferred = InferredConfig(
            similarity_threshold=inf.get("similarity_threshold", 0.7),
            top_k=inf.get("top_k", 50),
        )

    return config


def load_config(workspace_path: Path | str | None = None) -> Config:
    """Load configuration with priority: workspace > global > defaults.

    Args:
        workspace_path: Path to workspace directory. If provided, looks for
            .qmdc-semantic/config.yaml in that directory first.

    Returns:
        Config object with merged settings.
    """
    config_data: dict[str, Any] = {}

    # 1. Global config (~/.qmdc-semantic/config.yaml)
    global_config = Path.home() / ".qmdc-semantic" / "config.yaml"
    if global_config.exists():
        with open(global_config) as f:
            global_data = yaml.safe_load(f) or {}
            config_data.update(global_data)

    # 2. Workspace config (.qmdc-semantic/config.yaml)
    if workspace_path:
        workspace_path = Path(workspace_path)
        workspace_config = workspace_path / ".qmdc-semantic" / "config.yaml"
        if workspace_config.exists():
            with open(workspace_config) as f:
                workspace_data = yaml.safe_load(f) or {}
                # Deep merge embedding section
                if "embedding" in workspace_data:
                    if "embedding" not in config_data:
                        config_data["embedding"] = {}
                    config_data["embedding"].update(workspace_data["embedding"])
                if "chunking" in workspace_data:
                    if "chunking" not in config_data:
                        config_data["chunking"] = {}
                    config_data["chunking"].update(workspace_data["chunking"])
                if "inferred" in workspace_data:
                    if "inferred" not in config_data:
                        config_data["inferred"] = {}
                    config_data["inferred"].update(workspace_data["inferred"])

    return _dict_to_config(config_data)
