"""CLI commands for QMD Semantic."""

import re
from pathlib import Path

import click

from .config import load_config


def make_snippet(text: str, query: str, max_len: int = 120) -> str:
    """Create a search snippet highlighting relevant text.

    Args:
        text: Full text to extract snippet from.
        query: Query to find relevant parts.
        max_len: Max snippet length.

    Returns:
        Snippet string with ellipsis if truncated.
    """
    if not text:
        return ""

    # Clean text - remove extra whitespace and newlines
    text = " ".join(text.split())

    # Find query words
    query_words = [w.lower() for w in re.findall(r"\w+", query) if len(w) > 2]

    # Find best position (first match of any query word)
    best_pos = 0
    for word in query_words:
        match = re.search(rf"\b{re.escape(word)}\b", text, re.IGNORECASE)
        if match:
            best_pos = max(0, match.start() - 30)  # Start 30 chars before match
            break

    # Extract snippet
    if len(text) <= max_len:
        return text

    start = best_pos
    end = min(start + max_len, len(text))

    # Adjust to word boundaries
    if start > 0:
        # Find next space after start
        space = text.find(" ", start)
        if space != -1 and space < start + 20:
            start = space + 1

    if end < len(text):
        # Find last space before end
        space = text.rfind(" ", start, end)
        if space != -1 and space > end - 20:
            end = space

    snippet = text[start:end]

    # Add ellipsis
    prefix = "..." if start > 0 else ""
    suffix = "..." if end < len(text) else ""

    return f"{prefix}{snippet}{suffix}"


@click.group()
@click.version_option()
def cli():
    """QMD Semantic - Semantic search for QMD workspaces."""
    pass


@cli.command()
@click.argument("workspace_path", type=click.Path(exists=True), default=".")
@click.option("--force", "-f", is_flag=True, help="Reindex all chunks (ignore cache)")
@click.option("--verbose", "-v", is_flag=True, help="Verbose output")
def index(workspace_path: str, force: bool, verbose: bool):
    """Index a QMD workspace for semantic search.

    Creates/updates embeddings in .qmd-semantic/embeddings.db
    """
    from .chunking import extract_chunks
    from .embedding import get_provider
    from .inferred import compute_inferred_edges
    from .storage import Storage

    workspace = Path(workspace_path).resolve()
    config = load_config(workspace)

    if verbose:
        click.echo(f"Indexing workspace: {workspace}")
        click.echo(f"Provider: {config.embedding.provider}")
        click.echo(f"Model: {config.embedding.model}")

    # Initialize storage
    storage = Storage(workspace)

    # Extract chunks from workspace
    chunks = extract_chunks(workspace, config.chunking)
    if verbose:
        click.echo(f"Extracted {len(chunks)} chunks")

    # Extract explicit edges from workspace
    explicit_edges = _extract_explicit_edges(workspace, verbose)
    if verbose:
        click.echo(f"Extracted {len(explicit_edges)} explicit edges")
    storage.save_explicit_edges(explicit_edges)

    # Compute diff
    diff = storage.compute_diff(chunks, force=force)
    if verbose:
        click.echo(
            f"Diff: {len(diff['new'])} new, {len(diff['changed'])} changed, "
            f"{len(diff['unchanged'])} unchanged, {len(diff['deleted'])} deleted"
        )

    # Skip if nothing to do
    if not diff["new"] and not diff["changed"] and not diff["deleted"]:
        click.echo("No changes detected, index is up to date.")
        return

    # Get embedding provider
    provider = get_provider(config.embedding)

    # Embed new and changed chunks
    to_embed = diff["new"] + diff["changed"]
    if to_embed:
        if verbose:
            click.echo(f"Embedding {len(to_embed)} chunks...")
        embeddings = provider.embed([c["text"] for c in to_embed])
        for chunk, embedding in zip(to_embed, embeddings, strict=True):
            chunk["embedding"] = embedding
            chunk["model_id"] = f"{config.embedding.provider}:{config.embedding.model}"

    # Save to storage
    storage.save_chunks(to_embed)
    storage.delete_chunks([c["chunk_id"] for c in diff["deleted"]])

    # Compute inferred edges
    if verbose:
        click.echo("Computing inferred edges...")
    compute_inferred_edges(storage, config.inferred)

    click.echo(f"Indexed {len(to_embed)} chunks, deleted {len(diff['deleted'])}")


