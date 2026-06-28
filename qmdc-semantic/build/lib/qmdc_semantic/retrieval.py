"""Parent document retrieval for context.

Based on Finding 8: Load parent chunks for child chunk context.
"""

from typing import Any

from .storage import Storage


def get_parent_context(chunk_id: str, storage: Storage) -> str | None:
    """Get parent chunk text for context.

    If the chunk is a child chunk, returns the parent chunk text
    for additional context.

    Args:
        chunk_id: ID of the chunk.
        storage: Storage instance.

    Returns:
        Parent chunk text if available, None otherwise.
    """
    chunk = storage.get_chunk(chunk_id)
    if not chunk:
        return None

    if chunk.get("chunk_type") == "child" and chunk.get("parent_chunk_id"):
        parent = storage.get_chunk(chunk["parent_chunk_id"])
        if parent:
            return parent.get("text")

    return None


def enrich_results_with_context(
    results: list[dict[str, Any]],
    storage: Storage,
) -> list[dict[str, Any]]:
    """Enrich search results with parent context.

    For child chunks, adds parent_context field with the parent chunk text.

    Args:
        results: List of search result dicts.
        storage: Storage instance.

    Returns:
        Results with added parent_context field where applicable.
    """
    for result in results:
        chunk_id = result.get("chunk_id")
        if chunk_id:
            parent_context = get_parent_context(chunk_id, storage)
            if parent_context:
                result["parent_context"] = parent_context

    return results


def get_object_full_context(object_id: str, storage: Storage) -> dict[str, Any] | None:
    """Get full context for an object including all chunks.

    Args:
        object_id: Object ID in __global_id format.
        storage: Storage instance.

    Returns:
        Dict with object metadata and all chunk texts, or None if not found.
    """
    cursor = storage.conn.cursor()
    cursor.execute(
        "SELECT * FROM chunks WHERE object_id = ? ORDER BY chunk_type",
        (object_id,),
    )
    chunks = [dict(row) for row in cursor.fetchall()]

    if not chunks:
        return None

    # Build context
    context = {
        "object_id": object_id,
        "object_kind": chunks[0].get("object_kind"),
        "source_file": chunks[0].get("source_file"),
        "chunks": [],
    }

    for chunk in chunks:
        context["chunks"].append(
            {
                "chunk_id": chunk["chunk_id"],
                "chunk_type": chunk.get("chunk_type"),
                "text": chunk.get("text"),
            }
        )

    return context
