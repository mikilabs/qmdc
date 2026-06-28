"""Semantic search with hybrid approach.

Based on Finding 6: FTS5 + Dense embeddings with RRF fusion.
Based on Finding 8: Graph walk for result expansion.
"""

from collections import defaultdict
from typing import Any

from .config import Config
from .embedding import get_provider
from .storage import Storage


def _is_id_query(query: str) -> bool:
    """Check if query looks like an ID (e.g., QMD-17, TASK-123)."""
    import re

    # Short query with ID-like pattern
    words = query.split()
    if len(words) <= 2:
        # Check for patterns like QMD-17, QMD17, TASK-123
        for word in words:
            if re.match(r"^[A-Za-z]+-?\d+$", word):
                return True
    return False


def hybrid_fusion(
    dense_results: list[tuple[str, float]],
    fts_results: list[tuple[str, float]],
    query: str = "",
    trigram_results: list[tuple[str, float]] | None = None,
) -> dict[str, float]:
    """Score-based fusion of dense, FTS, and trigram results.

    Weights are dynamic:
    - For ID queries (QMD-17): FTS/trigram get higher weight (exact + substring match matters)
    - For semantic queries: Dense gets higher weight (meaning matters)

    Args:
        dense_results: Dense search results [(id, distance), ...]. Lower is better.
        fts_results: FTS results [(id, bm25_score), ...]. More negative is better.
        query: Original query string for weight adjustment.
        trigram_results: Optional trigram substring results [(id, bm25_score), ...].
            More negative is better. When None, behaves as a pure dense+FTS fusion.

    Returns:
        Dict of id -> fused score. Higher is better.
    """
    # Dynamic weights based on query type
    if _is_id_query(query):
        dense_weight, fts_weight, trigram_weight = 0.3, 0.7, 0.4  # exact/substring for IDs
    else:
        dense_weight, fts_weight, trigram_weight = 0.7, 0.3, 0.2  # meaning for semantic

    final_scores: dict[str, float] = {}

    # Normalize dense scores (distance -> similarity, 0-1 range)
    if dense_results:
        dense_distances = [d for _, d in dense_results]
        max_dist = max(dense_distances) if dense_distances else 1.0
        min_dist = min(dense_distances) if dense_distances else 0.0
        dist_range = max_dist - min_dist if max_dist != min_dist else 1.0

        for doc_id, distance in dense_results:
            # Convert distance to similarity (1 = closest, 0 = farthest)
            similarity = 1.0 - (distance - min_dist) / dist_range
            final_scores[doc_id] = dense_weight * similarity

    # Add FTS scores (BM25, more negative = better match)
    if fts_results:
        fts_scores = [abs(s) for _, s in fts_results]
        max_fts = max(fts_scores) if fts_scores else 1.0
        min_fts = min(fts_scores) if fts_scores else 0.0
        fts_range = max_fts - min_fts if max_fts != min_fts else 1.0

        for doc_id, score in fts_results:
            # Normalize BM25 (more negative = higher score after normalization)
            norm_fts = (abs(score) - min_fts) / fts_range
            if doc_id in final_scores:
                final_scores[doc_id] += fts_weight * norm_fts
            else:
                # FTS-only result gets partial score
                final_scores[doc_id] = fts_weight * norm_fts

    # Add trigram scores (substring FTS5, more negative = better match).
    # Max-abs normalization (not min-max) so a single trigram hit still yields a
    # positive boost — a present substring match should always lift its chunk.
    if trigram_results:
        max_tri = max((abs(s) for _, s in trigram_results), default=1.0) or 1.0
        for doc_id, score in trigram_results:
            norm_tri = abs(score) / max_tri
            final_scores[doc_id] = final_scores.get(doc_id, 0.0) + trigram_weight * norm_tri

    return final_scores


def group_by_object(
    chunk_scores: dict[str, float],
    storage: Storage,
) -> dict[str, float]:
    """Group chunk scores by object, using max + count boost.

    Objects with more matching chunks get a boost to surface documents
    that might have multiple relevant chunks.

    Args:
        chunk_scores: Dict of chunk_id -> score.
        storage: Storage instance to get chunk metadata.

    Returns:
        Dict of object_id -> aggregated score.
    """

    object_max_scores: dict[str, float] = defaultdict(float)
    object_chunk_counts: dict[str, int] = defaultdict(int)
    object_avg_scores: dict[str, list[float]] = defaultdict(list)

    for chunk_id, score in chunk_scores.items():
        chunk = storage.get_chunk(chunk_id)
        if chunk:
            object_id = chunk["object_id"]
            object_max_scores[object_id] = max(object_max_scores[object_id], score)
            object_chunk_counts[object_id] += 1
            object_avg_scores[object_id].append(score)

    # Combine: max score + boost for multiple chunks + average score factor
    object_scores = {}
    for object_id in object_max_scores:
        max_score = object_max_scores[object_id]
        chunk_count = object_chunk_counts[object_id]
        avg_score = sum(object_avg_scores[object_id]) / len(object_avg_scores[object_id])

        # Boost: log(1 + chunk_count) gives diminishing returns
        # Average factor: objects with consistently high scores get boost
        import math

        count_boost = 1 + 0.1 * math.log(1 + chunk_count)
        avg_factor = 0.5 + 0.5 * (avg_score / max_score) if max_score > 0 else 1.0

        object_scores[object_id] = max_score * count_boost * avg_factor

    return object_scores


