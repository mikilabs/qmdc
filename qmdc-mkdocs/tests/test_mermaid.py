"""Tests for Mermaid diagram rendering support in qmdc-mkdocs.

Mermaid support was originally built for the (now removed) static-site renderer
and the VS Code previewer. This ports the same capability into the MkDocs
pipeline: diagrams render at natural size and get a zoom/pan/toolbar viewport.

The plugin must, via on_config:
  1. Register a `mermaid` custom fence on pymdownx.superfences that emits
     `<div class="mermaid">` (NOT `<pre class="mermaid">`, which Material's own
     bundled loader would grab — we own rendering with our enhancement script).
  2. Inject the mermaid CSS and JS assets.

The JS asset must load mermaid, initialize it with `useMaxWidth: false` (natural
size) and wrap each diagram in a zoom/pan viewport with a toolbar.
"""

from pathlib import Path

import pytest

from qmdc_mkdocs.plugin import QmdcPlugin

TEMPLATES = Path(__file__).parent.parent / "qmdc_mkdocs" / "templates"


@pytest.fixture
def plugin():
    return QmdcPlugin()


class TestRequiredPackagesAvailable:
    """The plugin imports these at build time; they must be installed.

    pymdownx.* (pymdown-extensions) and material.extensions.emoji are declared
    dependencies (pymdown-extensions explicitly; emoji via mkdocs-material). If
    a future dependency change drops them, on_config would crash mid-build — this
    catches it up front.
    """

    def test_pymdownx_superfences_importable(self):
        from pymdownx.superfences import fence_div_format

        assert callable(fence_div_format)

    def test_material_emoji_importable(self):
        from material.extensions import emoji

        assert callable(emoji.twemoji)
        assert callable(emoji.to_svg)


class TestOnConfigMermaidFence:
    """on_config wires up the mermaid superfence so diagrams become div.mermaid."""

    def test_enables_superfences_extension(self, plugin):
        config = {"markdown_extensions": [], "mdx_configs": {}}
        result = plugin.on_config(config)
        assert "pymdownx.superfences" in result["markdown_extensions"]

    def test_does_not_duplicate_superfences(self, plugin):
        config = {
            "markdown_extensions": ["pymdownx.superfences"],
            "mdx_configs": {},
        }
        result = plugin.on_config(config)
        assert result["markdown_extensions"].count("pymdownx.superfences") == 1

    def test_registers_mermaid_custom_fence(self, plugin):
        config = {"markdown_extensions": [], "mdx_configs": {}}
        result = plugin.on_config(config)
        fences = result["mdx_configs"]["pymdownx.superfences"]["custom_fences"]
        names = {f["name"] for f in fences}
        assert "mermaid" in names

    def test_mermaid_fence_emits_div_not_pre(self, plugin):
        """The mermaid fence must render as <div class="mermaid"> via fence_div_format.

        Material's bundled loader hooks `pre.mermaid`; using a div keeps our
        enhancement script in sole control of rendering.
        """
        from pymdownx.superfences import fence_div_format

        config = {"markdown_extensions": [], "mdx_configs": {}}
        result = plugin.on_config(config)
        fences = result["mdx_configs"]["pymdownx.superfences"]["custom_fences"]
        mermaid = next(f for f in fences if f["name"] == "mermaid")
        assert mermaid["class"] == "mermaid"
        assert mermaid["format"] is fence_div_format

    def test_preserves_existing_custom_fences(self, plugin):
        """A user/workspace-defined custom fence is not clobbered."""
        existing = {"name": "math", "class": "arithmatex", "format": lambda *a, **k: ""}
        config = {
            "markdown_extensions": ["pymdownx.superfences"],
            "mdx_configs": {
                "pymdownx.superfences": {"custom_fences": [existing]},
            },
        }
        result = plugin.on_config(config)
        fences = result["mdx_configs"]["pymdownx.superfences"]["custom_fences"]
        names = {f["name"] for f in fences}
        assert "math" in names
        assert "mermaid" in names

    def test_does_not_duplicate_mermaid_fence_on_second_call(self, plugin):
        config = {"markdown_extensions": [], "mdx_configs": {}}
        result = plugin.on_config(config)
        result = plugin.on_config(result)
        fences = result["mdx_configs"]["pymdownx.superfences"]["custom_fences"]
        names = [f["name"] for f in fences]
        assert names.count("mermaid") == 1

    def test_replaces_existing_mermaid_fence_with_div_format(self, plugin):
        """A workspace mkdocs.yml mermaid fence (pre.mermaid) is forced to div.

        Material's bundled loader hooks pre.mermaid; if a workspace declared the
        fence with fence_code_format we must override it so our enhancement
        script owns rendering.
        """
        from pymdownx.superfences import fence_code_format, fence_div_format

        config = {
            "markdown_extensions": ["pymdownx.superfences"],
            "mdx_configs": {
                "pymdownx.superfences": {
                    "custom_fences": [
                        {
                            "name": "mermaid",
                            "class": "mermaid",
                            "format": fence_code_format,
                        }
                    ]
                }
            },
        }
        result = plugin.on_config(config)
        fences = result["mdx_configs"]["pymdownx.superfences"]["custom_fences"]
        mermaid = [f for f in fences if f["name"] == "mermaid"]
        assert len(mermaid) == 1
        assert mermaid[0]["format"] is fence_div_format


