"""QMDC → Markdown pre-rendering pipeline orchestration."""

from __future__ import annotations

import hashlib
import re
import shutil
import sys
from pathlib import Path, PurePosixPath
from typing import TYPE_CHECKING

import yaml

from .graph import GraphContext, compute_graph_context
from .hints import HintEntry, get_page_hints
from .ignore import is_excluded, load_siteignore
from .paths import md_to_url_path, qmdc_to_md_path
from .references import broken_link_span, obj_label, resolve_references
from .sql_blocks import process_sql_blocks
from .syntax import transform_definitions

if TYPE_CHECKING:
    from .database import WorkspaceData


def convert_workspace(
    workspace: Path,
    tmpdir: Path,
    ws_data: WorkspaceData,
    hints: dict[str, list[HintEntry]],
    namespace_prefix: str | None = None,
    hidden_kinds: list[str] | None = None,
) -> int:
    """Convert all .qmd.md files to .md in tmpdir/docs/. Returns page count.

    Args:
        workspace: Workspace root path.
        tmpdir: Temporary build directory.
        ws_data: Parsed workspace data.
        hints: Semantic hints dict.
        namespace_prefix: If set, only convert files under this prefix (e.g. 'adocs').
        hidden_kinds: Kinds to exclude from graph sidebar edges.

    Pipeline for each file:
    1. resolve_references — replaces [[#ref]] with Markdown links (position-based)
    2. transform_definitions — converts [[id: Kind]] to HTML spans (line-targeted)
    3. process_sql_blocks — executes ```table blocks
    4. compute_graph_context — gets sidebar data
    5. get_page_hints — gets hint data
    6. Build YAML front matter with _graph_sidebar and _semantic_hints
    7. Write output .md file

    Also copies _pages/*.md files unchanged into docs/_pages/.
    If a plain readme.md exists alongside readme.qmd.md, the plain one is used
    as the index page (copied unchanged, no QMDC transformation).
    """
    docs_dir = tmpdir / "docs"
    docs_dir.mkdir(parents=True, exist_ok=True)

    page_count = 0

    # Cache of already-copied media (dest-relative path → True), shared across pages so a
    # file referenced from several pages is copied once.
    copied_assets: dict[str, bool] = {}

    # Load .siteignore patterns (gitignore-style, for excluding files from the site)
    ignore_patterns = load_siteignore(workspace)

    # Collect plain readme.md files (these take priority over readme.qmd.md)
    plain_readmes: set[str] = set()
    for readme_file in workspace.rglob("readme.md"):
        rel = readme_file.relative_to(workspace)
        if any(part.startswith(".") for part in rel.parts):
            continue
        if "_pages" in rel.parts:
            continue
        # Store the directory (workspace-relative) that has a plain readme.md
        plain_readmes.add(str(PurePosixPath(*rel.parent.parts)) if rel.parent.parts else ".")

    # Find all .qmd.md files (skip hidden directories and _pages)
    for qmdc_file in workspace.rglob("*.qmd.md"):
        rel = qmdc_file.relative_to(workspace)

        # Skip hidden directories (any path component starting with '.')
        if any(part.startswith(".") for part in rel.parts):
            continue

        # Skip _pages directory
        if "_pages" in rel.parts:
            continue

        source_file = str(PurePosixPath(*rel.parts))

        # Skip files outside namespace prefix (when building a single namespace)
        if namespace_prefix and not source_file.startswith(namespace_prefix + "/"):
            continue

        # Skip files matching .qmdc-mkdocs.ignore (full or namespace-stripped path)
        if is_excluded(source_file, ignore_patterns, namespace_prefix):
            continue

        # Skip readme.qmd.md if a plain readme.md exists in the same directory
        if rel.name == "readme.qmd.md":
            dir_key = str(PurePosixPath(*rel.parent.parts)) if rel.parent.parts else "."
            if dir_key in plain_readmes:
                continue

        lines = qmdc_file.read_text(encoding="utf-8").splitlines(keepends=True)

        # Get parser objects for this file
        file_objects = getattr(ws_data, "objects_by_file", {}).get(source_file, [])

        # Pipeline: references (position-based) → fallback regex → definitions
        # → field headings → example strip → sql_blocks
        lines = resolve_references(
            lines, file_objects, source_file, ws_data, ignore_patterns, namespace_prefix
        )
        content = "".join(lines)
        content = _resolve_remaining_refs_regex(
            content, source_file, ws_data, ignore_patterns, namespace_prefix
        )
        # Check for unresolved references (after both resolvers ran)
        _warn_unresolved_refs(content, source_file)
        lines = content.splitlines(keepends=True)
        lines = transform_definitions(lines, file_objects)
        lines = _transform_remaining_heading_markers(lines)
        lines = _strip_example_modifier(lines)
        content = "".join(lines)
        content = process_sql_blocks(content, ws_data, source_file)
        content = _collapse_content_generators(content, file_objects)
        # Props card LAST: it changes the line count of the top object's metadata
        # block, so it must run after every line-number-based transform
        # (transform_definitions, _collapse_content_generators). The page object's
        # heading sits above any CG/SQL edits, so its source line number is still
        # valid here.
        content = _render_object_props_card(content, file_objects)

        # Compute metadata
        graph_ctx = compute_graph_context(
            source_file, ws_data, ignore_patterns,
            hidden_kinds=hidden_kinds, namespace_prefix=namespace_prefix,
        )
        page_hints = get_page_hints(source_file, ws_data, hints)

        # Strip namespace prefix for output paths and href computation
        out_source = source_file
        if namespace_prefix and out_source.startswith(namespace_prefix + "/"):
            out_source = out_source[len(namespace_prefix) + 1:]

        # Copy every locally-referenced image/video into the site and rewrite the
        # reference. Sources may live anywhere (hidden dir, outside the workspace, absolute
        # path) — MkDocs ignores dotfiles and can't serve external paths, so in-place
        # serving is impossible for those.
        content = _copy_and_rewrite_media(
            content, source_file, out_source, workspace, docs_dir, copied_assets
        )

        # Build front matter and prepend. Hrefs are relative to the page's OUTPUT
        # location (out_source, namespace-stripped), but the GitHub source link needs
        # the real workspace-relative .qmd.md path (source_file, un-stripped).
        front_matter = _build_front_matter(graph_ctx, page_hints, out_source, source_file)
        content = front_matter + content

        # Write output .md file
        # readme.qmd.md → index.md (MkDocs convention for directory index pages)
        output_path = qmdc_to_md_path(out_source)
        output_file = docs_dir / output_path
        output_file.parent.mkdir(parents=True, exist_ok=True)
        output_file.write_text(content, encoding="utf-8")

        page_count += 1

    # Copy plain readme.md files as index.md (no QMDC transformation, but still run the
    # media pass so referenced images are shipped + rewritten like on any other page).
    for readme_file in workspace.rglob("readme.md"):
        rel = readme_file.relative_to(workspace)
        if any(part.startswith(".") for part in rel.parts):
            continue
        if "_pages" in rel.parts:
            continue
        rel_str = str(PurePosixPath(*rel.parts))
        # readme.md → index.md in the output
        out_dir = docs_dir / PurePosixPath(*rel.parent.parts) if rel.parent.parts else docs_dir
        out_dir.mkdir(parents=True, exist_ok=True)
        out_file = out_dir / "index.md"
        text = readme_file.read_text(encoding="utf-8")
        text = _copy_and_rewrite_media(
            text, rel_str, rel_str, workspace, docs_dir, copied_assets
        )
        out_file.write_text(text, encoding="utf-8")
        page_count += 1

    # Copy _pages/*.md (media pass applied; no QMDC transformation)
    pages_dir = workspace / "_pages"
    if pages_dir.is_dir():
        dest_pages = docs_dir / "_pages"
        dest_pages.mkdir(parents=True, exist_ok=True)
        for md_file in pages_dir.glob("*.md"):
            rel_str = f"_pages/{md_file.name}"
            text = md_file.read_text(encoding="utf-8")
            text = _copy_and_rewrite_media(
                text, rel_str, rel_str, workspace, docs_dir, copied_assets
            )
            (dest_pages / md_file.name).write_text(text, encoding="utf-8")

    return page_count