def graph_walk(
    seeds: list[str],
    storage: Storage,
    depth: int = 2,
) -> set[str]:
    """Walk graph from seed objects to find related objects.

    Uses both explicit edges (from qmd_parser) and inferred edges.

    Args:
        seeds: List of starting object IDs.
        storage: Storage instance.
        depth: Max depth to walk.

    Returns:
        Set of all discovered object IDs (including seeds).
    """
    discovered = set(seeds)
    frontier = set(seeds)

    for _ in range(depth):
        next_frontier = set()
        for obj_id in frontier:
            neighbors = storage.get_neighbors(obj_id)
            for neighbor_id, _weight, _edge_type in neighbors:
                if neighbor_id not in discovered:
                    discovered.add(neighbor_id)
                    next_frontier.add(neighbor_id)
        frontier = next_frontier
        if not frontier:
            break

    return discovered


def _object_in_excluded_ns(object_id: str, storage: Storage, exclude_ns: list[str]) -> bool:
    """Check if an object belongs to an excluded namespace.

    Uses two heuristics:
    1. Global ID format: "workspace:namespace:local_id" — check namespace segment
    2. source_file path: "namespace/..." — check path prefix

    Args:
        object_id: Object's global ID (e.g., "docs2:tracking:qmd55").
        storage: Storage instance for chunk metadata lookup.
        exclude_ns: List of namespace names to exclude.

    Returns:
        True if the object should be excluded.
    """
    # Check global ID format: workspace:namespace:id
    parts = object_id.split(":")
    if len(parts) >= 3:
        namespace = parts[1]
        if namespace in exclude_ns:
            return True

    # Fallback: check source_file from chunk metadata
    chunk = storage.conn.execute(
        "SELECT source_file FROM chunks WHERE object_id = ? LIMIT 1",
        (object_id,),
    ).fetchone()
    if chunk and chunk[0]:
        source_file = chunk[0]
        for ns in exclude_ns:
            if source_file.startswith(f"{ns}/"):
                return True

    return False


def _filter_excluded_ns(
    object_scores: dict[str, float],
    storage: Storage,
    exclude_ns: list[str],
) -> dict[str, float]:
    """Filter out objects belonging to excluded namespaces.

    Args:
        object_scores: Dict of object_id -> score.
        storage: Storage instance.
        exclude_ns: List of namespace names to exclude.

    Returns:
        Filtered dict without excluded namespace objects.
    """
    return {
        obj_id: score
        for obj_id, score in object_scores.items()
        if not _object_in_excluded_ns(obj_id, storage, exclude_ns)
    }


