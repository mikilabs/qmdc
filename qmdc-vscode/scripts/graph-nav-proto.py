#!/usr/bin/env python3
"""
Interactive TUI: graph-aware navigation for QMDC workspaces.

Usage:
    uv run python3 scripts/graph-nav-proto.py <workspace_path> [file_path]

Controls:
    Tab           — switch focus between tree and context panel
    ↑/↓  or j/k  — move cursor
    Enter         — navigate to file (from either pane)
    Backspace     — go back in history
    q/Esc         — quit
"""

import curses
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
    print("ERROR: qmdc not found. Run: cd qmdc-rs && cargo build --release",
          file=sys.stderr)
    sys.exit(1)


def qmdc_query(workspace: str, sql: str) -> list[dict]:
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


def get_workspace_name(ws: str) -> str:
    rows = qmdc_query(ws, "SELECT __label FROM objects WHERE __kind = '__Workspace' LIMIT 1")
    return rows[0]["__label"] if rows else Path(ws).name


def get_namespace_for_file(ws: str, fp: str) -> str | None:
    rows = qmdc_query(ws, f"SELECT __namespace FROM objects WHERE __file = '{fp}' LIMIT 1")
    if rows and rows[0].get("__namespace"):
        return rows[0]["__namespace"]
    return None


def get_sibling_files(ws: str, namespace: str | None, current: str) -> list[dict]:
    nf = f"__namespace = '{namespace}'" if namespace else "(__namespace IS NULL OR __namespace = '')"
    rows = qmdc_query(ws, f"""
        SELECT __file, __kind, COUNT(*) as cnt FROM objects
        WHERE {nf} AND __kind NOT GLOB '__*'
        GROUP BY __file, __kind ORDER BY __file, __kind
    """)
    files: dict[str, list[tuple[str, int]]] = {}
    for r in rows:
        files.setdefault(r["__file"], []).append((r["__kind"], int(r["cnt"])))
    return [{"file": f, "kinds": k, "is_current": f == current}
            for f, k in sorted(files.items())]


def get_outgoing_edges(ws: str, fp: str) -> list[dict]:
    return qmdc_query(ws, f"""
        SELECT DISTINCT e.edge_type, t.__id as target, t.__label as target_label,
               t.__kind as target_kind, t.__file as target_file
        FROM edges e
        JOIN objects s ON e.source_id = s.__global_id
        JOIN objects t ON e.target_id = t.__global_id
        WHERE s.__file = '{fp}' AND t.__file != '{fp}' AND t.__kind NOT GLOB '__*'
        ORDER BY e.edge_type, t.__file
    """)


def get_incoming_edges(ws: str, fp: str) -> list[dict]:
    return qmdc_query(ws, f"""
        SELECT DISTINCT s.__id as source, s.__label as source_label,
               s.__kind as source_kind, s.__file as source_file, e.edge_type
        FROM edges e
        JOIN objects s ON e.source_id = s.__global_id
        JOIN objects t ON e.target_id = t.__global_id
        WHERE t.__file = '{fp}' AND s.__file != '{fp}' AND s.__kind NOT GLOB '__*'
        ORDER BY e.edge_type, s.__file
    """)


def get_all_files(ws: str) -> list[str]:
    rows = qmdc_query(ws, "SELECT DISTINCT __file FROM objects ORDER BY __file")
    return [r["__file"] for r in rows]


# ── Human-readable verbs ────────────────────────────────────────────────────

OUTGOING_VERB: dict[str, str] = {
    "depends": "depends on", "validates": "validates", "about": "describes",
    "affects": "affects", "uses": "uses", "implements": "implements",
    "extends": "extends", "contains": "contains", "references": "references",
    "content": "includes", "features": "provides", "description": "describes",
}
INCOMING_VERB: dict[str, str] = {
    "depends": "needed by", "validates": "validated by", "about": "described in",
    "affects": "affected by", "uses": "used by", "implements": "implemented by",
    "extends": "extended by", "contains": "part of", "references": "referenced in",
    "content": "included in", "features": "feature of", "description": "described in",
}


