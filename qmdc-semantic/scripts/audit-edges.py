#!/usr/bin/env python3
"""
Audit explicit edges against semantic similarity.

Finds:
1. Suspicious edges — explicit link exists but content is semantically distant
2. Missing edges — content is semantically close but no explicit link

Usage:
    uv run python3 scripts/audit-edges.py <workspace>
    uv run python3 scripts/audit-edges.py ../docs
    uv run python3 scripts/audit-edges.py ../docs --threshold 0.3 --top 20
    uv run python3 scripts/audit-edges.py ../docs --exclude tracking/,ideas/
    uv run python3 scripts/audit-edges.py ../docs --exclude ""  # include everything
"""

import sqlite3
import sys
from pathlib import Path

import numpy as np

workspace = Path(sys.argv[1]).resolve() if len(sys.argv) > 1 else Path("../docs").resolve()
threshold = float(sys.argv[sys.argv.index("--threshold") + 1]) if "--threshold" in sys.argv else 0.35
top_n = int(sys.argv[sys.argv.index("--top") + 1]) if "--top" in sys.argv else 15
exclude = sys.argv[sys.argv.index("--exclude") + 1].split(",") if "--exclude" in sys.argv else ["tracking/"]


def is_excluded(file_path: str) -> bool:
    return any(file_path.startswith(ex) for ex in exclude)

# ── Load data ───────────────────────────────────────────────────────────────

sem_db = workspace / ".qmdc-semantic" / "embeddings.db"
if not sem_db.exists():
    print(f"No embeddings.db in {workspace}. Run: make semantic-index WS={workspace}")
    sys.exit(1)

# Graph object metadata comes from the qmdc Python package in-process
# (no subprocess / native binary). Mirrors qmdc-mkdocs/qmdc_mkdocs/database.py.
from qmdc.db import QmdcDatabase
from qmdc.workspace import parse_workspace

_graph_db = QmdcDatabase()
_graph_db.sync_objects(parse_workspace(str(workspace)).objects)


def graph_query(sql: str) -> list[dict]:
    qr = _graph_db.query(sql)
    return [dict(zip(qr.columns, row)) for row in qr.rows]


# Connect to semantic DB — use Storage to load sqlite-vec extension
from qmdc_semantic.storage import Storage
storage = Storage(workspace)
conn = storage.conn

# Find the vec table
vec_tables = [r[0] for r in conn.execute(
    "SELECT name FROM sqlite_master WHERE type='table' AND name LIKE 'vec_chunks_%'").fetchall()]
if not vec_tables:
    print("No vector tables found in embeddings.db")
    sys.exit(1)
vec_table = vec_tables[0]


# ── Build object embedding index ────────────────────────────────────────────

# Get all chunks with their object IDs and embeddings
print("Loading embeddings...")
chunks = conn.execute("""
    SELECT c.chunk_id, c.object_id, c.object_kind, c.source_file, c.chunk_type
    FROM chunks c
    WHERE c.model_id IS NOT NULL
""").fetchall()

# Get embeddings from vec table
obj_embeddings: dict[str, np.ndarray] = {}  # object_id -> avg embedding
obj_meta: dict[str, dict] = {}  # object_id -> {kind, file, label}

for chunk_id, object_id, kind, source_file, chunk_type in chunks:
    if kind and kind.startswith("__"):
        continue  # skip system types
    if source_file and is_excluded(source_file):
        continue
    row = conn.execute(f"SELECT embedding FROM {vec_table} WHERE chunk_id = ?",
                       (chunk_id,)).fetchone()
    if not row:
        continue
    emb = np.frombuffer(row[0], dtype=np.float32)
    if object_id not in obj_embeddings:
        obj_embeddings[object_id] = emb
        obj_meta[object_id] = {"kind": kind, "file": source_file}
    else:
        # Average embeddings for objects with multiple chunks
        obj_embeddings[object_id] = (obj_embeddings[object_id] + emb) / 2

print(f"Loaded {len(obj_embeddings)} object embeddings")

# Get object labels from the graph
labels: dict[str, str] = {}
for r in graph_query("SELECT __global_id, __label, __id FROM objects WHERE __kind NOT GLOB '__*'"):
    gid = r["__global_id"]
    labels[gid] = r["__label"] or r["__id"]


def cosine_sim(a: np.ndarray, b: np.ndarray) -> float:
    na, nb = np.linalg.norm(a), np.linalg.norm(b)
    if na == 0 or nb == 0:
        return 0.0
    return float(np.dot(a, b) / (na * nb))


def obj_label(gid: str) -> str:
    return labels.get(gid, gid.split(":")[-1])


# ── Audit 1: Suspicious explicit edges ──────────────────────────────────────
# Edges where the source and target content are semantically distant.

