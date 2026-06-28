#!/usr/bin/env python3
"""Debug script to compare edge extraction across parsers."""

from collections import Counter
from pathlib import Path

from qmdc.db import QmdcDatabase
from qmdc.workspace import parse_workspace


def find_project_root() -> Path:
    """Find project root by looking for the qmdc-rs/ package directory."""
    current = Path(__file__).resolve().parent
    while current != current.parent:
        if (current / "qmdc-rs").exists():
            return current
        current = current.parent
    raise RuntimeError("Could not find project root")


def main():
    root = find_project_root()
    ws_path = root / "tests/workspace/test-workspace"

    print(f"Workspace path: {ws_path}")
    print(f"Exists: {ws_path.exists()}\n")

    result = parse_workspace(str(ws_path))

    print(f"Workspace ID: {result.workspace_id}")
    print(f"Total objects: {len(result.objects)}\n")

    # Count by kind
    kinds = Counter(obj.get("__kind", "unknown") for obj in result.objects)
    print("Objects by kind:")
    for kind, count in sorted(kinds.items()):
        print(f"  {kind}: {count}")
    print()

    # Check for specific objects
    services = next((o for o in result.objects if o.get("__id") == "services"), None)
    doc = next((o for o in result.objects if o.get("__id") == "doc_ry4ljv"), None)
    text_blocks = [o for o in result.objects if o.get("__kind") == "__TextBlock"]

    print(f"services object: {'found' if services else 'NOT FOUND'}")
    if services:
        print(f"  fields: {[k for k in services if not k.startswith('__')]}")

    print(f"doc_ry4ljv object: {'found' if doc else 'NOT FOUND'}")
    if doc:
        print(f"  fields: {[k for k in doc if not k.startswith('__')]}")

    print(f"__TextBlock objects: {len(text_blocks)}")
    for tb in text_blocks[:5]:
        print(f"  {tb.get('__id')}")
    print()

    # Sync to DB and check edges
    db = QmdcDatabase()
    db.sync_objects(result.objects)

    total = db.query("SELECT COUNT(*) FROM edges")
    print(f"Total edges: {total.rows[0][0]}")

    # Edges by source
    by_source = db.query(
        "SELECT source_id, COUNT(*) as cnt FROM edges GROUP BY source_id ORDER BY cnt DESC LIMIT 10"
    )
    print("\nTop 10 sources by edge count:")
    for row in by_source.rows:
        print(f"  {row[0]}: {row[1]} edges")

    # Check text_* edges
    text_edges = db.query('SELECT COUNT(*) FROM edges WHERE source_id LIKE "text_%"')
    print(f"\nEdges from text_*: {text_edges.rows[0][0]}")

    text_edge_details = db.query(
        "SELECT source_id, source_field, target_id FROM edges "
        'WHERE source_id LIKE "text_%" ORDER BY source_id'
    )
    print("Details:")
    for row in text_edge_details.rows:
        print(f"  {row[0]}|{row[1]}|{row[2]}")

    # Check services edges
    services_edges = db.query(
        "SELECT source_id, source_field, target_id FROM edges "
        'WHERE source_id = "services" ORDER BY target_id'
    )
    print(f"\nEdges from services: {len(services_edges.rows)}")
    for row in services_edges.rows[:10]:
        print(f"  {row[0]}|{row[1]}|{row[2]}")

    # Check doc_ry4ljv edges
    doc_edges = db.query(
        "SELECT source_id, source_field, target_id FROM edges "
        'WHERE source_id = "doc_ry4ljv" ORDER BY target_id'
    )
    print(f"\nEdges from doc_ry4ljv: {len(doc_edges.rows)}")
    for row in doc_edges.rows[:10]:
        print(f"  {row[0]}|{row[1]}|{row[2]}")


if __name__ == "__main__":
    main()