def _verb(edge_type: str, direction: str) -> str:
    return (OUTGOING_VERB if direction == "out" else INCOMING_VERB).get(edge_type, edge_type)


def _short_file(fp: str, cur_dir: str) -> str:
    d = str(Path(fp).parent)
    n = Path(fp).name
    return f"{d}/{n}" if d != cur_dir and d != "." else n


# ── Tree builder ────────────────────────────────────────────────────────────

class TreeNode:
    """A line in the site nav tree. Navigable if file is set."""
    def __init__(self, label: str, file: str | None = None, indent: int = 0,
                 is_section: bool = False):
        self.label = label
        self.file = file
        self.indent = indent
        self.is_section = is_section


def _get_namespace_labels(ws: str) -> dict[str, str]:
    """Map namespace id -> human label (e.g. 'lsp' -> 'LSP Server')."""
    rows = qmdc_query(ws, """
        SELECT __id, __label FROM objects
        WHERE __kind = '__Namespace' ORDER BY __id
    """)
    return {r["__id"]: r["__label"] for r in rows}


def _get_file_labels(ws: str) -> dict[str, str]:
    """Map file path -> best human label for the file.

    Priority:
    1. Level-1 business object label (the h1 with [[id]])
    2. Cleaned-up filename stem (not in this dict → caller handles it)

    We intentionally skip "first business object" as a fallback because
    it's often a child object (e.g. the first command in a commands file),
    not the file's topic.
    """
    rows = qmdc_query(ws, """
        SELECT __file, __label FROM objects
        WHERE __level = 1 AND __kind NOT GLOB '__*' AND __label IS NOT NULL
        ORDER BY __file
    """)
    return {r["__file"]: r["__label"] for r in rows}


def _file_display_name(f: str, file_labels: dict[str, str]) -> str:
    """Human label for a file, falling back to a cleaned-up filename."""
    if f in file_labels:
        return file_labels[f]
    stem = Path(f).stem
    if stem.endswith(".qmd"):
        stem = stem[:-4]
    # "readme" → "Overview" (these are namespace index pages with content)
    if stem.lower() == "readme":
        return "Overview"
    return stem.replace("-", " ").replace("_", " ").title()


