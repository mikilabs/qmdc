# Semantic Configuration [[semantic_configuration: NarrativeDoc]]

- about: [[#algo_chunking]], [[#algo_inferred_edges]]

Configuration format and options for QMDC Semantic.

## Config File Priority

Configuration is loaded with priority:

1. **Workspace config**: `.qmdc-semantic/config.yaml` (highest)
2. **Global config**: `~/.qmdc-semantic/config.yaml`
3. **Defaults** (lowest)

Workspace config overrides global config.

## Full Format

```yaml
embedding:
  provider: ollama          # ollama | openrouter
  model: qwen3-embedding    # Model name (default)
  base_url: http://localhost:11434  # For ollama
  api_key_env: OPENROUTER_API_KEY   # For openrouter
  dimension: null           # Auto-detected if null

chunking:
  min_text_length: 30       # Minimum chunk text length
  long_field_threshold: 50  # Threshold for long fields
  max_chunk_size: 3000      # Split child chunks longer than this (chars)

inferred:
  similarity_threshold: 0.7 # Minimum similarity for edges
  top_k: 50                 # KNN neighbors per object

search:
  # Chunk types preferred for the result snippet (first match wins)
  snippet_priority: [solution, description, text, summary, body, combined]
  snippet_max_length: 120   # Max snippet length (chars)
```

> Note: the `search` section is currently applied only by the CLI's snippet rendering. Only the `embedding`, `chunking`, and `inferred` sections are deep-merged from workspace config over global config; `search` falls back to defaults.

## Embedding Providers

### Ollama (Local)

```yaml
embedding:
  provider: ollama
  model: qwen3-embedding
  base_url: http://localhost:11434
```

Supported models (any Ollama embedding model works; dimension is auto-detected):

- `qwen3-embedding` (default)
- `nomic-embed-text` (768 dim)
- `mxbai-embed-large` (1024 dim)
- `all-minilm` (384 dim)

Requirements: Ollama installed and running, model pulled via `ollama pull qwen3-embedding`.

### OpenRouter (Cloud)

```yaml
embedding:
  provider: openrouter
  model: openai/text-embedding-3-small
  api_key_env: OPENROUTER_API_KEY
```

Supported models:

- `openai/text-embedding-3-small` (1536 dim)
- `openai/text-embedding-3-large` (3072 dim)

Requirements: `OPENROUTER_API_KEY` environment variable set. Rate limits apply (~10 req/sec).

## Chunking Options

- about: [[#algo_chunking]]

| Option | Default | Description |
|--------|---------|-------------|
| `min_text_length` | 30 | Chunks shorter than this are filtered |
| `long_field_threshold` | 50 | Fields longer than this become child chunks |
| `max_chunk_size` | 3000 | Child chunks longer than this are split at section/paragraph boundaries |

## Inferred Edges Options

- about: [[#algo_inferred_edges]]

| Option | Default | Description |
|--------|---------|-------------|
| `similarity_threshold` | 0.7 | Minimum cosine similarity for edge |
| `top_k` | 50 | Number of KNN neighbors to consider |

## Example Configs

### Local Development

```yaml
# .qmdc-semantic/config.yaml
embedding:
  provider: ollama
  model: qwen3-embedding
```

### Production (OpenRouter)

```yaml
# ~/.qmdc-semantic/config.yaml
embedding:
  provider: openrouter
  model: openai/text-embedding-3-small
  api_key_env: OPENROUTER_API_KEY
```

### Strict Inferred Edges

```yaml
# Higher threshold, fewer but stronger edges
inferred:
  similarity_threshold: 0.85
  top_k: 20
```
