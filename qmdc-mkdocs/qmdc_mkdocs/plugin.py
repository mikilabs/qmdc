"""MkDocs plugin (QmdcPlugin) — hooks into MkDocs lifecycle."""

import re

from mkdocs.config import config_options
from mkdocs.plugins import BasePlugin


class QmdcPlugin(BasePlugin):
    """QMDC integration plugin for MkDocs.

    Hooks into MkDocs lifecycle to:
    - Inject custom CSS/JS assets via on_config
    - Pass graph sidebar and semantic hints data to templates via on_page_context
    - Inject inline hint icons next to object headings via on_post_page

    Options (set under ``plugins: - qmdc:`` in mkdocs.yml):
    - ``hidden_kinds``: list of Kind names to exclude from the graph sidebar
      edges (e.g. ``[ContentGenerator]``).
    """

    config_scheme = (
        ("hidden_kinds", config_options.ListOfItems(config_options.Type(str), default=[])),
    )

    def on_config(self, config):
        """Inject QMDC assets, Mermaid support, and the required Markdown extensions.

        Everything the QMDC conversion pipeline depends on is injected here so the
        plugin works regardless of the user's (or auto-generated minimal)
        ``mkdocs.yml`` — users only need ``plugins: [qmdc]``.

        - extra_css / extra_javascript: QMDC features + Mermaid zoom/pan viewport.
          The Mermaid renderer (js/qmdc-mermaid.js) is a thin bootstrap that loads
          a vendored, self-hosted mermaid (js/mermaid.min.js — no third-party CDN)
          and the shared renderer core (js/qmdc-mermaid-core.js, also used by the
          VS Code preview). All three ship via the scaffolded ``custom_dir``.
        - markdown_extensions / mdx_configs:
          * a ``mermaid`` custom fence on pymdownx.superfences that emits
            ``<div class="mermaid">`` (a div — not ``<pre>`` — so Material's own
            bundled mermaid loader leaves rendering to js/qmdc-mermaid.js);
          * the extensions the converter's output needs: ``admonition`` +
            ``pymdownx.details`` (ContentGenerator ``??? note`` collapsibles),
            ``md_in_html`` (raw HTML sidebar / hint popovers / SQL tables),
            ``attr_list``, and ``pymdownx.emoji`` (the hint icon etc.).
        """
        extra_css = config.setdefault("extra_css", [])
        for asset in ("css/qmdc-extra.css", "css/qmdc-mermaid.css"):
            if asset not in extra_css:
                extra_css.append(asset)
        extra_js = config.setdefault("extra_javascript", [])
        for asset in ("js/qmdc-extra.js", "js/qmdc-mermaid.js"):
            if asset not in extra_js:
                extra_js.append(asset)

        _enable_mermaid_fence(config)
        _ensure_required_extensions(config)
        return config

    def on_page_context(self, context, page, config, nav):
        """Read graph sidebar and semantic hints from page meta, inject into template context."""
        meta = page.meta or {}
        context["qmdc_graph_sidebar"] = meta.get("_graph_sidebar", {})
        context["qmdc_semantic_hints"] = meta.get("_semantic_hints", {})
        # Semantic "next" link (footer.html renders it as Material's next-link).
        context["qmdc_next"] = meta.get("_next")
        # Repoint the per-page source/edit link at the real .qmd.md source. MkDocs
        # computes page.edit_url from the generated .md path inside the temp build
        # dir, which does not exist in the repo — rebuild it from the original path.
        self._fix_edit_url(page, config, meta)
        return context

    @staticmethod
    def _fix_edit_url(page, config, meta):
        """Override ``page.edit_url`` to point at the real ``.qmd.md`` source on the repo.

        No-op unless the workspace's ``mkdocs.yml`` sets ``repo_url`` and the page
        carries a ``_source_file`` (the un-stripped workspace-relative source path).
        The result feeds Material's ``content.action.edit`` / ``content.action.view``
        buttons, so the link lands on the actual source instead of a 404 ``.md`` path.
        """
        source_file = meta.get("_source_file")
        if not source_file:
            return
        # config is a MkDocsConfig (dict-like) at build time, a plain dict in tests.
        repo_url = config.get("repo_url") if hasattr(config, "get") else None
        if not repo_url:
            return
        edit_uri = (config.get("edit_uri") if hasattr(config, "get") else None) or ""
        parts = [repo_url.rstrip("/")]
        if edit_uri:
            parts.append(edit_uri.strip("/"))
        parts.append(source_file.lstrip("/"))
        page.edit_url = "/".join(parts)

    def on_post_page(self, output, page, config):
        """Inject inline hint icons next to object headings in rendered HTML."""
        meta = page.meta or {}
        hints = meta.get("_semantic_hints", {})
        if not hints:
            return output

        # For each object with hints, find its heading (by id) and inject hint icon
        for obj_id, entries in hints.items():
            if not entries:
                continue

            # Build the popover HTML
            items_html = ""
            for h in entries:
                label = _escape(h.get("label", ""))
                kind = h.get("kind", "")
                score = int(h.get("score", 0) * 100)
                href = _escape(h.get("href", "#"))
                kind_span = (
                    f' <span class="sb-edge-kind">{_escape(kind)}</span>' if kind else ""
                )
                items_html += (
                    f'<a href="{href}" class="qmdc-hint-item">'
                    f'{label}{kind_span}'
                    f' <span class="qmdc-hint-score">{score}%</span></a>'
                )

            hint_html = (
                f'<span class="qmdc-hint-wrapper" data-pagefind-ignore>'
                f'<button class="qmdc-hint-icon" data-hint-toggle="{_escape(obj_id)}" '
                f'aria-label="Similar to {_escape(obj_id)}" aria-expanded="false">💡</button>'
                f'<span class="qmdc-hint-popover" id="hint-popover-{_escape(obj_id)}" hidden>'
                f'{items_html}</span></span>'
            )

            # Find the span with this id and inject the hint icon inside the
            # heading (before the closing tag)
            id_pattern = re.escape(f'id="{obj_id}"')
            heading_pattern = re.compile(
                rf'(<h[2-6][^>]*>)(.*?{id_pattern}.*?)(</h[2-6]>)',
                re.DOTALL,
            )
            match = heading_pattern.search(output)
            if match:
                # Insert hint icon before the closing </hN> tag
                output = output[:match.start(3)] + hint_html + output[match.start(3):]

        return output


