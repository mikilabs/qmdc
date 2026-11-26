"""QMD.md syntax transformation — [[id: Kind]] → hidden ID span + kind badge.

Two approaches:
- `transform_qmdc_syntax(content)` — legacy full-file regex scan (kept for compatibility)
- `transform_definitions(lines, file_objects)` — line-targeted using parser's `__line` data
"""

import re
from html import escape as html_escape
from typing import Any


def transform_definitions(
    lines: list[str],
    file_objects: list[dict[str, Any]],
) -> list[str]:
    """Transform [[id: Kind]] and [[id]] markers on known heading lines.

    Uses parser's __line to locate definitions precisely — no full-file scanning.
    Only heading lines pointed to by objects' __line are processed.

    Args:
        lines: File content as list of lines.
        file_objects: Objects from parser for this file (with __line, __id, __kind).

    Returns:
        Modified lines with definition markers replaced by HTML spans.
    """
    result = list(lines)

    for obj in file_objects:
        obj_line = obj.get("__line")
        if not isinstance(obj_line, int) or obj_line < 1:
            continue

        idx = obj_line - 1  # Convert to 0-based
        if idx >= len(result):
            continue

        line_text = result[idx]

        # Skip if not a heading line
        if not line_text.lstrip().startswith("#"):
            continue

        # Apply targeted replacement on this one line. Use the object's full
        # __id (dot-composed for nested objects) as the anchor, so it matches
        # the link targets produced by references.py / the graph sidebar.
        result[idx] = _transform_heading_line(line_text, obj.get("__id"))

    return result


# An id attr_list can carry as ``{ #id }`` (no quotes, braces, spaces or ampersands).
_ATTR_LIST_ID_RE = re.compile(r"^[A-Za-z0-9_][A-Za-z0-9_.:-]*$")


def _transform_heading_line(line: str, anchor_id: str | None = None) -> str:
    """Transform QMDC markers on a single heading line.

    MkDocs builds the heading's slug and its search title from the heading's
    text, so no marker text may stay in it (#38). Markers become:

    1. ``[[id: Kind]]`` → a kind badge whose text is drawn by CSS from
       ``data-qmdc-kind`` (system types ``__*``: no badge)
    2. ``[[:Kind]]`` → nothing
    3. ``[[id]]`` → nothing

    and the first marker's id becomes the heading's own ``id`` through an
    ``attr_list`` suffix (``{ #id }``). An id that syntax cannot carry is kept on
    an empty anchor span instead, so it stays targetable.

    ``anchor_id`` overrides the id. For nested objects the parser's ``__id`` is
    dot-composed (``parent.child``) while the heading literally writes the local
    id; the anchor must be the full ``__id`` so it matches reference/sidebar link
    targets.

    Note: callers that handle field-definition headings
    (``_transform_remaining_heading_markers``) invoke this without an
    ``anchor_id`` and so emit local-id anchors. That is fine — that path only
    reaches field headings, not object headings, so there is no nested-id
    mismatch to reconcile there.
    """
    body = line.rstrip("\n")
    newline = line[len(body):]
    found: list[str] = []

    def def_with_kind(m: re.Match) -> str:
        found.append(m.group(1))
        return _kind_badge(m.group(2).strip())

    def bare_id(m: re.Match) -> str:
        found.append(m.group(1))
        return ""

    # 1. [[id: Kind]] — definition with kind
    body = re.sub(r"\[\[([^\]:#]+):\s*([^\]]+)\]\]", def_with_kind, body)
    # 2. [[:Kind]] — kind-only definition
    body = re.sub(r"\s*\[\[:([^\]]+)\]\]", "", body)
    # 3. [[id]] — plain definition (no kind, no #)
    body = re.sub(r"\[\[([^\]#:]+)\]\]", bare_id, body)

    body = body.rstrip()
    anchor = anchor_id if anchor_id else (found[0] if found else None)
    if anchor:
        if _ATTR_LIST_ID_RE.match(anchor):
            body = f"{body} {{ #{anchor} }}"
        else:
            body = f'{body} <span class="qmdc-anchor" id="{html_escape(anchor)}"></span>'
    return body + newline


