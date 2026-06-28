"""Reranker for improving search results.

Uses cross-encoder models to rerank top-N results from hybrid search.
"""

from typing import Any

import httpx
import numpy as np


class OllamaReranker:
    """Ollama-based reranker using BGE or similar cross-encoder models."""

    def __init__(
        self,
        model: str = "qllama/bge-reranker-v2-m3",
        base_url: str = "http://localhost:11434",
    ):
        """Initialize reranker.

        Args:
            model: Ollama model name for reranking.
            base_url: Ollama API base URL.
        """
        self.model = model
        self.base_url = base_url.rstrip("/")
        self.client = httpx.Client(timeout=60.0)

    def _get_score(self, query: str, passage: str) -> float:
        """Get relevance score for query-passage pair.

        BGE reranker uses format: "query: X\npassage: Y"
        Returns embedding, we use L2 norm as score (higher = more relevant).
        """
        text = f"query: {query}\npassage: {passage}"

        response = self.client.post(
            f"{self.base_url}/api/embeddings",
            json={"model": self.model, "prompt": text},
        )
        response.raise_for_status()

        data = response.json()
        embedding = data.get("embedding", [])

        if not embedding:
            return 0.0

        # Use L2 norm as relevance score
        # Higher norm = more relevant for BGE reranker
        return float(np.linalg.norm(embedding))

    def rerank(
        self,
        query: str,
        results: list[dict[str, Any]],
        text_field: str = "text",
        top_k: int | None = None,
    ) -> list[dict[str, Any]]:
        """Rerank search results by relevance to query.

        Args:
            query: Search query.
            results: List of search results with text field.
            text_field: Name of field containing document text.
            top_k: Return only top-K results after reranking.

        Returns:
            Reranked results with added 'rerank_score' field.
        """
        if not results:
            return []

        # Score each result
        scored = []
        for result in results:
            text = result.get(text_field, "")
            if not text:
                text = result.get("object_id", "")

            # Truncate long passages (reranker has context limit)
            if len(text) > 512:
                text = text[:512]

            score = self._get_score(query, text)
            scored.append(
                {
                    **result,
                    "rerank_score": score,
                }
            )

        # Sort by rerank score (descending)
        scored.sort(key=lambda x: x["rerank_score"], reverse=True)

        if top_k:
            scored = scored[:top_k]

        return scored


def get_reranker(config: Any = None) -> OllamaReranker | None:
    """Get reranker instance from config.

    Args:
        config: Config object with reranker settings.

    Returns:
        Reranker instance or None if not configured.
    """
    if config is None:
        return None

    reranker_config = getattr(config, "reranker", None)
    if not reranker_config:
        return None

    provider = getattr(reranker_config, "provider", "ollama")
    if provider != "ollama":
        return None

    model = getattr(reranker_config, "model", "qllama/bge-reranker-v2-m3")
    base_url = getattr(reranker_config, "base_url", "http://localhost:11434")

    return OllamaReranker(model=model, base_url=base_url)