print("\n" + "=" * 70)
print("SUSPICIOUS EDGES — explicit link but content is semantically distant")
print("=" * 70)

explicit_edges = graph_query("""
    SELECT e.source_id, e.target_id, e.edge_type, e.source_field,
           s.__label as source_label, s.__kind as source_kind, s.__file as source_file,
           t.__label as target_label, t.__kind as target_kind, t.__file as target_file
    FROM edges e
    JOIN objects s ON e.source_id = s.__global_id
    JOIN objects t ON e.target_id = t.__global_id
    WHERE s.__kind NOT GLOB '__*' AND t.__kind NOT GLOB '__*'
    ORDER BY e.source_id
""")

suspicious = []
for e in explicit_edges:
    if is_excluded(e["source_file"] or "") or is_excluded(e["target_file"] or ""):
        continue
    src_gid = e["source_id"]
    tgt_gid = e["target_id"]
    if src_gid not in obj_embeddings or tgt_gid not in obj_embeddings:
        continue
    sim = cosine_sim(obj_embeddings[src_gid], obj_embeddings[tgt_gid])
    if sim < threshold:
        suspicious.append({
            "source": obj_label(src_gid),
            "source_kind": e["source_kind"],
            "source_file": e["source_file"],
            "target": obj_label(tgt_gid),
            "target_kind": e["target_kind"],
            "target_file": e["target_file"],
            "edge_type": e["edge_type"],
            "similarity": sim,
        })

suspicious.sort(key=lambda x: x["similarity"])

if suspicious:
    print(f"\nFound {len(suspicious)} edges with similarity < {threshold}:\n")
    for i, s in enumerate(suspicious[:top_n], 1):
        print(f"  {i}. {s['source']} ({s['source_kind']})")
        print(f"     --[{s['edge_type']}]--> {s['target']} ({s['target_kind']})")
        print(f"     similarity: {s['similarity']:.3f}  (low = semantically distant)")
        print(f"     {s['source_file']} → {s['target_file']}")
        print()
    if len(suspicious) > top_n:
        print(f"  ... and {len(suspicious) - top_n} more")
else:
    print(f"\n  ✅ All explicit edges have similarity >= {threshold}")


# ── Audit 2: Missing edges ──────────────────────────────────────────────────
# Objects that are semantically very similar but have no explicit edge.

print("\n" + "=" * 70)
print("POTENTIAL MISSING EDGES — semantically close but no explicit link")
print("=" * 70)

# Build set of existing edges for fast lookup
existing_edges = set()
for e in explicit_edges:
    existing_edges.add((e["source_id"], e["target_id"]))
    existing_edges.add((e["target_id"], e["source_id"]))  # bidirectional check

# Use inferred edges from semantic DB (already computed by qmdc-semantic index)
inferred = conn.execute("""
    SELECT source_id, target_id, similarity
    FROM inferred_edges
    WHERE similarity > 0.7
    ORDER BY similarity DESC
""").fetchall()

missing = []
for src_gid, tgt_gid, sim in inferred:
    if (src_gid, tgt_gid) in existing_edges:
        continue
    # Skip if same file
    src_meta = obj_meta.get(src_gid, {})
    tgt_meta = obj_meta.get(tgt_gid, {})
    if src_meta.get("file") == tgt_meta.get("file"):
        continue
    if is_excluded(src_meta.get("file", "")) or is_excluded(tgt_meta.get("file", "")):
        continue
    missing.append({
        "source": obj_label(src_gid),
        "source_kind": src_meta.get("kind", "?"),
        "source_file": src_meta.get("file", "?"),
        "target": obj_label(tgt_gid),
        "target_kind": tgt_meta.get("kind", "?"),
        "target_file": tgt_meta.get("file", "?"),
        "similarity": sim,
    })

if missing:
    print(f"\nFound {len(missing)} potential missing edges (similarity > 0.7):\n")
    for i, m in enumerate(missing[:top_n], 1):
        print(f"  {i}. {m['source']} ({m['source_kind']})")
        print(f"     ~~~ {m['target']} ({m['target_kind']})")
        print(f"     similarity: {m['similarity']:.3f}  (high = semantically close)")
        print(f"     {m['source_file']} ↔ {m['target_file']}")
        print()
    if len(missing) > top_n:
        print(f"  ... and {len(missing) - top_n} more")
else:
    print(f"\n  ✅ No high-similarity pairs without explicit edges")

# ── Summary ─────────────────────────────────────────────────────────────────

print("\n" + "=" * 70)
print(f"SUMMARY: {len(suspicious)} suspicious edges, {len(missing)} potential missing edges")
print("=" * 70)

storage.close()
