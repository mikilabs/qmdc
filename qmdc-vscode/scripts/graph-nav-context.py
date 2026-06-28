#!/usr/bin/env python3
"""
Graph navigation context for a QMD.md file — machine-readable JSON.

Outputs everything a renderer needs to build navigation for a page:
breadcrumb, siblings, outgoing/incoming links, related files, site tree.

Usage:
    uv run python3 scripts/graph-nav-context.py <workspace> <file>
    uv run python3 scripts/graph-nav-context.py <workspace> <file> --tree
    uv run python3 scripts/graph-nav-context.py <workspace> --tree-only

Examples:
    uv run python3 scripts/graph-nav-context.py ./docs lsp/diagnostics.qmd.md
    uv run python3 scripts/graph-nav-context.py ./docs lsp/diagnostics.qmd.md --tree
    uv run python3 scripts/graph-nav-context.py ./docs --tree-only
"""

import json
import subprocess
import sys
from pathlib import Path

# ── Data layer ──────────────────────────────────────────────────────────────

QMDC: str | None = None


def find_qmdc() -> str:
    global QMDC
    if QMDC:
        return QMDC
    for sub in ("target/release/qmdc", "target/debug/qmdc"):
        p = Path(__file__).resolve().parent.parent.parent / "qmdc-rs" / sub
        if p.exists():
            QMDC = str(p)
            return QMDC
    print("ERROR: qmdc not found", file=sys.stderr)
    sys.exit(1)


def q(workspace: str, sql: str) -> list[dict]:
    result = subprocess.run(
        [find_qmdc(), "query", workspace, sql, "--format", "json"],
        capture_output=True, text=True,
    )
    if result.returncode != 0:
        return []
    try:
        data = json.loads(result.stdout) if result.stdout.strip() else {}
        cols = data.get("columns", [])
        return [dict(zip(cols, row)) for row in data.get("rows", [])]
    except json.JSONDecodeError:
        return []


# ── Label resolution ────────────────────────────────────────────────────────

def get_workspace_info(ws: str) -> dict:
    rows = q(ws, "SELECT __id, __label FROM objects WHERE __kind = '__Workspace' LIMIT 1")
    if rows:
        return {"id": rows[0]["__id"], "label": rows[0]["__label"]}
    return {"id": Path(ws).name, "label": Path(ws).name}


def get_namespace_labels(ws: str) -> dict[str, str]:
    return {r["__id"]: r["__label"]
            for r in q(ws, "SELECT __id, __label FROM objects WHERE __kind = '__Namespace'")}


def get_file_labels(ws: str) -> dict[str, str]:
    """Level-1 business object label per file."""
    return {r["__file"]: r["__label"]
            for r in q(ws, """
                SELECT __file, __label FROM objects
                WHERE __level = 1 AND __kind NOT GLOB '__*' AND __label IS NOT NULL
            """)}


def file_display_name(f: str, labels: dict[str, str]) -> str:
    if f in labels:
        return labels[f]
    stem = Path(f).stem
    if stem.endswith(".qmd"):
        stem = stem[:-4]
    if stem.lower() == "readme":
        return "Overview"
    return stem.replace("-", " ").replace("_", " ").title()


def get_all_files(ws: str) -> list[str]:
    return [r["__file"] for r in q(ws, "SELECT DISTINCT __file FROM objects ORDER BY __file")]


# ── File context ────────────────────────────────────────────────────────────

