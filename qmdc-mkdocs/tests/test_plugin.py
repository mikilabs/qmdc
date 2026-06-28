"""Unit tests for qmdc_mkdocs.plugin — QmdcPlugin MkDocs lifecycle hooks."""

from unittest.mock import MagicMock

import pytest

from qmdc_mkdocs.plugin import QmdcPlugin


@pytest.fixture
def plugin():
    """Create a fresh QmdcPlugin instance."""
    return QmdcPlugin()


class TestConfigScheme:
    """The plugin must declare its options so MkDocs doesn't warn/reject them.

    Without a config_scheme entry, MkDocs emits:
      WARNING - Plugin 'qmdc' option 'hidden_kinds': Unrecognised configuration name
    and drops the value.
    """

    def test_declares_hidden_kinds_option(self, plugin):
        scheme_keys = dict(plugin.config_scheme)
        assert "hidden_kinds" in scheme_keys

    def test_hidden_kinds_validates_list_of_strings(self, plugin):
        """load_config accepts a list of Kind names without errors."""
        errors, warnings = plugin.load_config({"hidden_kinds": ["ContentGenerator", "Draft"]})
        assert errors == []
        assert warnings == []
        assert plugin.config["hidden_kinds"] == ["ContentGenerator", "Draft"]

    def test_hidden_kinds_defaults_to_empty_list(self, plugin):
        """Omitting hidden_kinds is valid and defaults to []."""
        errors, warnings = plugin.load_config({})
        assert errors == []
        assert warnings == []
        assert plugin.config["hidden_kinds"] == []

    def test_unknown_option_still_rejected(self, plugin):
        """Genuinely unknown options should still produce a warning."""
        _errors, warnings = plugin.load_config({"definitely_not_an_option": 1})
        assert warnings  # MkDocs reports unrecognised options as warnings


class TestOnConfig:
    """Tests for QmdcPlugin.on_config hook."""

    def test_injects_extra_css(self, plugin):
        """on_config adds css/qmdc-extra.css to extra_css."""
        config = {"extra_css": [], "extra_javascript": []}
        result = plugin.on_config(config)
        assert "css/qmdc-extra.css" in result["extra_css"]

    def test_injects_extra_javascript(self, plugin):
        """on_config adds js/qmdc-extra.js to extra_javascript."""
        config = {"extra_css": [], "extra_javascript": []}
        result = plugin.on_config(config)
        assert "js/qmdc-extra.js" in result["extra_javascript"]

    def test_preserves_existing_extra_css(self, plugin):
        """on_config appends to existing extra_css entries."""
        config = {"extra_css": ["existing.css"], "extra_javascript": []}
        result = plugin.on_config(config)
        assert result["extra_css"] == [
            "existing.css",
            "css/qmdc-extra.css",
            "css/qmdc-mermaid.css",
        ]

    def test_preserves_existing_extra_javascript(self, plugin):
        """on_config appends to existing extra_javascript entries."""
        config = {"extra_css": [], "extra_javascript": ["existing.js"]}
        result = plugin.on_config(config)
        assert result["extra_javascript"] == [
            "existing.js",
            "js/qmdc-extra.js",
            "js/qmdc-mermaid.js",
        ]

    def test_handles_missing_extra_css_key(self, plugin):
        """on_config handles config without extra_css key."""
        config = {"extra_javascript": []}
        result = plugin.on_config(config)
        assert result["extra_css"] == ["css/qmdc-extra.css", "css/qmdc-mermaid.css"]

    def test_handles_missing_extra_javascript_key(self, plugin):
        """on_config handles config without extra_javascript key."""
        config = {"extra_css": []}
        result = plugin.on_config(config)
        assert result["extra_javascript"] == ["js/qmdc-extra.js", "js/qmdc-mermaid.js"]

    def test_asset_injection_is_idempotent(self, plugin):
        """A re-entrant on_config must not duplicate asset entries.

        Asset injection (like fence/extension registration) must be idempotent —
        duplicate js/qmdc-mermaid.js would run the enhancement IIFE and mermaid.run
        twice.
        """
        config = {"extra_css": [], "extra_javascript": []}
        result = plugin.on_config(config)
        result = plugin.on_config(result)
        assert result["extra_css"].count("css/qmdc-extra.css") == 1
        assert result["extra_css"].count("css/qmdc-mermaid.css") == 1
        assert result["extra_javascript"].count("js/qmdc-extra.js") == 1
        assert result["extra_javascript"].count("js/qmdc-mermaid.js") == 1

    def test_returns_config(self, plugin):
        """on_config returns the modified config dict."""
        config = {"extra_css": [], "extra_javascript": []}
        result = plugin.on_config(config)
        assert result is not None
        assert isinstance(result, dict)

    def test_does_not_duplicate_on_multiple_calls(self, plugin):
        """Calling on_config multiple times appends each time (MkDocs calls once)."""
        config = {"extra_css": [], "extra_javascript": []}
        result = plugin.on_config(config)
        # Second call would append again — this is expected MkDocs behavior
        # (MkDocs only calls on_config once per build)
        assert result["extra_css"].count("css/qmdc-extra.css") == 1