class TestOnConfigMermaidAssets:
    """on_config injects the mermaid CSS and JS assets."""

    def test_injects_mermaid_css(self, plugin):
        config = {"extra_css": [], "extra_javascript": []}
        result = plugin.on_config(config)
        assert "css/qmdc-mermaid.css" in result["extra_css"]

    def test_injects_mermaid_js(self, plugin):
        config = {"extra_css": [], "extra_javascript": []}
        result = plugin.on_config(config)
        assert "js/qmdc-mermaid.js" in result["extra_javascript"]


class TestOnConfigRequiredExtensions:
    """on_config guarantees the markdown extensions the QMD pipeline depends on.

    The converter emits content that needs specific extensions regardless of the
    user's (or auto-generated minimal) mkdocs.yml:
      - admonition + pymdownx.details  → ContentGenerator `??? note` collapsibles
      - md_in_html                     → raw HTML (sidebar, hint popovers, tables)
      - attr_list                      → attribute lists
      - pymdownx.superfences           → mermaid + nested fences
      - pymdownx.emoji                 → emoji (e.g. the 💡 hint icon)
    """

    REQUIRED = [
        "admonition",
        "pymdownx.details",
        "md_in_html",
        "attr_list",
        "pymdownx.superfences",
        "pymdownx.emoji",
    ]

    def test_all_required_extensions_present_from_empty(self, plugin):
        config = {"markdown_extensions": [], "mdx_configs": {}}
        result = plugin.on_config(config)
        for ext in self.REQUIRED:
            assert ext in result["markdown_extensions"], f"missing {ext}"

    def test_works_with_no_markdown_extensions_key(self, plugin):
        """A freshly-generated minimal config has no markdown_extensions key."""
        config = {}
        result = plugin.on_config(config)
        for ext in self.REQUIRED:
            assert ext in result["markdown_extensions"], f"missing {ext}"

    def test_no_duplicates_when_user_already_declared_them(self, plugin):
        config = {
            "markdown_extensions": [
                "admonition",
                "pymdownx.details",
                "md_in_html",
                "attr_list",
                "pymdownx.superfences",
                "pymdownx.emoji",
            ],
            "mdx_configs": {},
        }
        result = plugin.on_config(config)
        for ext in self.REQUIRED:
            assert result["markdown_extensions"].count(ext) == 1, f"duplicate {ext}"

    def test_emoji_configured_with_material_index_and_generator(self, plugin):
        """pymdownx.emoji must be wired to Material's twemoji index + svg generator."""
        from material.extensions import emoji

        config = {"markdown_extensions": [], "mdx_configs": {}}
        result = plugin.on_config(config)
        emoji_cfg = result["mdx_configs"]["pymdownx.emoji"]
        assert emoji_cfg["emoji_index"] is emoji.twemoji
        assert emoji_cfg["emoji_generator"] is emoji.to_svg

    def test_preserves_unrelated_user_extension(self, plugin):
        config = {
            "markdown_extensions": ["toc"],
            "mdx_configs": {"toc": {"permalink": True}},
        }
        result = plugin.on_config(config)
        assert "toc" in result["markdown_extensions"]
        assert result["mdx_configs"]["toc"] == {"permalink": True}

    def test_does_not_clobber_existing_emoji_config(self, plugin):
        """If the user already configured emoji, don't overwrite their choice."""
        sentinel_index = object()
        config = {
            "markdown_extensions": ["pymdownx.emoji"],
            "mdx_configs": {"pymdownx.emoji": {"emoji_index": sentinel_index}},
        }
        result = plugin.on_config(config)
        assert result["mdx_configs"]["pymdownx.emoji"]["emoji_index"] is sentinel_index


