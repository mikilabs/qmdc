# QMDC Semantic

Semantic search for QMDC workspaces with hybrid approach (FTS5 + Dense embeddings) and graph walk.

## Installation

```bash
cd qmdc-semantic
uv pip install -e .
```

**Requirements:**

- Python 3.12+
- qmdc (install from `../qmdc-py`)
- sqlite-vec extension
- Ollama (for local embeddings) or OpenRouter API key

## Quick Start

```bash
# 1. Index a workspace
qmdc-semantic index /path/to/workspace

# 2. Search
qmdc-semantic search /path/to/workspace "how to test LSP"

# Impact scan (query from file)
qmdc-semantic search /path/to/workspace --from-file task.qmd.md
```

## Configuration

Create `.qmdc-semantic/config.yaml` in workspace or `~/.qmdc-semantic/config.yaml` globally:

```yaml
embedding:
  provider: ollama        # ollama | openrouter
  model: nomic-embed-text
  base_url: http://localhost:11434

# Or for OpenRouter:
# embedding:
#   provider: openrouter
#   model: openai/text-embedding-3-small
#   api_key_env: OPENROUTER_API_KEY

chunking:
  min_text_length: 10
  long_field_threshold: 50

inferred:
  similarity_threshold: 0.7
  top_k: 50
```

## Features

- **Hybrid Search**: Combines FTS5 keyword search with dense vector search
- **RRF Fusion**: Reciprocal Rank Fusion for combining rankings
- **Graph Walk**: Expands results through explicit and inferred edges
- **Inferred Edges**: Computes semantic similarity between objects
- **Incremental Update**: Hash-based diff for efficient re-indexing
- **Parent Document Retrieval**: Returns context for child chunks

## CLI Commands

### index

```bash
qmdc-semantic index [WORKSPACE_PATH] [--force] [--verbose]
```

Creates/updates embeddings in `.qmdc-semantic/embeddings.db`.

Options:

- `--force, -f`: Reindex all chunks (ignore cache)
- `--verbose, -v`: Verbose output

### search

```bash
qmdc-semantic search [WORKSPACE_PATH] "query" [-k N] [--depth N] [--from-file FILE]
```

Search for relevant objects.

Options:

- `-k, --top-k`: Number of results (default: 10)
- `--depth, -d`: Graph walk depth (default: 2)
- `--from-file, -f`: Use file content as query (for impact scan)
- `--verbose, -v`: Verbose output

## API Usage

```python
from qmdc_semantic import load_config, Storage, extract_chunks, semantic_search

# Load config
config = load_config("/path/to/workspace")

# Initialize storage
storage = Storage("/path/to/workspace")

# Extract and index chunks (see cli.py for full flow)
chunks = extract_chunks("/path/to/workspace", config.chunking)

# Search
results = semantic_search(
    storage=storage,
    query="how to test LSP",
    config=config,
    top_k=10,
    depth=2,
)

for result in results:
    print(f"{result['object_kind']}: {result['object_id']}")
    print(f"  Score: {result['score']:.3f}")
```

## Storage

Data is stored in `.qmdc-semantic/embeddings.db` (SQLite):

- `chunks`: Metadata and text for each chunk
- `vec_chunks_{dim}`: Vector embeddings (via sqlite-vec)
- `chunks_fts`: FTS5 index for keyword search
- `inferred_edges`: Semantic similarity edges

**Git LFS**: For large workspaces, consider using Git LFS for the database file:

```bash
git lfs track ".qmdc-semantic/*.db"
```

## Development

```bash
# Install with dev dependencies
uv pip install -e ".[dev]"

# Run tests
uv run pytest tests/ -v

# Run linter
uv run ruff check .
```

## License

[AGPL-3.0-or-later](LICENSE) © mikilabs