def _kind_badge(kind: str) -> str:
    """An empty kind badge; CSS draws ``data-qmdc-kind`` (no text in the heading)."""
    if kind.startswith("__"):
        return ""  # system types: no badge
    k = html_escape(kind)
    return (
        f'<span class="qmdc-kind" data-qmdc-kind="{k}" '
        f'data-pagefind-filter="kind:{k}"></span>'
    )


def _replace_bare_id(m: re.Match, anchor_id: str | None = None) -> str:
    """Replace a bare ``[[id]]`` heading marker with a hidden anchor span.

    ``anchor_id`` overrides the emitted HTML ``id``; the displayed ``[[id]]``
    text always keeps the literal local id.
    """
    anchor = anchor_id if anchor_id else m.group(1)
    return (
        f'<span class="qmdc-id" data-pagefind-ignore id="{html_escape(anchor)}">'
        f"[[{m.group(1)}]]</span>"
    )


def _replace_def_with_kind(m: re.Match, anchor_id: str | None = None) -> str:
    """Replace [[id: Kind]] with hidden span + kind badge.

    System types (starting with __) get hidden span only, no badge.

    ``anchor_id`` overrides the emitted HTML ``id`` (the heading anchor); the
    displayed ``[[id: Kind]]`` text always keeps the literal local id ``id``.
    """
    obj_id = m.group(1)
    kind = m.group(2).strip()
    anchor = anchor_id if anchor_id else obj_id
    if kind.startswith("__"):
        # System types: hidden span only, no kind badge
        return (
            f'<span class="qmdc-id" data-pagefind-ignore id="{html_escape(anchor)}">'
            f"[[{obj_id}: {kind}]]</span>"
        )
    return (
        f'<span class="qmdc-id" data-pagefind-ignore id="{html_escape(anchor)}">'
        f"[[{obj_id}: {kind}]]</span>"
        f'<span class="qmdc-kind" data-pagefind-filter="kind">{html_escape(kind)}</span>'
    )


def transform_qmdc_syntax(content: str) -> str:
    """Transform QMDC markers into styled HTML elements (full-file scan).

    Legacy approach — kept for compatibility. Prefer `transform_definitions`
    which uses parser's __line data for precise, line-targeted replacement.

    This matches the existing site's behavior:
    - [[id: Kind]] → hidden ID span + visible kind badge
    - [[:Kind]] → hidden span (Kind-only, auto-generated ID)
    - [[id]] → hidden ID span (anchor only)
    - Code fences and inline code are protected from transformation.

    Note: [[#ref]] references are already resolved by references.py before
    this step runs. Any remaining [[#ref]] in text fields are left as-is.
    """
    # Protect code fences
    fences: list[str] = []

    def save_fence(m: re.Match) -> str:
        fences.append(m.group(0))
        return f"___CODE_FENCE_{len(fences) - 1}___"

    content = re.sub(r"```[^\n]*\n[\s\S]*?```", save_fence, content)

    # Protect inline code
    inlines: list[str] = []

    def save_inline(m: re.Match) -> str:
        inlines.append(m.group(0))
        return f"___INLINE_CODE_{len(inlines) - 1}___"

    content = re.sub(r"`[^`]+`", save_inline, content)

    # 1. [[id: Kind]] definitions → hidden ID span + kind badge
    content = re.sub(r"\[\[([^\]:#]+):\s*([^\]]+)\]\]", _replace_def_with_kind, content)

    # 2. [[:Kind]] definitions (Kind-only, auto-generated ID) → hidden span
    content = re.sub(
        r"\[\[:([^\]]+)\]\]",
        r'<span class="qmdc-id" data-pagefind-ignore>[[:\1]]</span>',
        content,
    )

    # 3. [[id]] definitions (without Kind) → hidden ID span with anchor
    content = re.sub(
        r"\[\[([^\]#:]+)\]\]",
        lambda m: (
            f'<span class="qmdc-id" data-pagefind-ignore id="{html_escape(m.group(1))}">'
            f"[[{m.group(1)}]]</span>"
        ),
        content,
    )

    # Restore code fences and inline code
    for i, fence in enumerate(fences):
        content = content.replace(f"___CODE_FENCE_{i}___", fence)
    for i, inline in enumerate(inlines):
        content = content.replace(f"___INLINE_CODE_{i}___", inline)

    return content