class TestMermaidAssetsExist:
    """The bundled asset files exist and contain the enhancement logic.

    The rendering + zoom/pan logic lives in the shared core (qmdc-mermaid-core.js,
    also used by the VS Code preview). qmdc-mermaid.js is a thin MkDocs bootstrap
    that self-hosts mermaid and loads the core.
    """

    def test_mermaid_css_file_exists(self):
        assert (TEMPLATES / "css" / "qmdc-mermaid.css").is_file()

    def test_mermaid_js_file_exists(self):
        assert (TEMPLATES / "js" / "qmdc-mermaid.js").is_file()

    def test_mermaid_core_file_exists(self):
        assert (TEMPLATES / "js" / "qmdc-mermaid-core.js").is_file()

    def test_vendored_mermaid_is_self_hosted(self):
        """mermaid is vendored (no third-party CDN), so the site works offline."""
        assert (TEMPLATES / "js" / "mermaid.min.js").is_file()

    def test_bootstrap_loads_vendored_mermaid_and_core_not_cdn(self):
        """The bootstrap must load the local mermaid + core, never a remote CDN."""
        js = (TEMPLATES / "js" / "qmdc-mermaid.js").read_text(encoding="utf-8")
        assert "mermaid.min.js" in js
        assert "qmdc-mermaid-core.js" in js
        assert "unpkg.com" not in js
        assert "cdn." not in js

    def test_bootstrap_follows_material_palette(self):
        """The bootstrap derives the mermaid theme from Material's palette (Q3)."""
        js = (TEMPLATES / "js" / "qmdc-mermaid.js").read_text(encoding="utf-8")
        assert "data-md-color-scheme" in js
        assert "__qmdcMermaidTheme" in js

    def test_core_disables_use_max_width(self):
        """Natural-size rendering: useMaxWidth must be turned off."""
        js = (TEMPLATES / "js" / "qmdc-mermaid-core.js").read_text(encoding="utf-8")
        assert "useMaxWidth: false" in js

    def test_core_pins_security_level_strict(self):
        """securityLevel must be pinned strict so diagram source can't run script."""
        js = (TEMPLATES / "js" / "qmdc-mermaid-core.js").read_text(encoding="utf-8")
        assert 'securityLevel: "strict"' in js

    def test_core_builds_zoom_pan_viewport(self):
        """Diagrams get a scroll/zoom viewport and toolbar."""
        js = (TEMPLATES / "js" / "qmdc-mermaid-core.js").read_text(encoding="utf-8")
        assert "mermaid-viewport" in js
        assert "mermaid-toolbar" in js

    def test_core_runs_mermaid(self):
        """The core must actually invoke mermaid rendering."""
        js = (TEMPLATES / "js" / "qmdc-mermaid-core.js").read_text(encoding="utf-8")
        assert "mermaid.run" in js or "mermaid.init" in js

    def test_mermaid_css_styles_viewport_and_toolbar(self):
        css = (TEMPLATES / "css" / "qmdc-mermaid.css").read_text(encoding="utf-8")
        assert ".mermaid-viewport" in css
        assert ".mermaid-toolbar" in css


