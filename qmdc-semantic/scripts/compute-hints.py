#!/usr/bin/env python3
"""
Compute per-object and per-field semantic hints as JSON.

For each chunk (object or field), finds the most similar chunks in other files
that have no explicit edge. Outputs a JSON file (hints.json) for downstream
consumers such as the semantic audit and site generators.

Usage:
    uv run python3 scripts/compute-hints.py <workspace> [output_file]
    uv run python3 scripts/compute-hints.py ../docs
    uv run python3 scripts/compute-hints.py ../docs ../docs/.qmdc-semantic/hints.json
"""

import json
import sys
from pathlib import Path

import numpy as np

workspace = Path(sys.argv[1]).resolve() if len(sys.argv) > 1 else Path("../docs").resolve()
output = sys.argv[2] if len(sys.argv) > 2 else str(workspace / ".qmdc-semantic" / "hints.json")

from qmdc_semantic.storage import Storage

storage = Storage(workspace)
conn = storage.conn

# Graph object metadata comes from the qmdc Python package in-process
# (no subprocess / native binary). Mirrors qmdc-mkdocs/qmdc_mkdocs/database.py.
from qmdc.db import QmdcDatabase
from qmdc.workspace import parse_workspace

_graph_db = QmdcDatabase()
_graph_db.sync_objects(parse_workspace(str(workspace)).objects)


def graph_query(sql):
    qr = _graph_db.query(sql)
    return [dict(zip(qr.columns, row)) for row in qr.rows]


# Get object labels
labels = {}
for r in graph_query("SELECT __global_id, __label, __id, __file FROM objects WHERE __kind NOT GLOB '__*'"):
    labels[r["__global_id"]] = {"label": r["__label"] or r["__id"], "file": r["__file"]}
# Also include namespaces
for r in graph_query("SELECT __global_id, __label, __id, __file FROM objects WHERE __kind IN ('__Namespace', '__Workspace')"):
    labels[r["__global_id"]] = {"label": r["__label"] or r["__id"], "file": r["__file"]}

# Get explicit edges
explicit = set()
for row in conn.execute("SELECT source_id, target_id FROM edges").fetchall():
    explicit.add(f"{row[0]}→{row[1]}")
    explicit.add(f"{row[1]}→{row[0]}")

# Get all chunks with embeddings
vec_tables = [r[0] for r in conn.execute(
    "SELECT name FROM sqlite_master WHERE type='table' AND name LIKE 'vec_chunks_%'").fetchall()]
if not vec_tables:
    print("No vec tables")
    sys.exit(1)
vec_table = vec_tables[0]

print("Loading chunks...")
chunks = conn.execute("""
    SELECT c.chunk_id, c.object_id, c.object_kind, c.source_file, c.chunk_type
    FROM chunks c WHERE c.model_id IS NOT NULL
""").fetchall()

# Load embeddings
chunk_emb = {}
for chunk_id, object_id, kind, source_file, chunk_type in chunks:
    if source_file and source_file.startswith("tracking/"):
        continue
    row = conn.execute(f"SELECT embedding FROM {vec_table} WHERE chunk_id = ?", (chunk_id,)).fetchone()
    if not row:
        continue
    emb = np.frombuffer(row[0], dtype=np.float32)
    chunk_emb[chunk_id] = {
        "emb": emb,
        "object_id": object_id,
        "kind": kind or "",
        "file": source_file or "",
        "type": chunk_type or "",
    }

print(f"Loaded {len(chunk_emb)} chunk embeddings")

# For each child chunk (field-level), find top similar chunks in other files
hints = {}  # keyed by "object_global_id" or "field:field_id@file"

for chunk_id, info in chunk_emb.items():
    if info["kind"].startswith("__"):
        continue

    # KNN via vec0
    similar = storage.knn_search(info["emb"], k=20)

    for sim_chunk_id, distance in similar:
        sim = 1.0 - distance
        if sim < 0.65:
            continue

        sim_info = chunk_emb.get(sim_chunk_id)
        if not sim_info:
            continue
        if sim_info["file"] == info["file"]:
            continue
        if sim_info["kind"].startswith("__") and sim_info["kind"] not in ("__Namespace", "__Workspace"):
            continue

        # Skip if explicit edge exists between the objects
        if f"{info['object_id']}→{sim_info['object_id']}" in explicit:
            continue

        sim_label_info = labels.get(sim_info["object_id"], {})
        sim_label = sim_label_info.get("label", sim_info["object_id"].split(":")[-1])
        sim_file = sim_label_info.get("file", sim_info["file"])

        # Determine hint key
        if info["type"] == "child":
            # Field-level: extract field name from chunk_id
            parts = chunk_id.split(":")
            # chunk_id is like "ws:ns:obj:field" or "ws:ns:obj@file:field"
            field_name = parts[-1] if len(parts) > 3 else None
            if field_name:
                key = f"field:{field_name}@{info['file']}"
            else:
                key = info["object_id"]
        else:
            key = info["object_id"]

        if key not in hints:
            hints[key] = []
        # Deduplicate by label
        if not any(h["label"] == sim_label for h in hints[key]):
            sim_kind = sim_info["kind"]
            if sim_kind.startswith("__"):
                sim_kind = sim_kind[2:]  # __Namespace -> Namespace
            # Extract local __id from global_id (last component after last colon)
            sim_id = sim_info["object_id"].split(":")[-1]
            hints[key].append({"label": sim_label, "kind": sim_kind, "file": sim_file, "score": round(sim, 3), "id": sim_id})

# Sort and limit
for key in hints:
    hints[key].sort(key=lambda h: -h["score"])
    hints[key] = hints[key][:3]

# Write output
Path(output).parent.mkdir(parents=True, exist_ok=True)
with open(output, "w") as f:
    json.dump(hints, f, indent=2, ensure_ascii=False)

print(f"Wrote {len(hints)} hint entries to {output}")
storage.close()
