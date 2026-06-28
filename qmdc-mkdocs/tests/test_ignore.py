"""Tests for .qmdc-mkdocs.ignore loading and matching (ignore.py)."""

from qmdc_mkdocs.ignore import is_excluded, is_ignored, load_siteignore


class TestIsIgnored:
    """is_ignored matches workspace-relative paths against gitignore-ish patterns."""

    def test_full_path_glob(self):
        assert is_ignored("tracking/x.md", ["tracking/x.md"])
        assert not is_ignored("tracking/y.md", ["tracking/x.md"])

    def test_dir_recursive_star_star(self):
        assert is_ignored("tracking/x.md", ["tracking/**"])
        assert is_ignored("tracking/done/x.md", ["tracking/**"])

    def test_dir_star_is_also_recursive(self):
        """Documented behavior: `tracking/*` matches at any depth, like `**`.

        fnmatch '*' crosses '/', and the directory-prefix branch matches
        arbitrarily deep. There is intentionally no shallow-only form.
        """
        assert is_ignored("tracking/x.md", ["tracking/*"])
        assert is_ignored("tracking/done/x.md", ["tracking/*"])

    def test_filename_pattern_any_depth(self):
        assert is_ignored("a/b/c/notes.sop.md", ["*.sop.md"])
        assert is_ignored("notes.sop.md", ["*.sop.md"])
        assert not is_ignored("notes.md", ["*.sop.md"])

    def test_no_patterns(self):
        assert not is_ignored("anything.md", [])


class TestIsExcluded:
    """is_excluded is the single source of truth for page exclusion + dead links."""

    def test_empty_patterns_never_excludes(self):
        assert not is_excluded("tracking/x.md", [], namespace_prefix="tracking")

    def test_full_path_match(self):
        assert is_excluded("tracking/done/x.md", ["tracking/**"])

    def test_namespace_stripped_match(self):
        """A namespace-relative pattern matches the prefix-stripped path.

        Building the 'tracking' namespace with a pattern 'done/**' must exclude
        'tracking/done/x.md' (its stripped path 'done/x.md' matches) — so the
        converter drop and the reference dead-link agree.
        """
        assert is_excluded(
            "tracking/done/x.md", ["done/**"], namespace_prefix="tracking"
        )

    def test_no_false_exclude_without_prefix(self):
        """Without the namespace prefix, a namespace-relative pattern shouldn't match."""
        assert not is_excluded("tracking/done/x.md", ["done/**"])


class TestLoadSiteignore:
    """load_siteignore reads ONLY the workspace root file (no parent walking)."""

    def test_reads_root_patterns(self, tmp_path):
        (tmp_path / ".qmdc-mkdocs.ignore").write_text(
            "# a comment\ntracking/**\n\n*.sop.md\n", encoding="utf-8"
        )
        patterns = load_siteignore(tmp_path)
        assert patterns == ["tracking/**", "*.sop.md"]

    def test_missing_file_returns_empty(self, tmp_path):
        assert load_siteignore(tmp_path) == []

    def test_does_not_walk_into_parent(self, tmp_path):
        """A parent dir's ignore file must NOT influence the workspace build."""
        (tmp_path / ".qmdc-mkdocs.ignore").write_text("parent/**\n", encoding="utf-8")
        ws = tmp_path / "ws"
        ws.mkdir()
        (ws / ".qmdc-mkdocs.ignore").write_text("local/**\n", encoding="utf-8")
        patterns = load_siteignore(ws)
        assert patterns == ["local/**"]
        assert "parent/**" not in patterns