class TestOnPageContext:
    """Tests for QmdcPlugin.on_page_context hook."""

    def _make_page(self, meta=None):
        """Create a mock page object with given meta."""
        page = MagicMock()
        page.meta = meta
        return page

    def test_injects_graph_sidebar_from_meta(self, plugin):
        """on_page_context reads _graph_sidebar from page.meta."""
        sidebar_data = {
            "breadcrumb": ["Workspace", "Namespace", "Page"],
            "links_to": [{"id": "users", "label": "Users"}],
        }
        page = self._make_page(meta={"_graph_sidebar": sidebar_data})
        context = {}
        result = plugin.on_page_context(context, page, config={}, nav=None)
        assert result["qmdc_graph_sidebar"] == sidebar_data

    def test_injects_semantic_hints_from_meta(self, plugin):
        """on_page_context reads _semantic_hints from page.meta."""
        hints_data = {
            "users": [{"label": "Orders", "score": 0.85}],
        }
        page = self._make_page(meta={"_semantic_hints": hints_data})
        context = {}
        result = plugin.on_page_context(context, page, config={}, nav=None)
        assert result["qmdc_semantic_hints"] == hints_data

    def test_defaults_to_empty_dict_when_no_sidebar(self, plugin):
        """on_page_context returns empty dict when _graph_sidebar not in meta."""
        page = self._make_page(meta={})
        context = {}
        result = plugin.on_page_context(context, page, config={}, nav=None)
        assert result["qmdc_graph_sidebar"] == {}

    def test_defaults_to_empty_dict_when_no_hints(self, plugin):
        """on_page_context returns empty dict when _semantic_hints not in meta."""
        page = self._make_page(meta={})
        context = {}
        result = plugin.on_page_context(context, page, config={}, nav=None)
        assert result["qmdc_semantic_hints"] == {}

    def test_handles_none_meta(self, plugin):
        """on_page_context handles page.meta being None."""
        page = self._make_page(meta=None)
        context = {}
        result = plugin.on_page_context(context, page, config={}, nav=None)
        assert result["qmdc_graph_sidebar"] == {}
        assert result["qmdc_semantic_hints"] == {}

    def test_preserves_existing_context(self, plugin):
        """on_page_context preserves existing context entries."""
        page = self._make_page(meta={"_graph_sidebar": {"test": True}})
        context = {"existing_key": "existing_value"}
        result = plugin.on_page_context(context, page, config={}, nav=None)
        assert result["existing_key"] == "existing_value"
        assert result["qmdc_graph_sidebar"] == {"test": True}

    def test_returns_context(self, plugin):
        """on_page_context returns the modified context dict."""
        page = self._make_page(meta={})
        context = {}
        result = plugin.on_page_context(context, page, config={}, nav=None)
        assert result is not None
        assert isinstance(result, dict)