def _enable_mermaid_fence(config) -> None:
    """Register a ``mermaid`` custom fence on pymdownx.superfences (idempotent).

    Ensures ``pymdownx.superfences`` is in ``markdown_extensions`` and adds a
    custom fence that renders ```` ```mermaid ```` blocks as
    ``<div class="mermaid">SOURCE</div>`` via ``fence_div_format``.

    A div (not the default ``<pre>``) keeps Material's bundled mermaid loader —
    which hooks ``pre.mermaid`` — from touching our diagrams, so the QMDC
    enhancement script (js/qmdc-mermaid.js) is the sole renderer.

    Other custom fences (user- or workspace-defined) are preserved. A
    pre-existing ``mermaid`` fence is replaced with the div form (so a workspace
    mkdocs.yml using ``fence_code_format`` doesn't defeat our renderer), which
    also makes repeated calls idempotent.
    """
    from pymdownx.superfences import fence_div_format

    extensions = config.get("markdown_extensions")
    if extensions is None:
        extensions = []
        config["markdown_extensions"] = extensions
    if "pymdownx.superfences" not in extensions:
        extensions.append("pymdownx.superfences")

    mdx_configs = config.get("mdx_configs")
    if mdx_configs is None:
        mdx_configs = {}
        config["mdx_configs"] = mdx_configs

    superfences_cfg = mdx_configs.setdefault("pymdownx.superfences", {})
    custom_fences = superfences_cfg.setdefault("custom_fences", [])

    mermaid_fence = {
        "name": "mermaid",
        "class": "mermaid",
        "format": fence_div_format,
    }

    # Replace any pre-existing mermaid fence (e.g. a workspace mkdocs.yml that
    # declared one with fence_code_format → <pre class="mermaid">). We force the
    # div form so Material's bundled loader (which hooks pre.mermaid) stays out
    # and our enhancement script is the sole renderer.
    for i, fence in enumerate(custom_fences):
        if fence.get("name") == "mermaid":
            custom_fences[i] = mermaid_fence
            return

    custom_fences.append(mermaid_fence)


def _ensure_required_extensions(config) -> None:
    """Ensure the Markdown extensions the QMDC pipeline depends on are enabled.

    The converter emits content that only renders with specific extensions, so
    we guarantee them here rather than relying on the user's mkdocs.yml:

      - ``admonition`` + ``pymdownx.details`` — ContentGenerator sections are
        wrapped in ``??? note`` collapsibles.
      - ``md_in_html`` — the graph sidebar, hint popovers and SQL-block tables
        are injected as raw HTML containing Markdown.
      - ``attr_list`` — attribute lists.
      - ``pymdownx.superfences`` — mermaid + nested fences (also added by
        ``_enable_mermaid_fence``; listed here for completeness/idempotency).
      - ``pymdownx.emoji`` — emoji shortcodes (e.g. the hint icon), wired to
        Material's twemoji index + svg generator.

    Idempotent: already-present extensions are not duplicated, and an existing
    ``pymdownx.emoji`` config (user choice) is left untouched.
    """
    extensions = config.get("markdown_extensions")
    if extensions is None:
        extensions = []
        config["markdown_extensions"] = extensions

    required = [
        "admonition",
        "pymdownx.details",
        "md_in_html",
        "attr_list",
        "pymdownx.superfences",
        "pymdownx.emoji",
    ]
    for ext in required:
        if ext not in extensions:
            extensions.append(ext)

    mdx_configs = config.get("mdx_configs")
    if mdx_configs is None:
        mdx_configs = {}
        config["mdx_configs"] = mdx_configs

    # Wire emoji to Material's twemoji index + svg generator (the !!python/name
    # tags a user would otherwise write in YAML), unless the user already
    # configured it.
    if "pymdownx.emoji" not in mdx_configs:
        from material.extensions import emoji

        mdx_configs["pymdownx.emoji"] = {
            "emoji_index": emoji.twemoji,
            "emoji_generator": emoji.to_svg,
        }


def _escape(s: str) -> str:
    """HTML-escape a string."""
    return s.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;").replace('"', "&quot;")