def semantic_search(
    storage: Storage,
    query: str,
    config: Config,
    top_k: int = 10,
    depth: int = 2,
    exclude_ns: list[str] | None = None,
) -> list[dict[str, Any]]:
    """Perform semantic search with hybrid approach and graph walk.

    Algorithm:
    1. Dense search via vec0 KNN
    2. FTS5 keyword search
    3. RRF fusion
    4. Group by object
    5. Filter excluded namespaces
    6. Graph walk from top results
    7. Rerank all by query similarity
    8. Return top-K

    Args:
        storage: Storage instance.
        query: Search query text.
        config: Configuration.
        top_k: Number of results to return.
        depth: Graph walk depth.
        exclude_ns: List of namespace names to exclude from results.

    Returns:
        List of result dicts with object_id, score, metadata.
    """
    # Get embedding provider
    provider = get_provider(config.embedding)
    model_id = f"{config.embedding.provider}:{config.embedding.model}"

    # Embed query
    query_embedding = provider.embed([query])[0]

    # 1. Dense search
    dense_results = storage.knn_search(query_embedding, k=top_k * 2, model_id=model_id)

    # 2. FTS5 search
    # Simple tokenization for FTS5 query
    fts_query = " OR ".join(query.split())
    try:
        fts_results = storage.fts_search(fts_query, limit=top_k * 2)
    except Exception:
        # FTS5 query might fail on special chars
        fts_results = []

    # 2b. Trigram substring search (finds e.g. "333" inside "me333")
    try:
        trigram_results = storage.trigram_search(query, limit=top_k * 2)
    except Exception:
        # Trigram query might fail on special chars / unindexed db
        trigram_results = []

    # 3. Hybrid fusion (dynamic weights based on query type)
    chunk_scores = hybrid_fusion(dense_results, fts_results, query, trigram_results)

    # 4. Group by object
    object_scores = group_by_object(chunk_scores, storage)

    # 5. Filter excluded namespaces
    if exclude_ns:
        object_scores = _filter_excluded_ns(object_scores, storage, exclude_ns)

    # 6. Get top seeds for graph walk
    sorted_objects = sorted(object_scores.items(), key=lambda x: x[1], reverse=True)
    seed_objects = [obj_id for obj_id, _ in sorted_objects[: top_k // 2]]

    # 7. Graph walk
    if depth > 0 and seed_objects:
        expanded_objects = graph_walk(seed_objects, storage, depth)
    else:
        expanded_objects = set(object_scores.keys())

    # Filter expanded objects too
    if exclude_ns:
        expanded_objects = {
            obj_id
            for obj_id in expanded_objects
            if not _object_in_excluded_ns(obj_id, storage, exclude_ns)
        }

    # 8. Rerank all discovered objects
    # Objects from initial search keep their scores
    # Objects from graph walk get reranked by query similarity
    all_results = []

    # Find objects that need reranking (from graph walk, not in initial results)
    objects_to_rerank = expanded_objects - set(object_scores.keys())

    # Rerank graph walk objects using KNN on their chunks
    if objects_to_rerank:
        # Get chunk IDs for objects to rerank
        rerank_chunk_ids = []
        chunk_to_object = {}

        for obj_id in objects_to_rerank:
            chunks = storage.conn.execute(
                "SELECT chunk_id FROM chunks WHERE object_id = ?",
                (obj_id,),
            ).fetchall()
            for (chunk_id,) in chunks:
                rerank_chunk_ids.append(chunk_id)
                chunk_to_object[chunk_id] = obj_id

        # KNN search to get similarity scores for these chunks
        if rerank_chunk_ids:
            # Use a larger K to cover all chunks we need
            rerank_results = storage.knn_search(
                query_embedding, k=len(rerank_chunk_ids) + 50, model_id=model_id
            )

            # Convert to scores (distance -> similarity)
            for chunk_id, distance in rerank_results:
                if chunk_id in chunk_to_object:
                    obj_id = chunk_to_object[chunk_id]
                    similarity = max(0, 1.0 - distance)  # Cosine distance to similarity
                    # Apply small graph walk discount (0.8x) to slightly prefer direct matches
                    discounted_score = similarity * 0.8
                    # Keep max score per object
                    if obj_id not in object_scores or discounted_score > object_scores[obj_id]:
                        object_scores[obj_id] = discounted_score

    # Build results for all objects
    for obj_id in expanded_objects:
        best_chunk = None
        best_score = object_scores.get(obj_id, 0.0)
        longest_chunk = None
        longest_len = 0

        chunks = storage.conn.execute(
            "SELECT chunk_id, chunk_type, length(text) as text_len FROM chunks WHERE object_id = ?",
            (obj_id,),
        ).fetchall()

        for chunk_id, chunk_type, text_len in chunks:
            if best_chunk is None:
                best_chunk = chunk_id
            # Track longest non-combined chunk for snippet
            if chunk_type != "combined" and (text_len or 0) > longest_len:
                longest_len = text_len or 0
                longest_chunk = chunk_id

        if best_chunk and best_score > 0:
            chunk_data = storage.get_chunk(best_chunk)
            if chunk_data:
                snippet_text = chunk_data.get("text", "")
                if longest_chunk and longest_chunk != best_chunk:
                    snippet_data = storage.get_chunk(longest_chunk)
                    if snippet_data:
                        snippet_text = snippet_data.get("text", "")

                all_results.append(
                    {
                        "object_id": obj_id,
                        "score": best_score,
                        "object_kind": chunk_data.get("object_kind"),
                        "source_file": chunk_data.get("source_file"),
                        "text": snippet_text,
                        "chunk_type": chunk_data.get("chunk_type"),
                    }
                )

    # Sort by score and return top-K
    all_results.sort(key=lambda x: x["score"], reverse=True)
    return all_results[:top_k]