def _resolve_remaining_refs_regex(
    content: str,
    source_file: str,
    ws_data: WorkspaceData,
    ignore_patterns: list[str] | None = None,
    namespace_prefix: str | None = None,
) -> str:
    """Fallback: resolve any remaining [[#...]] references via regex.

    The position-based resolver only handles references the parser extracted into
    __references. Some references (in text field preambles, nested fields) may be
    missed. This pass catches them.

    Protects code fences and inline code from transformation using line-by-line
    fence tracking (handles nested fences correctly).

    A reference whose resolved target page is excluded from the site renders as a
    non-navigable dead link (broken-link span with the label) — using the same
    exclusion rule as page generation (ignore.is_excluded).
    """
    from .references import _compute_relative_path, _direct_lookup

    # Split into lines and track which are inside EXAMPLE code fences only
    # Regular code fences DO have refs resolved (per QMD.md spec)
    lines = content.split("\n")
    in_example_fence = False
    fence_lines: set[int] = set()

    for i, line in enumerate(lines):
        stripped = line.strip()
        if stripped.startswith("```"):
            if in_example_fence:
                in_example_fence = False
                fence_lines.add(i)
            elif "example" in stripped:
                in_example_fence = True
                fence_lines.add(i)
        elif in_example_fence:
            fence_lines.add(i)

    # Process only non-fence lines
    ref_pattern = re.compile(r"\[\[#([^\[\]]+)\]\]")

    for i, line in enumerate(lines):
        if i in fence_lines:
            continue
        if "[[#" not in line:
            continue
        # Skip inline code spans
        # Replace refs outside of backticks
        def replace_ref(m: re.Match, line: str = line) -> str:
            # Check if this match is inside inline code (between backtick pairs)
            start = m.start()
            # Find all inline code spans on this line
            code_spans = [(cm.start(), cm.end()) for cm in re.finditer(r'`[^`]*`', line)]
            for cs_start, cs_end in code_spans:
                if cs_start < start < cs_end:
                    return m.group(0)  # Inside inline code, skip

            raw = m.group(0)
            ref_str = m.group(1)
            parts = ref_str.split(":")
            target = _direct_lookup(parts, ws_data, source_file)
            if target is None:
                return raw  # Leave as-is (don't mark broken in fallback — might be intentional)
            if is_excluded(target["__file"], ignore_patterns or [], namespace_prefix):
                # Target page is excluded from the site — dead link, no href.
                return broken_link_span(obj_label(target))
            rel_path = _compute_relative_path(source_file, target["__file"])
            return f"[{obj_label(target)}]({rel_path}#{target['__id']})"

        lines[i] = ref_pattern.sub(replace_ref, line)

    return "\n".join(lines)


