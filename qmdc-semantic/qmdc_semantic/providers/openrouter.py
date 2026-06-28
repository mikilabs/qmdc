"""OpenRouter embedding provider."""

import os

import httpx
import numpy as np
from tqdm import tqdm


class OpenRouterProvider:
    """OpenRouter embedding provider.

    Uses OpenRouter API for cloud embeddings (OpenAI-compatible).
    """

    BASE_URL = "https://openrouter.ai/api/v1/embeddings"

    def __init__(
        self,
        model: str = "openai/text-embedding-3-small",
        api_key_env: str | None = "OPENROUTER_API_KEY",
        dimension: int | None = None,
        batch_size: int = 50,
    ):
        """Initialize OpenRouter provider.

        Args:
            model: Model name (e.g., openai/text-embedding-3-small).
            api_key_env: Environment variable name for API key.
            dimension: Expected embedding dimension (auto-detected if None).
            batch_size: Batch size for embedding requests.
        """
        self.model = model
        self.api_key_env = api_key_env
        self._dimension = dimension
        self.batch_size = batch_size

        # Get API key
        self.api_key = os.environ.get(api_key_env or "OPENROUTER_API_KEY")
        if not self.api_key:
            raise ValueError(f"API key not found in environment variable: {api_key_env}")

        self.client = httpx.Client(timeout=60.0)

    def _embed_batch(self, texts: list[str]) -> list[np.ndarray]:
        """Embed a batch of texts."""
        response = self.client.post(
            self.BASE_URL,
            headers={
                "Authorization": f"Bearer {self.api_key}",
                "Content-Type": "application/json",
            },
            json={"model": self.model, "input": texts},
        )
        response.raise_for_status()
        data = response.json()

        # Sort by index to maintain order
        embeddings = sorted(data["data"], key=lambda x: x["index"])
        return [np.array(emb["embedding"], dtype=np.float32) for emb in embeddings]

    def embed(self, texts: list[str]) -> list[np.ndarray]:
        """Embed a list of texts with batching, progress bar, and rate limiting.

        Args:
            texts: List of text strings.

        Returns:
            List of embedding vectors.
        """
        import time

        if not texts:
            return []

        results = []
        for i in tqdm(range(0, len(texts), self.batch_size), desc="Embedding"):
            batch = texts[i : i + self.batch_size]
            batch_embeddings = self._embed_batch(batch)
            results.extend(batch_embeddings)

            # Rate limiting: 10 req/sec max
            if i + self.batch_size < len(texts):
                time.sleep(0.1)

        return results

    def get_dimension(self) -> int:
        """Get embedding dimension (auto-detect if not set)."""
        if self._dimension is None:
            # Auto-detect via test embedding
            test_emb = self._embed_batch(["test"])[0]
            self._dimension = len(test_emb)
        return self._dimension