def build_file_tree(ws: str, ws_name: str) -> list[TreeNode]:
    """Build a site-nav tree using human-readable labels from the graph."""
    all_files = get_all_files(ws)
    ns_labels = _get_namespace_labels(ws)
    file_labels = _get_file_labels(ws)

    # Group files by namespace directory
    dirs: dict[str, list[str]] = {}
    for f in all_files:
        d = str(Path(f).parent)
        dirs.setdefault(d, []).append(f)

    # Readme files that have real business objects should be shown;
    # those that are purely namespace declarations (no business objects) are skipped.
    files_with_content = set(qmdc_query(ws, """
        SELECT DISTINCT __file FROM objects WHERE __kind NOT GLOB '__*'
    """) and [r["__file"] for r in qmdc_query(ws, """
        SELECT DISTINCT __file FROM objects WHERE __kind NOT GLOB '__*'
    """)])

    def _is_empty_readme(f: str) -> bool:
        return Path(f).name == "readme.qmd.md" and f not in files_with_content

    nodes: list[TreeNode] = []
    nodes.append(TreeNode(ws_name, indent=0, is_section=True))

    # Root files (skip workspace readme only if it has no content)
    for f in dirs.get(".", []):
        if _is_empty_readme(f):
            continue
        label = _file_display_name(f, file_labels)
        nodes.append(TreeNode(label, file=f, indent=1))

    # Namespace sections — use the namespace label, not the directory name
    # Dirs with 3+ parts (tracking/done/QMD-10) get grouped under their 2-part prefix.
    # Dirs with 2 parts (releases/v1) become sections directly.
    # We track all emitted section keys to avoid duplicates.
    emitted_sections: set[str] = set()
    for d in sorted(k for k in dirs if k != "."):
        ns_id = d.split("/")[0]
        section_label = ns_labels.get(ns_id, ns_id.title())
        parts = d.split("/")

        if len(parts) >= 3:
            # Deep nesting — group under 2-part prefix
            group_key = "/".join(parts[:2])
            if group_key not in emitted_sections:
                emitted_sections.add(group_key)
                group_label = f"{section_label} › {parts[1]}"
                nodes.append(TreeNode(group_label, indent=1, is_section=True))
            # Prefix with the 3rd-level dir name (task id, subfolder, etc.)
            sub_prefix = "/".join(parts[2:])
            for f in dirs[d]:
                label = _file_display_name(f, file_labels)
                display = f"{sub_prefix}: {label}" if sub_prefix else label
                nodes.append(TreeNode(display, file=f, indent=2))
        elif len(parts) == 2:
            # 2-level dir like releases/v1 — becomes a section
            sub = parts[1]
            full_label = f"{section_label} › {sub}"
            section_key = d  # "releases/v1"
            if section_key not in emitted_sections:
                emitted_sections.add(section_key)
                nodes.append(TreeNode(full_label, indent=1, is_section=True))
            for f in dirs[d]:
                if _is_empty_readme(f):
                    continue
                label = _file_display_name(f, file_labels)
                nodes.append(TreeNode(label, file=f, indent=2))
        else:
            # Top-level namespace dir
            nodes.append(TreeNode(section_label, indent=1, is_section=True))
            for f in dirs[d]:
                if _is_empty_readme(f):
                    continue
                label = _file_display_name(f, file_labels)
                nodes.append(TreeNode(label, file=f, indent=2))

    return nodes


# ── Context panel builder ───────────────────────────────────────────────────

class NavItem:
    def __init__(self, text: str, target_file: str | None = None, style: str = "normal"):
        self.text = text
        self.target_file = target_file
        self.style = style  # normal, header, dim, current, separator


def build_context(ws: str, fp: str, ws_name: str) -> list[NavItem]:
    items: list[NavItem] = []
    namespace = get_namespace_for_file(ws, fp)
    cur_dir = str(Path(fp).parent)
    file_labels = _get_file_labels(ws)

    # Breadcrumb — use human label for the file
    parts = [ws_name]
    if namespace:
        ns_labels = _get_namespace_labels(ws)
        parts.append(ns_labels.get(namespace, namespace))
    parts.append(_file_display_name(fp, file_labels))
    items.append(NavItem("📍 " + " › ".join(parts), style="header"))
    items.append(NavItem(""))

    # Siblings — use human labels, not filenames
    siblings = get_sibling_files(ws, namespace, fp)
    if siblings:
        ns_labels = _get_namespace_labels(ws)
        section = ns_labels.get(namespace, namespace) if namespace else ws_name
        items.append(NavItem(section, style="header"))
        items.append(NavItem("─" * 48, style="separator"))
        for s in siblings:
            label = _file_display_name(s["file"], file_labels)
            kinds = ", ".join(f"{c} {k}" for k, c in s["kinds"])
            if s["is_current"]:
                items.append(NavItem(f" ▸ {label}  ({kinds})", style="current"))
            else:
                items.append(NavItem(f"   {label}  ({kinds})", target_file=s["file"]))
        items.append(NavItem(""))

    # Outgoing — grouped by verb, show object label not ID
    outgoing = get_outgoing_edges(ws, fp)
    if outgoing:
        items.append(NavItem("This file links to", style="header"))
        items.append(NavItem("─" * 48, style="separator"))
        by_t: dict[str, list[dict]] = {}
        for e in outgoing:
            by_t.setdefault(e["edge_type"], []).append(e)
        for et, edges in sorted(by_t.items()):
            items.append(NavItem(f"  {_verb(et, 'out')}:", style="dim"))
            for e in edges:
                name = e.get("target_label") or e["target"]
                items.append(NavItem(
                    f"    {name}  ({e['target_kind']})",
                    target_file=e["target_file"]))
        items.append(NavItem(""))

    # Incoming — grouped by verb, show object label not ID
    incoming = get_incoming_edges(ws, fp)
    if incoming:
        items.append(NavItem("Linked from", style="header"))
        items.append(NavItem("─" * 48, style="separator"))
        by_t: dict[str, list[dict]] = {}
        for e in incoming:
            by_t.setdefault(e["edge_type"], []).append(e)
        for et, edges in sorted(by_t.items()):
            items.append(NavItem(f"  {_verb(et, 'in')}:", style="dim"))
            for e in edges:
                name = e.get("source_label") or e["source"]
                items.append(NavItem(
                    f"    {name}  ({e['source_kind']})",
                    target_file=e["source_file"]))
        items.append(NavItem(""))

    # Related files
    related: dict[str, list[str]] = {}
    for e in outgoing:
        if e["target_file"] != fp:
            name = e.get("target_label") or e["target"]
            related.setdefault(e["target_file"], []).append(
                f"{_verb(e['edge_type'], 'out')} {name}")
    for e in incoming:
        if e["source_file"] != fp:
            name = e.get("source_label") or e["source"]
            related.setdefault(e["source_file"], []).append(
                f"{_verb(e['edge_type'], 'in')} {name}")
    if related:
        items.append(NavItem("Related files", style="header"))
        items.append(NavItem("─" * 48, style="separator"))
        for f, reasons in sorted(related.items()):
            uniq = list(dict.fromkeys(reasons))
            rs = ", ".join(uniq[:2])
            if len(uniq) > 2:
                rs += f" +{len(uniq)-2} more"
            items.append(NavItem(f"  ◇ {_file_display_name(f, file_labels)}  ({rs})",
                                 target_file=f))

    return items