# Media reference patterns. Markdown images `![alt](url "title")` and HTML media tags
# (`<img>`, `<video>`, `<source>`) with a `src="…"`/`src='…'` attribute.
_MD_IMAGE_RE = re.compile(r"(!\[[^\]]*\]\()([^)\s]+)((?:\s+\"[^\"]*\")?\))")
_HTML_SRC_RE = re.compile(
    r"(<(?:img|video|source)\b[^>]*?\bsrc=[\"'])([^\"']+)([\"'])",
    re.IGNORECASE,
)

# Reference targets that are never local files.
_NON_LOCAL_PREFIXES = ("http://", "https://", "//", "data:", "mailto:", "#")

# Where copied media lands inside the built site. A non-dot dir (MkDocs ships it) with a
# per-source-path hash subdir (collision-proof across source dirs) keeping the basename.
_ASSET_DEST_ROOT = PurePosixPath("assets/qmdc")


def _copy_and_rewrite_media(
    content: str,
    source_file: str,
    out_source: str,
    workspace: Path,
    docs_dir: Path,
    copied: dict[str, bool],
) -> str:
    """Copy every locally-referenced image/video into the site and rewrite its reference.

    Reference-driven (never an extension sweep): only files a page actually references are
    shipped. A target may live anywhere — a hidden dir, outside the workspace, or an
    absolute path — because MkDocs ignores dotfiles and cannot serve paths outside
    ``docs_dir``. Such files are copied under ``docs/assets/qmdc/<hash>/<name>`` and the
    reference rewritten to point there (relative to the page's output location).

    Left untouched: remote/`data:`/anchor targets; and files already servable in place
    (inside the workspace under a non-dot path) — MkDocs serves those as-is. A referenced
    file that does not exist is a stderr warning, not a failure (the build proceeds).

    Code fences are skipped so example snippets aren't rewritten.
    """

    def process(url: str) -> str | None:
        return _resolve_copy_rewrite(
            url, source_file, out_source, workspace, docs_dir, copied
        )

    def md_repl(m: re.Match) -> str:
        new = process(m.group(2))
        return m.group(1) + (new if new is not None else m.group(2)) + m.group(3)

    def html_repl(m: re.Match) -> str:
        new = process(m.group(2))
        return m.group(1) + (new if new is not None else m.group(2)) + m.group(3)

    lines = content.split("\n")
    in_fence = False
    for i, line in enumerate(lines):
        if line.strip().startswith("```"):
            in_fence = not in_fence
            continue
        if in_fence:
            continue
        line = _MD_IMAGE_RE.sub(md_repl, line)
        line = _HTML_SRC_RE.sub(html_repl, line)
        lines[i] = line
    return "\n".join(lines)


