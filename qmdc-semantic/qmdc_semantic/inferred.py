"""Inferred edges computation.

Based on Finding 7: Semantic similarity edges via vec0 KNN.
Based on Finding 11: Avoid O(n²) with KNN approach.
"""

from collections import defaultdict

import numpy as np

from .config import InferredConfig
from .storage import Storage


def compute_inferred_edges(
    storage: Storage,
    config: InferredConfig,
):
    """Compute inferred edges between objects based on semantic similarity.

    Uses vec0 KNN to find similar chunks, then aggregates by object.
    Avoids O(n²) by using KNN search instead of pairwise comparison.

    Algorithm:
    1. For each object, get its representative chunk embedding
    2. KNN search to find top-K similar chunks
    3. Aggregate by object pair
    4. Filter by threshold
    5. Save edges (exclude self-references)

    Args:
        storage: Storage instance with embeddings.
        config: Inferred edges configuration.
    """
    # Get all unique objects with embeddings
    chunks = storage.get_all_chunks()
    if not chunks:
        return

    # Group chunks by object
    object_chunks = defaultdict(list)
    for chunk in chunks:
        obj_id = chunk["object_id"]
        object_chunks[obj_id].append(chunk)

    # Get dimension from first chunk (via vec table)
    cursor = storage.conn.cursor()
    cursor.execute("SELECT name FROM sqlite_master WHERE type='table' AND name LIKE 'vec_chunks_%'")
    vec_tables = [row[0] for row in cursor.fetchall()]
    if not vec_tables:
        return  # No embeddings stored yet

    # Use the first vec table
    vec_table = vec_tables[0]

    # Collect all edges
    edges = []
    seen_pairs = set()

    for obj_id, obj_chunks_list in object_chunks.items():
        # Get representative chunk (first one for simplicity)
        # In practice, could use parent chunk or average embedding
        rep_chunk_id = obj_chunks_list[0]["chunk_id"]

        # Get embedding for representative chunk
        cursor.execute(
            f"SELECT embedding FROM {vec_table} WHERE chunk_id = ?",
            (rep_chunk_id,),
        )
        row = cursor.fetchone()
        if not row:
            continue

        embedding_bytes = row[0]
        embedding = np.frombuffer(embedding_bytes, dtype=np.float32)

        # KNN search
        similar = storage.knn_search(embedding, k=config.top_k)

        for similar_chunk_id, distance in similar:
            # Get object for similar chunk
            similar_chunk = storage.get_chunk(similar_chunk_id)
            if not similar_chunk:
                continue

            similar_obj_id = similar_chunk["object_id"]

            # Skip self-references
            if similar_obj_id == obj_id:
                continue

            # Convert distance to similarity (cosine distance -> similarity)
            # sqlite-vec returns distance, smaller is more similar
            similarity = 1.0 - distance

            # Apply threshold
            if similarity < config.similarity_threshold:
                continue

            # Avoid duplicate edges (symmetric)
            pair = tuple(sorted([obj_id, similar_obj_id]))
            if pair in seen_pairs:
                continue
            seen_pairs.add(pair)

            edges.append((obj_id, similar_obj_id, similarity))

    # Clear old edges and save new ones
    cursor.execute("DELETE FROM inferred_edges")
    storage.save_inferred_edges(edges)
