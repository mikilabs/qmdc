"""QMDC Semantic - Semantic search for QMDC workspaces."""

__version__ = "1.0.0"

from .chunking import extract_chunks
from .config import load_config
from .search import semantic_search
from .storage import Storage

__all__ = [
    "extract_chunks",
    "load_config",
    "semantic_search",
    "Storage",
]