def _resolve_copy_rewrite(
    url: str,
    source_file: str,
    out_source: str,
    workspace: Path,
    docs_dir: Path,
    copied: dict[str, bool],
) -> str | None:
    """Resolve one media reference; copy it in and return the rewritten path, or ``None``
    to leave the reference unchanged."""
    raw = url.strip()
    if not raw or raw.lower().startswith(_NON_LOCAL_PREFIXES):
        return None

    # Drop any #fragment / ?query before touching the filesystem.
    path_part = raw.split("#", 1)[0].split("?", 1)[0]
    if not path_part:
        return None

    candidate = Path(path_part).expanduser()
    if candidate.is_absolute():
        abs_src = candidate
    else:
        abs_src = (workspace / source_file).parent / candidate
    try:
        abs_src = abs_src.resolve()
    except OSError:
        return None

    if not abs_src.is_file():
        print(
            f"  WARN: {source_file}: referenced media not found: {raw}",
            file=sys.stderr,
        )
        return None

    # Already servable in place? (inside the workspace, no dot-prefixed path part.)
    try:
        rel = abs_src.relative_to(workspace.resolve())
        if not any(part.startswith(".") for part in rel.parts):
            return None
    except ValueError:
        pass  # outside the workspace → must copy

    digest = hashlib.sha1(str(abs_src).encode("utf-8")).hexdigest()[:12]
    dest_rel = _ASSET_DEST_ROOT / digest / abs_src.name
    dest_rel_str = str(dest_rel)
    if dest_rel_str not in copied:
        dest_abs = docs_dir / dest_rel
        dest_abs.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(abs_src, dest_abs)
        copied[dest_rel_str] = True

    # Rewrite to a path relative to the page's output location (MkDocs then resolves it).
    from .references import _compute_relative_path

    return _compute_relative_path(out_source, dest_rel_str)


def _transform_remaining_heading_markers(lines: list[str]) -> list[str]:
    """Transform any remaining [[id: Kind]] markers on heading lines.

    This catches field definition headings (like ### Description [[description: text]])
    that weren't handled by transform_definitions (which only processes object headings).
    Skips lines already transformed (containing qmdc-id class).
    Skips lines inside code fences.
    """
    from .syntax import _transform_heading_line

    result = list(lines)
    in_fence = False
    for i, line in enumerate(result):
        stripped = line.strip()
        if stripped.startswith("```"):
            in_fence = not in_fence
            continue
        if in_fence:
            continue
        # Only process heading lines that still have [[ markers and aren't already transformed
        if line.lstrip().startswith("#") and "[[" in line and "qmdc-id" not in line:
            result[i] = _transform_heading_line(line)
    return result