# ── Two-pane curses TUI ────────────────────────────────────────────────────

FOCUS_TREE = 0
FOCUS_CTX = 1


def _attr_for(item_style: str, is_selected: bool, has_target: bool,
              is_active_file: bool, pairs: dict[str, int]) -> int:
    if is_selected:
        return curses.color_pair(pairs["sel"]) | curses.A_BOLD
    if is_active_file:
        return curses.color_pair(pairs["cur"]) | curses.A_BOLD
    if item_style == "header":
        return curses.color_pair(pairs["hdr"]) | curses.A_BOLD
    if item_style == "current":
        return curses.color_pair(pairs["cur"]) | curses.A_BOLD
    if item_style == "separator":
        return curses.color_pair(pairs["dim"])
    if item_style == "dim":
        return curses.color_pair(pairs["verb"])
    if has_target:
        return curses.color_pair(pairs["link"])
    return curses.A_NORMAL


def run_tui(stdscr: curses.window, workspace: str, start_file: str):
    curses.curs_set(0)
    curses.use_default_colors()

    # Color pairs
    curses.init_pair(1, curses.COLOR_BLUE, -1)                    # header
    curses.init_pair(2, curses.COLOR_CYAN, -1)                    # link
    curses.init_pair(3, curses.COLOR_GREEN, -1)                   # current
    curses.init_pair(4, curses.COLOR_BLACK, -1)                   # dim
    curses.init_pair(5, curses.COLOR_BLACK, curses.COLOR_CYAN)    # selected
    curses.init_pair(6, curses.COLOR_YELLOW, -1)                  # verb
    curses.init_pair(7, curses.COLOR_WHITE, curses.COLOR_BLUE)    # status bar
    curses.init_pair(8, curses.COLOR_WHITE, -1)                   # tree normal
    curses.init_pair(9, curses.COLOR_BLACK, curses.COLOR_GREEN)   # tree selected
    curses.init_pair(10, curses.COLOR_GREEN, -1)                  # tree active file
    curses.init_pair(11, curses.COLOR_BLUE, -1)                   # tree dir

    P_TREE = {"hdr": 11, "link": 8, "cur": 10, "dim": 4, "verb": 6, "sel": 9}
    P_CTX = {"hdr": 1, "link": 2, "cur": 3, "dim": 4, "verb": 6, "sel": 5}

    ws_name = get_workspace_name(workspace)
    tree_nodes = build_file_tree(workspace, ws_name)
    tree_selectable = [i for i, n in enumerate(tree_nodes) if n.file]

    history: list[str] = []
    current_file = start_file
    focus = FOCUS_CTX
    tree_cursor = 0
    ctx_cursor = 0
    tree_scroll = 0
    ctx_scroll = 0

    # Pre-select current file in tree
    for idx, si in enumerate(tree_selectable):
        if tree_nodes[si].file == current_file:
            tree_cursor = idx
            break

    def navigate_to(target: str):
        nonlocal current_file, ctx_cursor, ctx_scroll, tree_cursor
        history.append(current_file)
        current_file = target
        ctx_cursor = 0
        ctx_scroll = 0
        # Sync tree cursor
        for idx, si in enumerate(tree_selectable):
            if tree_nodes[si].file == current_file:
                tree_cursor = idx
                break

    def navigate_back():
        nonlocal current_file, ctx_cursor, ctx_scroll, tree_cursor
        if history:
            current_file = history.pop()
            ctx_cursor = 0
            ctx_scroll = 0
            for idx, si in enumerate(tree_selectable):
                if tree_nodes[si].file == current_file:
                    tree_cursor = idx
                    break

    while True:
        ctx_items = build_context(workspace, current_file, ws_name)
        ctx_selectable = [i for i, it in enumerate(ctx_items) if it.target_file]

        # Clamp cursors
        if tree_selectable:
            tree_cursor = min(tree_cursor, len(tree_selectable) - 1)
        if ctx_selectable:
            ctx_cursor = min(ctx_cursor, len(ctx_selectable) - 1)

        # Inner draw/input loop (breaks on navigation to rebuild context)
        needs_rebuild = False
        while not needs_rebuild:
            stdscr.clear()
            h, w = stdscr.getmaxyx()
            content_h = h - 2  # status bar takes last line, title takes first

            # Pane widths: tree gets ~35% but at least 25 cols, max 40
            tree_w = max(25, min(40, w * 35 // 100))
            ctx_x = tree_w + 1  # 1 col for divider
            ctx_w = w - ctx_x

            if ctx_w < 20:
                # Terminal too narrow for split — just show context
                tree_w = 0
                ctx_x = 0
                ctx_w = w

            # ── Draw tree pane ──────────────────────────────────────
            if tree_w > 0:
                # Scroll tree
                if tree_selectable:
                    tl = tree_selectable[tree_cursor]
                    if tl < tree_scroll:
                        tree_scroll = tl
                    elif tl >= tree_scroll + content_h:
                        tree_scroll = tl - content_h + 1

                for row in range(content_h):
                    ni = row + tree_scroll
                    if ni >= len(tree_nodes):
                        break
                    node = tree_nodes[ni]
                    is_sel = (focus == FOCUS_TREE and tree_selectable
                              and tree_cursor < len(tree_selectable)
                              and tree_selectable[tree_cursor] == ni)
                    is_active = (node.file == current_file)
                    prefix = "  " * node.indent
                    label = prefix + node.label
                    attr = _attr_for(
                        "header" if node.is_section else "normal",
                        is_sel, bool(node.file), is_active, P_TREE)
                    try:
                        stdscr.addstr(row, 0, label[:tree_w - 1], attr)
                    except curses.error:
                        pass

                # Divider
                for row in range(content_h):
                    try:
                        stdscr.addstr(row, tree_w, "│", curses.color_pair(4))
                    except curses.error:
                        pass

            # ── Draw context pane ───────────────────────────────────
            if ctx_selectable:
                cl = ctx_selectable[ctx_cursor]
                if cl < ctx_scroll:
                    ctx_scroll = cl
                elif cl >= ctx_scroll + content_h:
                    ctx_scroll = cl - content_h + 1

            for row in range(content_h):
                ii = row + ctx_scroll
                if ii >= len(ctx_items):
                    break
                item = ctx_items[ii]
                is_sel = (focus == FOCUS_CTX and ctx_selectable
                          and ctx_cursor < len(ctx_selectable)
                          and ctx_selectable[ctx_cursor] == ii)
                attr = _attr_for(item.style, is_sel, bool(item.target_file),
                                 False, P_CTX)
                text = item.text[:ctx_w - 1]
                try:
                    stdscr.addstr(row, ctx_x, text, attr)
                except curses.error:
                    pass

            # ── Status bar ──────────────────────────────────────────
            pane_name = "TREE" if focus == FOCUS_TREE else "CONTEXT"
            back_hint = f" ← {Path(history[-1]).name}" if history else ""
            left = f" [{pane_name}] {current_file}{back_hint}"
            right = " Tab:switch  ↑↓:move  ⏎:go  ⌫:back  q:quit "
            pad = max(0, w - len(left) - len(right))
            bar = left + " " * pad + right
            try:
                stdscr.addstr(h - 1, 0, bar[:w], curses.color_pair(7))
            except curses.error:
                pass

            stdscr.refresh()

            # ── Input ───────────────────────────────────────────────
            key = stdscr.getch()

            if key in (ord("q"), ord("Q"), 27):
                return

            elif key == 9:  # Tab
                focus = FOCUS_CTX if focus == FOCUS_TREE else FOCUS_TREE

            elif key in (curses.KEY_UP, ord("k")):
                if focus == FOCUS_TREE:
                    if tree_selectable and tree_cursor > 0:
                        tree_cursor -= 1
                else:
                    if ctx_selectable and ctx_cursor > 0:
                        ctx_cursor -= 1

            elif key in (curses.KEY_DOWN, ord("j")):
                if focus == FOCUS_TREE:
                    if tree_selectable and tree_cursor < len(tree_selectable) - 1:
                        tree_cursor += 1
                else:
                    if ctx_selectable and ctx_cursor < len(ctx_selectable) - 1:
                        ctx_cursor += 1

            elif key == curses.KEY_PPAGE:
                if focus == FOCUS_TREE and tree_selectable:
                    tree_cursor = max(0, tree_cursor - content_h)
                elif ctx_selectable:
                    ctx_cursor = max(0, ctx_cursor - content_h)

            elif key == curses.KEY_NPAGE:
                if focus == FOCUS_TREE and tree_selectable:
                    tree_cursor = min(len(tree_selectable) - 1, tree_cursor + content_h)
                elif ctx_selectable:
                    ctx_cursor = min(len(ctx_selectable) - 1, ctx_cursor + content_h)

            elif key in (curses.KEY_ENTER, 10, 13):
                if focus == FOCUS_TREE and tree_selectable:
                    target = tree_nodes[tree_selectable[tree_cursor]].file
                    if target and target != current_file:
                        navigate_to(target)
                        needs_rebuild = True
                elif focus == FOCUS_CTX and ctx_selectable:
                    target = ctx_items[ctx_selectable[ctx_cursor]].target_file
                    if target:
                        navigate_to(target)
                        needs_rebuild = True

            elif key in (curses.KEY_BACKSPACE, 127):
                navigate_back()
                needs_rebuild = True

            elif key == curses.KEY_RESIZE:
                pass


# ── Entry point ─────────────────────────────────────────────────────────────

def main():
    if len(sys.argv) < 2:
        print(__doc__)
        sys.exit(1)

    workspace = sys.argv[1]
    file_path = sys.argv[2] if len(sys.argv) > 2 else None

    if not file_path:
        files = get_all_files(workspace)
        if not files:
            print("No files found in workspace", file=sys.stderr)
            sys.exit(1)
        file_path = files[0]

    curses.wrapper(run_tui, workspace, file_path)


if __name__ == "__main__":
    main()
