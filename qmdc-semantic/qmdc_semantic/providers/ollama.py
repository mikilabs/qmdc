"""Ollama embedding provider."""

import httpx
import numpy as np
from tqdm import tqdm


class OllamaProvider:
    """Ollama embedding provider.

    Uses the Ollama API for local embeddings.
    """

    def __init__(
        self,
        model: str = "nomic-embed-text",
        base_url: str = "http://localhost:11434",
        dimension: int | None = None,
        batch_size: int = 100,
    ):
        """Initialize Ollama provider.

        Args:
            model: Model name (e.g., nomic-embed-text, mxbai-embed-large).
            base_url: Ollama API base URL.
            dimension: Expected embedding dimension (auto-detected if None).
            batch_size: Batch size for embedding requests.
        """
        self.model = model
        self.base_url = base_url.rstrip("/")
        self._dimension = dimension
        self.batch_size = batch_size
        self.client = httpx.Client(timeout=300.0)

    def _embed_single(self, text: str) -> np.ndarray:
        """Embed a single text."""
        response = self.client.post(
            f"{self.base_url}/api/embeddings",
            json={"model": self.model, "prompt": text},
        )
        response.raise_for_status()
        data = response.json()
        return np.array(data["embedding"], dtype=np.float32)

    def _embed_batch(self, texts: list[str]) -> list[np.ndarray]:
        """Embed a batch of texts (one at a time, Ollama doesn't support batch)."""
        return [self._embed_single(text) for text in texts]

    def embed(self, texts: list[str]) -> list[np.ndarray]:
        """Embed a list of texts with batching and progress bar.

        Args:
            texts: List of text strings.

        Returns:
            List of embedding vectors.
        """
        if not texts:
            return []

        results = []
        for i in tqdm(range(0, len(texts), self.batch_size), desc="Embedding"):
            batch = texts[i : i + self.batch_size]
            batch_embeddings = self._embed_batch(batch)
            results.extend(batch_embeddings)

        return results

    def get_dimension(self) -> int:
        """Get embedding dimension (auto-detect if not set)."""
        if self._dimension is None:
            # Auto-detect via test embedding
            test_emb = self._embed_single("test")
            self._dimension = len(test_emb)
        return self._dimension