def build_file_context(ws: str, fp: str) -> dict:
    """Full navigation context for a single file."""
    ws_info = get_workspace_info(ws)
    ns_labels = get_namespace_labels(ws)
    fl = get_file_labels(ws)

    # Namespace
    ns_rows = q(ws, f"SELECT __namespace FROM objects WHERE __file = '{fp}' LIMIT 1")
    ns_id = ns_rows[0]["__namespace"] if ns_rows and ns_rows[0].get("__namespace") else None
    ns_label = ns_labels.get(ns_id, ns_id) if ns_id else None

    # Breadcrumb
    breadcrumb = [{"id": ws_info["id"], "label": ws_info["label"], "type": "workspace"}]
    if ns_id:
        breadcrumb.append({"id": ns_id, "label": ns_label, "type": "namespace"})
    breadcrumb.append({
        "file": fp,
        "label": file_display_name(fp, fl),
        "type": "file",
    })

    # Objects in this file
    objects_in_file = q(ws, f"""
        SELECT __id, __label, __kind, __level, __line FROM objects
        WHERE __file = '{fp}' AND __kind NOT GLOB '__*'
        ORDER BY __line
    """)

    # Siblings
    ns_filter = f"__namespace = '{ns_id}'" if ns_id else "(__namespace IS NULL OR __namespace = '')"
    sib_rows = q(ws, f"""
        SELECT __file, __kind, COUNT(*) as count FROM objects
        WHERE {ns_filter} AND __kind NOT GLOB '__*'
        GROUP BY __file, __kind ORDER BY __file, __kind
    """)
    sib_files: dict[str, list[dict]] = {}
    for r in sib_rows:
        sib_files.setdefault(r["__file"], []).append(
            {"kind": r["__kind"], "count": int(r["count"])})
    siblings = [
        {"file": f, "label": file_display_name(f, fl), "kinds": kinds, "current": f == fp}
        for f, kinds in sorted(sib_files.items())
    ]

    # Outgoing edges
    outgoing = q(ws, f"""
        SELECT DISTINCT e.edge_type, t.__id as id, t.__label as label,
               t.__kind as kind, t.__file as file
        FROM edges e
        JOIN objects s ON e.source_id = s.__global_id
        JOIN objects t ON e.target_id = t.__global_id
        WHERE s.__file = '{fp}' AND t.__file != '{fp}' AND t.__kind NOT GLOB '__*'
        ORDER BY e.edge_type, t.__file
    """)
    links_to: list[dict] = []
    for e in outgoing:
        links_to.append({
            "edge_type": e["edge_type"],
            "target_id": e["id"],
            "target_label": e["label"] or e["id"],
            "target_kind": e["kind"],
            "target_file": e["file"],
            "target_file_label": file_display_name(e["file"], fl),
        })

    # Incoming edges
    incoming = q(ws, f"""
        SELECT DISTINCT s.__id as id, s.__label as label,
               s.__kind as kind, s.__file as file, e.edge_type
        FROM edges e
        JOIN objects s ON e.source_id = s.__global_id
        JOIN objects t ON e.target_id = t.__global_id
        WHERE t.__file = '{fp}' AND s.__file != '{fp}' AND s.__kind NOT GLOB '__*'
        ORDER BY e.edge_type, s.__file
    """)
    linked_from: list[dict] = []
    for e in incoming:
        linked_from.append({
            "edge_type": e["edge_type"],
            "source_id": e["id"],
            "source_label": e["label"] or e["id"],
            "source_kind": e["kind"],
            "source_file": e["file"],
            "source_file_label": file_display_name(e["file"], fl),
        })

    # Related files (deduplicated union of outgoing + incoming files)
    related_map: dict[str, dict] = {}
    for e in links_to:
        f = e["target_file"]
        if f not in related_map:
            related_map[f] = {"file": f, "label": e["target_file_label"], "edge_types": set()}
        related_map[f]["edge_types"].add(e["edge_type"])
    for e in linked_from:
        f = e["source_file"]
        if f not in related_map:
            related_map[f] = {"file": f, "label": e["source_file_label"], "edge_types": set()}
        related_map[f]["edge_types"].add(e["edge_type"])
    related = [
        {"file": v["file"], "label": v["label"], "edge_types": sorted(v["edge_types"])}
        for v in sorted(related_map.values(), key=lambda x: x["file"])
    ]

    return {
        "workspace": ws_info,
        "file": fp,
        "file_label": file_display_name(fp, fl),
        "namespace": {"id": ns_id, "label": ns_label} if ns_id else None,
        "breadcrumb": breadcrumb,
        "objects": objects_in_file,
        "siblings": siblings,
        "links_to": links_to,
        "linked_from": linked_from,
        "related_files": related,
    }


# ── Site tree ───────────────────────────────────────────────────────────────