class TestMermaidCoreParity:
    """The shared core must be byte-identical to the VS Code copy (drift guard).

    Single source of truth lives in qmdc-mkdocs; vscode-qmd ships a synced copy
    (run `make mermaid-sync`). If this fails, the copies have drifted.
    """

    def test_core_matches_vscode_copy(self):
        canonical = TEMPLATES / "js" / "qmdc-mermaid-core.js"
        vscode_copy = (
            TEMPLATES.parent.parent.parent
            / "vscode-qmd"
            / "templates"
            / "qmdc-mermaid-core.js"
        )
        if not vscode_copy.exists():
            pytest.skip("vscode-qmd checkout not present alongside qmdc-mkdocs")
        assert canonical.read_text(encoding="utf-8") == vscode_copy.read_text(
            encoding="utf-8"
        ), "qmdc-mermaid-core.js drifted from the VS Code copy — run `make mermaid-sync`"


class TestMermaidEndToEnd:
    """Full build pipeline: mermaid works with the auto-generated minimal config.

    A fresh workspace has no hand-written mkdocs.yml, so generate_mkdocs_config
    emits a minimal config with NO markdown_extensions. The plugin's on_config
    must still register the mermaid superfence at build time, producing a
    <div class="mermaid"> (not <pre>) and loading the renderer assets.
    """

    def _make_ws(self, tmp_path):
        ws = tmp_path / "mmws"
        ws.mkdir()
        (ws / "readme.qmd.md").write_text(
            "# Mermaid WS [[mmws: __Workspace]]\n\n"
            "- version: 1.0\n\n"
            "## Overview [[overview: Section]]\n\n"
            "A diagram:\n\n"
            "```mermaid\ngraph LR\n  a[\"A\"] --> b[\"B\"]\n```\n\n"
            "## Detail [[detail: Section]]\n\n"
            "Body text.\n\n"
            "## Notes Generator [[notesgen: ContentGenerator]]\n\n"
            "- target: detail.content\n\n"
            "### Content [[gencontent: text]]\n\n"
            "Generated body inside a collapsible admonition.\n",
            encoding="utf-8",
        )
        return ws

    def test_generated_config_renders_mermaid_as_div(self, tmp_path):
        from click.testing import CliRunner

        from qmdc_mkdocs.cli import cli

        ws = self._make_ws(tmp_path)
        out = tmp_path / "site"

        runner = CliRunner()
        result = runner.invoke(
            cli, ["--workspace", str(ws), "--output", str(out), "build"]
        )
        assert result.exit_code == 0, f"build failed: {result.output}"

        # A minimal config was generated (no markdown_extensions declared).
        generated = (ws / "mkdocs.yml").read_text(encoding="utf-8")
        assert "markdown_extensions" not in generated

        index = (out / "index.html").read_text(encoding="utf-8")
        # Mermaid fence rendered as a div (plugin-injected superfence), NOT a pre.
        assert '<div class="mermaid">' in index
        assert '<pre class="mermaid">' not in index
        # Renderer assets are referenced.
        assert "js/qmdc-mermaid.js" in index
        assert "css/qmdc-mermaid.css" in index
        # All three JS assets are emitted into the site: the bootstrap, the
        # shared core, and the vendored (self-hosted) mermaid library. The latter
        # two are loaded dynamically by the bootstrap, so they must ship even
        # though they aren't in extra_javascript.
        assert (out / "js" / "qmdc-mermaid.js").is_file()
        assert (out / "js" / "qmdc-mermaid-core.js").is_file()
        assert (out / "js" / "mermaid.min.js").is_file()
        assert (out / "css" / "qmdc-mermaid.css").is_file()

    def test_generated_config_renders_admonition_not_literal(self, tmp_path):
        """ContentGenerator `??? note` must render as a collapsible, not leak as text.

        This needs admonition + pymdownx.details, which the plugin injects even
        when the auto-generated config declares no markdown_extensions. The
        ``???`` form renders as a ``<details class="note">`` collapsible.
        """
        from click.testing import CliRunner

        from qmdc_mkdocs.cli import cli

        ws = self._make_ws(tmp_path)
        out = tmp_path / "site"

        runner = CliRunner()
        result = runner.invoke(
            cli, ["--workspace", str(ws), "--output", str(out), "build"]
        )
        assert result.exit_code == 0, f"build failed: {result.output}"

        index = (out / "index.html").read_text(encoding="utf-8")
        # Rendered as a collapsible details/admonition...
        assert "<details" in index
        assert "<summary>" in index
        # ...not leaked as literal markup.
        assert "??? note" not in index
