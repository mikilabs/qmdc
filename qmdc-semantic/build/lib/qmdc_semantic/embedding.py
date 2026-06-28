"""Embedding provider abstraction.

Based on Finding 5: Protocol-based providers with auto-dimension detection.
"""

from typing import Protocol

import numpy as np

from .config import EmbeddingConfig


class EmbeddingProvider(Protocol):
    """Protocol for embedding providers."""

    def embed(self, texts: list[str]) -> list[np.ndarray]:
        """Embed a list of texts.

        Args:
            texts: List of text strings.

        Returns:
            List of embedding vectors (numpy arrays).
        """
        ...

    def get_dimension(self) -> int:
        """Get embedding dimension.

        Returns:
            Dimension of embedding vectors.
        """
        ...


def get_provider(config: EmbeddingConfig) -> EmbeddingProvider:
    """Get embedding provider from config.

    Args:
        config: Embedding configuration.

    Returns:
        Configured embedding provider.

    Raises:
        ValueError: If provider type is unknown.
    """
    if config.provider == "ollama":
        from .providers.ollama import OllamaProvider

        return OllamaProvider(
            model=config.model,
            base_url=config.base_url,
            dimension=config.dimension,
        )
    elif config.provider == "openrouter":
        from .providers.openrouter import OpenRouterProvider

        return OpenRouterProvider(
            model=config.model,
            api_key_env=config.api_key_env,
            dimension=config.dimension,
        )
    else:
        raise ValueError(f"Unknown embedding provider: {config.provider}")
