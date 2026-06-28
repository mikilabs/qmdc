# Semantic Layer [[semantic:__Namespace]]

Semantic search for QMDC workspaces.

## Overview

QMDC Semantic provides semantic search capabilities for QMDC workspaces:

- **Hybrid Search**: Combines keyword search (FTS5), trigram substring matching, and dense vector search
- **Graph Walk**: Expands results through explicit and inferred edges
- **Inferred Edges**: Discovers semantic relationships between objects
- **Incremental Indexing**: Hash-based updates for efficiency

## Quick Start

### Installation

```bash
# From the qmdc repo root
uv pip install -e ./qmdc-py -e ./qmdc-semantic
```

### Index a Workspace

```bash
qmdc-semantic index /path/to/workspace
```

Creates `.qmdc-semantic/embeddings.db` with:

- Chunk embeddings
- FTS5 index
- Inferred edges

### Search

```bash
# Text query
qmdc-semantic search /path/to/workspace "how to test LSP"

# Impact scan (query from file)
qmdc-semantic search /path/to/workspace --from-file task.qmd.md -k 20
```

## Configuration

Create `.qmdc-semantic/config.yaml` in workspace or `~/.qmdc-semantic/config.yaml` globally.

See [[#semantic_configuration]] for details.

## Namespace Contents

- [[#semantic_commands]] — CLI reference
- [[#semantic_algorithms]] — Algorithm documentation
- [[#semantic_configuration]] — Configuration guide
- [[#semantic_storage]] — Storage schema
- [[#semantic_testing]] — Testing guide
