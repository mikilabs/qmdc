"""Embedding providers for QMD Semantic."""

from .ollama import OllamaProvider
from .openrouter import OpenRouterProvider

__all__ = ["OllamaProvider", "OpenRouterProvider"]