def _strip_example_modifier(lines: list[str]) -> list[str]:
    """Strip 'example' modifier from code fence info strings.

    QMD.md uses ```markdown example to mark example code blocks where references
    aren't parsed. MkDocs doesn't understand this modifier — it needs just
    the language name. We strip 'example' so fences render correctly.

    ```markdown example  →  ```markdown
    ```json example      →  ```json
    ```                  →  ``` (unchanged)
    """
    result = list(lines)
    for i, line in enumerate(result):
        if line.lstrip().startswith("```") and " example" in line:
            result[i] = re.sub(r"^(\s*```\w*)\s+example", r"\1", line)
    return result


_PROPS_BULLET_RE = re.compile(r"^\s*-\s+([A-Za-z_][\w-]*):\s?(.*)$")


def _render_object_props_card(content: str, file_objects: list[dict]) -> str:
    """Replace the page object's leading metadata bullets with a styled props card.

    The inline fields directly under the page's top-level object heading
    (``- key: value`` bullets) render by default as a plain bullet list. This
    rewrites that block into ``<div class="qmdc-props" markdown="1">`` wrapping a
    list whose keys are wrapped in muted-label spans — styled as a key/value panel
    in qmdc-extra.css.

    The ``next`` field is dropped here: it is surfaced as a footer link instead
    (see plugin/footer.html). References in values are already resolved to Markdown
    links by the time this runs, and ``markdown="1"`` lets md_in_html process them.

    Only the first (top-level) object's block is transformed. Operates on the final
    content string and MUST run after every line-number-based transform, since it
    changes the metadata block's line count; the page object's heading is above any
    such edits, so ``__line`` still resolves correctly. If the block can't be cleanly
    parsed as ``key: value`` bullets, the content is returned unchanged.
    """
    if not file_objects:
        return content

    lined = [o for o in file_objects if o.get("__line")]
    if not lined:
        return content
    top = min(lined, key=lambda o: int(o["__line"]))
    heading_idx = int(top["__line"]) - 1

    lines = content.split("\n")
    n = len(lines)
    if heading_idx < 0 or heading_idx >= n:
        return content

    i = heading_idx + 1
    while i < n and lines[i].strip() == "":
        i += 1
    block_start = i

    rows: list[tuple[str, str]] = []
    while i < n:
        raw = lines[i]
        if raw.strip() == "":
            break
        m = _PROPS_BULLET_RE.match(raw)
        if not m:
            break
        rows.append((m.group(1), m.group(2).strip()))
        i += 1
    block_end = i

    if not rows:
        return content

    # `next` now lives in the footer — drop it from the card.
    rows = [(k, v) for k, v in rows if k != "next"]
    replacement = [_props_card_html(rows)] if rows else []
    new_lines = lines[:block_start] + replacement + lines[block_end:]
    return "\n".join(new_lines)


def _props_card_html(rows: list[tuple[str, str]]) -> str:
    """Render parsed (key, value) rows as a md_in_html props-card block.

    Emits a Markdown list (so reference links in values still render) inside a
    ``markdown="1"`` div; each key is wrapped in a muted-label span.
    """
    items = "\n".join(
        f'- <span class="qmdc-prop__key">{key}</span>'
        f'<span class="qmdc-prop__val">{value}</span>'
        for key, value in rows
    )
    return f'<div class="qmdc-props" markdown="1">\n\n{items}\n\n</div>'


