"""Embedding providers for QMDC Semantic."""

from .ollama import OllamaProvider
from .openrouter import OpenRouterProvider

__all__ = ["OllamaProvider", "OpenRouterProvider"]