def build_site_tree(ws: str) -> dict:
    """Full site navigation tree — namespace hierarchy with file labels."""
    ws_info = get_workspace_info(ws)
    ns_labels = get_namespace_labels(ws)
    fl = get_file_labels(ws)
    all_files = get_all_files(ws)

    # Files with business objects (for filtering empty readmes)
    content_files = set(r["__file"] for r in q(ws,
        "SELECT DISTINCT __file FROM objects WHERE __kind NOT GLOB '__*'"))

    def is_empty_readme(f: str) -> bool:
        return Path(f).name == "readme.qmd.md" and f not in content_files

    # Group by directory
    dirs: dict[str, list[str]] = {}
    for f in all_files:
        dirs.setdefault(str(Path(f).parent), []).append(f)

    sections: list[dict] = []

    # Root files
    root_files = []
    for f in dirs.get(".", []):
        if is_empty_readme(f):
            continue
        root_files.append({"file": f, "label": file_display_name(f, fl)})
    if root_files:
        sections.append({"label": ws_info["label"], "type": "root", "files": root_files})

    # Namespace sections
    # Pre-scan: which 2-part dirs have 3+ part children? Those become groups.
    has_children: set[str] = set()
    for d in dirs:
        parts = d.split("/")
        if len(parts) >= 3:
            has_children.add("/".join(parts[:2]))

    emitted: set[str] = set()
    for d in sorted(k for k in dirs if k != "."):
        ns_id = d.split("/")[0]
        ns_label = ns_labels.get(ns_id, ns_id.title())
        parts = d.split("/")

        if len(parts) >= 3 or (len(parts) == 2 and d in has_children):
            # Group: 2-part dir with children, or 3+ part dir
            group_key = "/".join(parts[:2]) if len(parts) >= 3 else d
            if group_key not in emitted:
                emitted.add(group_key)
                group_files = []
                # Direct files in the 2-level dir
                if group_key in dirs:
                    for f in dirs[group_key]:
                        if is_empty_readme(f):
                            continue
                        group_files.append({"file": f, "label": file_display_name(f, fl)})
                # Files from 3+ level subdirs
                for gd in sorted(k for k in dirs if k.startswith(group_key + "/") and k != group_key):
                    sub_prefix = gd[len(group_key) + 1:]
                    for f in dirs[gd]:
                        label = file_display_name(f, fl)
                        if sub_prefix:
                            label = f"{sub_prefix}: {label}"
                        group_files.append({"file": f, "label": label})
                gk_parts = group_key.split("/")
                sections.append({
                    "label": f"{ns_label} › {gk_parts[1]}" if len(gk_parts) > 1 else ns_label,
                    "namespace": ns_id,
                    "type": "group",
                    "files": group_files,
                })
        elif len(parts) == 2:
            if d not in emitted:
                emitted.add(d)
                sec_files = []
                for f in dirs[d]:
                    if is_empty_readme(f):
                        continue
                    sec_files.append({"file": f, "label": file_display_name(f, fl)})
                sections.append({
                    "label": f"{ns_label} › {parts[1]}",
                    "namespace": ns_id,
                    "type": "subsection",
                    "files": sec_files,
                })
        else:
            files = []
            for f in dirs[d]:
                if is_empty_readme(f):
                    continue
                files.append({"file": f, "label": file_display_name(f, fl)})
            sections.append({
                "label": ns_label,
                "namespace": ns_id,
                "type": "namespace",
                "files": files,
            })

    return {"workspace": ws_info, "sections": sections}


# ── CLI ─────────────────────────────────────────────────────────────────────

def main():
    args = sys.argv[1:]
    if not args:
        print(__doc__)
        sys.exit(1)

    workspace = args[0]
    include_tree = "--tree" in args
    tree_only = "--tree-only" in args
    file_path = None
    for a in args[1:]:
        if not a.startswith("-"):
            file_path = a
            break

    result: dict = {}

    if tree_only:
        result = build_site_tree(workspace)
    elif file_path:
        result = build_file_context(workspace, file_path)
        if include_tree:
            result["site_tree"] = build_site_tree(workspace)
    else:
        print("Error: provide a file path or use --tree-only", file=sys.stderr)
        sys.exit(1)

    json.dump(result, sys.stdout, indent=2, ensure_ascii=False)
    print()


if __name__ == "__main__":
    main()