def _build_front_matter(
    graph_ctx: GraphContext,
    page_hints: dict[str, list[HintEntry]],
    source_file: str,
    repo_source_file: str | None = None,
) -> str:
    """Build YAML front matter with _graph_sidebar and _semantic_hints metadata.

    The front matter includes href fields computed as relative paths from the
    source file to each linked target.

    Args:
        source_file: Page-relative path used as the base for href computation
            (namespace-stripped output path).
        repo_source_file: The real, un-stripped workspace-relative .qmd.md path,
            emitted as ``_source_file`` so the plugin can build a GitHub source
            link that points at the actual source (not the generated .md).
    """
    # Build graph sidebar data with hrefs
    sidebar_data: dict = {
        "workspace_label": graph_ctx.workspace_label,
        "namespace_label": graph_ctx.namespace_label,
        "file_label": graph_ctx.file_label,
        "links_to": [
            {
                "edge_type": edge.edge_type,
                "obj_id": edge.obj_id,
                "label": edge.label,
                "kind": edge.kind,
                "href": _compute_href(source_file, edge.file, edge.obj_id),
            }
            for edge in graph_ctx.links_to
        ],
        "linked_from": [
            {
                "edge_type": edge.edge_type,
                "obj_id": edge.obj_id,
                "label": edge.label,
                "kind": edge.kind,
                "href": _compute_href(source_file, edge.file, edge.obj_id),
            }
            for edge in graph_ctx.linked_from
        ],
        "siblings": [
            {
                "file": sib.file.replace(".qmd.md", ".md"),
                "label": sib.label,
                "is_current": sib.is_current,
                "href": _compute_href(source_file, sib.file, None),
            }
            for sib in graph_ctx.siblings
        ],
        "toc": graph_ctx.toc,
    }

    # Build semantic hints data with hrefs
    hints_data: dict = {}
    for obj_id, entries in page_hints.items():
        hints_data[obj_id] = [
            {
                "label": entry.label,
                "kind": entry.kind,
                "file": entry.file.replace(".qmd.md", ".md"),
                "score": entry.score,
                "href": _compute_href(source_file, entry.file, entry.id),
            }
            for entry in entries
        ]

    # Only emit front matter if there's data
    meta: dict = {}
    if sidebar_data:
        meta["_graph_sidebar"] = sidebar_data
    if hints_data:
        meta["_semantic_hints"] = hints_data

    # Semantic "next" link (the `next` edge) — rendered as a Material footer link.
    next_edge = next((e for e in graph_ctx.links_to if e.edge_type == "next"), None)
    if next_edge:
        meta["_next"] = {
            "label": next_edge.label,
            "href": _compute_href(source_file, next_edge.file, next_edge.obj_id),
        }

    # Real source path (un-stripped .qmd.md) for the GitHub "view source" link.
    if repo_source_file:
        meta["_source_file"] = repo_source_file

    if not meta:
        return ""

    yaml_str = yaml.dump(meta, default_flow_style=False, sort_keys=False, allow_unicode=True)
    return f"---\n{yaml_str}---\n"


def _compute_href(source_file: str, target_file: str, anchor: str | None) -> str:
    """Compute relative href from source file to target file with optional anchor.

    Both files are workspace-relative paths like 'storage/tables.qmd.md'.
    Outputs directory-style URLs (no .md extension) since these hrefs are
    rendered as raw HTML in templates, not processed by MkDocs's link resolver.

    This is intentionally different from ``references._compute_relative_path``,
    which emits FILE-style ``.md`` paths for the Markdown body (MkDocs rewrites
    those). These hrefs go into raw HTML (the graph sidebar / hint popovers),
    which MkDocs does NOT rewrite, so they must already be final directory URLs.
    Do not merge the two — they target different consumers.

    MkDocs with use_directory_urls=true serves:
    - commands.md → /extension/commands/ (directory)
    - index.md → /extension/ (parent directory)
    """
    source_md = qmdc_to_md_path(source_file)
    target_md = qmdc_to_md_path(target_file)

    # Convert to directory-style URL paths (how MkDocs serves them)
    source_url = md_to_url_path(source_md)
    target_url = md_to_url_path(target_md)

    # The source URL IS a directory (e.g. "architecture/algorithms/")
    # We compute relative path FROM this directory TO the target directory.
    # Split into parts (filter empty strings from trailing slashes)
    s_parts = [p for p in source_url.split("/") if p]
    t_parts = [p for p in target_url.split("/") if p]

    common = 0
    for a, b in zip(s_parts, t_parts, strict=False):
        if a == b:
            common += 1
        else:
            break

    up = len(s_parts) - common
    down = "/".join(t_parts[common:])
    rel_path = ("../" * up) + down

    # Ensure trailing slash for directory URLs
    if rel_path and not rel_path.endswith("/"):
        rel_path += "/"

    if anchor:
        return f"{rel_path}#{anchor}"
    return rel_path