def _extract_explicit_edges(workspace: Path, verbose: bool = False) -> list[tuple[str, str, str]]:
    """Extract explicit edges from [[#ref]] references in workspace.

    Args:
        workspace: Path to workspace.
        verbose: Print debug info.

    Returns:
        List of (source_global_id, target_global_id, source_field) tuples.
    """
    from qmdc import parse_workspace

    from .chunking import enrich_objects_with_global_id

    result = parse_workspace(str(workspace))
    objects = enrich_objects_with_global_id(result.objects)

    # Build index for resolving target IDs
    # by_id: {id -> global_id} for same-namespace resolution
    # by_ns_id: {namespace:id -> global_id} for cross-namespace resolution
    by_id: dict[str, str] = {}
    by_ns_id: dict[str, str] = {}

    for obj in objects:
        obj_id = obj.get("__id", "")
        global_id = obj.get("__global_id", "")
        namespace = obj.get("__namespace", "")

        if obj_id and global_id:
            by_id[obj_id] = global_id
            if namespace:
                by_ns_id[f"{namespace}:{obj_id}"] = global_id

    edges = []

    for obj in objects:
        source_global_id = obj.get("__global_id", "")
        source_namespace = obj.get("__namespace", "")
        refs = obj.get("__references", [])

        if not source_global_id or not refs:
            continue

        for ref in refs:
            target = ref.get("target", "")

            # Parse target: #id or #namespace:id
            if target.startswith("#"):
                target = target[1:]  # Remove leading #

            target_global_id = None

            if ":" in target:
                # Cross-namespace reference: #namespace:id
                target_global_id = by_ns_id.get(target)
                if not target_global_id:
                    # Try any namespace
                    target_global_id = by_ns_id.get(target) or by_id.get(target.split(":")[-1])
            else:
                # Same-namespace reference: #id
                # First try same namespace
                if source_namespace:
                    target_global_id = by_ns_id.get(f"{source_namespace}:{target}")
                # Then try any namespace
                if not target_global_id:
                    target_global_id = by_id.get(target)

            if target_global_id:
                # Determine source field from reference position
                source_field = "reference"  # Default
                edges.append((source_global_id, target_global_id, source_field))

    return edges


@cli.command()
@click.argument("workspace_path", type=click.Path(exists=True), default=".")
@click.argument("query", required=False)
@click.option("--from-file", "-f", type=click.Path(exists=True), help="Query from file content")
@click.option("--top-k", "-k", default=10, help="Number of results (default: 10)")
@click.option("--depth", "-d", default=2, help="Graph walk depth (default: 2)")
@click.option(
    "--exclude-ns",
    "-x",
    multiple=True,
    help="Exclude namespace(s) from results (repeatable, e.g. -x tracking -x ideas)",
)
@click.option("--verbose", "-v", is_flag=True, help="Verbose output")
def search(
    workspace_path: str,
    query: str | None,
    from_file: str | None,
    top_k: int,
    depth: int,
    exclude_ns: tuple[str, ...],
    verbose: bool,
):
    """Search for relevant objects in a QMD workspace.

    QUERY is a text query to search for.
    Use --from-file to use file content as query (for impact scan).
    """
    from .search import semantic_search
    from .storage import Storage

    if not query and not from_file:
        raise click.UsageError("Either QUERY or --from-file must be provided")

    workspace = Path(workspace_path).resolve()
    config = load_config(workspace)

    # Get query text
    query_text = Path(from_file).read_text() if from_file else query  # type: ignore

    if verbose:
        click.echo(f"Searching workspace: {workspace}")
        click.echo(f"Query: {query_text[:100]}...")

    # Initialize storage
    storage = Storage(workspace)

    # Perform search
    results = semantic_search(
        storage=storage,
        query=query_text,
        config=config,
        top_k=top_k,
        depth=depth,
        exclude_ns=list(exclude_ns) if exclude_ns else None,
    )

    # Display results
    click.echo(f"\nFound {len(results)} results:\n")
    for i, result in enumerate(results, 1):
        click.echo(f"{i}. [{result['object_kind']}] {result['object_id']}")
        click.echo(f"   Score: {result['score']:.3f}")
        click.echo(f"   File: {result['source_file']}")
        # Always show snippet
        if result.get("text"):
            snippet = make_snippet(result["text"], query_text)
            if snippet:
                click.echo(f"   {snippet}")
        click.echo()


if __name__ == "__main__":
    cli()