def _collapse_content_generators(content: str, file_objects: list[dict]) -> str:
    """Wrap ContentGenerator sections in collapsible blocks and hide target Content headings.

    1. Finds ContentGenerator objects and wraps their section in admonition-style collapsible
    2. Finds Content headings that are targets of generators and hides them
    """
    # Find ContentGenerator objects in this file
    generators = [
        obj for obj in file_objects
        if obj.get("__kind") == "ContentGenerator" and obj.get("__line")
    ]

    if not generators:
        return content

    lines = content.split("\n")

    # Collect target field names to hide their headings
    target_fields: set[str] = set()
    for gen in generators:
        data = gen
        target = data.get("target", "")
        if "." in target:
            field_name = target.rsplit(".", 1)[1].strip("[]#")
            target_fields.add(field_name)

    # Process generators in reverse order (so line indices stay valid)
    for gen in sorted(generators, key=lambda g: g["__line"], reverse=True):
        gen_line_idx = gen["__line"] - 1  # 0-based
        if gen_line_idx >= len(lines):
            continue

        # Find the heading line and its level
        heading_line = lines[gen_line_idx]
        heading_match = re.match(r"^(#{1,6})\s", heading_line)
        if not heading_match:
            continue
        heading_level = len(heading_match.group(1))

        # Find the end of this section (next heading at same or higher level)
        end_idx = len(lines)
        for i in range(gen_line_idx + 1, len(lines)):
            next_match = re.match(r"^(#{1,6})\s", lines[i])
            if next_match and len(next_match.group(1)) <= heading_level:
                end_idx = i
                break

        # Extract the section content (without the heading itself)
        section_lines = lines[gen_line_idx + 1:end_idx]
        section_content = "\n".join(section_lines).strip()

        # Indent content for admonition
        indented = "\n".join(
            "    " + line if line.strip() else "" for line in section_content.split("\n")
        )

        # Build collapsible admonition (MkDocs Material syntax)
        label = gen.get("__label", "Content Generator")
        collapsed = f'\n??? note "🤖 {label}"\n{indented}\n'

        # Replace the section (heading + content) with the collapsed version
        lines[gen_line_idx:end_idx] = [collapsed]

    # Hide Content headings that are targets of generators
    if target_fields:
        result_lines = []
        for line in lines:
            # Check if this is a heading for a target field (e.g. ### Content [[content: text]])
            if line.lstrip().startswith("#"):
                should_hide = False
                for field_name in target_fields:
                    # Match patterns like: ### Content [[content: text]] or ### Content [[content]]
                    if f"[[{field_name}" in line:
                        should_hide = True
                        break
                if should_hide:
                    # Skip this heading line entirely
                    continue
            result_lines.append(line)
        lines = result_lines

    return "\n".join(lines)


def _warn_unresolved_refs(content: str, source_file: str) -> None:
    """Warn about [[#ref]] references that survived both resolvers.

    Skips refs inside code fences and inline code. Reports the rest as
    unresolved reference warnings to stderr.
    """
    import sys

    lines = content.split("\n")
    in_fence = False
    ref_pattern = re.compile(r"\[\[#([^\[\]]+)\]\]")

    for i, line in enumerate(lines):
        stripped = line.strip()
        if stripped.startswith("```"):
            in_fence = not in_fence
            continue
        if in_fence:
            continue

        for m in ref_pattern.finditer(line):
            # Skip if inside inline code
            code_spans = [(cm.start(), cm.end()) for cm in re.finditer(r"`[^`]*`", line)]
            if any(cs_start < m.start() < cs_end for cs_start, cs_end in code_spans):
                continue

            ref_id = m.group(1)
            # Skip dot-path refs (e.g. [[#obj.field]]): these target a field on an
            # object rather than a page anchor, so a surviving dot-path is not
            # necessarily a broken link — don't warn on it.
            if "." in ref_id:
                continue

            print(
                f"  WARN: {source_file}:{i + 1}: unresolved reference [[#{ref_id}]]",
                file=sys.stderr,
            )
