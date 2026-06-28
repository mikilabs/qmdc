"""Property-based tests for reference resolution correctness.

**Validates: Requirements 8.1, 8.2, 8.3, 8.4, 8.5**

Property 2: Reference Resolution Correctness
For any [[#id]], [[#Kind:id]], or [[#namespace:id]] reference in a QMD file,
given a WorkspaceDB containing the target object, the converter SHALL produce
a Markdown link [label](relative_path#anchor) where the relative path correctly
navigates from the source file's directory to the target file.

Property 3: Broken Reference Rendering
For any [[#id]] reference whose target does not exist in the WorkspaceDB,
the converter SHALL render it as a <span class="broken-link"> element containing
the original reference text, and the output SHALL NOT contain a Markdown link
for that reference.
"""

from __future__ import annotations

import re
from pathlib import PurePosixPath

import pytest
from hypothesis import given, settings, assume, HealthCheck
from hypothesis import strategies as st

from qmdc_mkdocs.references import resolve_references, _compute_relative_path

# --- Fixture data: non-system objects available in mock_workspace_db ---

# These match the SAMPLE_WORKSPACE_FILES in conftest.py (excluding system types)
FIXTURE_OBJECTS = [
    {
        "id": "users",
        "kind": "Table",
        "label": "Users",
        "file": "storage/tables.qmd.md",
        "namespace": "storage",
    },
    {
        "id": "orders",
        "kind": "Table",
        "label": "Orders",
        "file": "storage/tables.qmd.md",
        "namespace": "storage",
    },
    {
        "id": "get_users",
        "kind": "Endpoint",
        "label": "Get Users",
        "file": "api/endpoints.qmd.md",
        "namespace": "api",
    },
    {
        "id": "get_orders",
        "kind": "Endpoint",
        "label": "Get Orders",
        "file": "api/endpoints.qmd.md",
        "namespace": "api",
    },
]

# Source files we can reference FROM (any .qmd.md file in the workspace)
SOURCE_FILES = [
    "readme.qmd.md",
    "storage/readme.qmd.md",
    "storage/tables.qmd.md",
    "api/readme.qmd.md",
    "api/endpoints.qmd.md",
]

# Markdown link pattern: [label](path#anchor)
MARKDOWN_LINK_RE = re.compile(r"\[([^\]]+)\]\(([^)]+)\)")


# --- Strategies ---

object_strategy = st.sampled_from(FIXTURE_OBJECTS)
source_file_strategy = st.sampled_from(SOURCE_FILES)


# --- Helpers ---

def namespace_for_file(source_file: str) -> str | None:
    """Derive namespace from source file path (first directory component or None)."""
    parts = PurePosixPath(source_file).parts
    if len(parts) > 1:
        return parts[0]
    return None


def verify_relative_path(source_file: str, rel_path: str, target_file: str) -> bool:
    """Verify that following rel_path from source_file's directory reaches target_file.

    Both source and target are workspace-relative .qmd.md paths.
    The rel_path is computed between .md equivalents.
    """
    source_md = source_file.replace(".qmd.md", ".md")
    target_md = target_file.replace(".qmd.md", ".md")

    source_dir = PurePosixPath(source_md).parent
    # Resolve the relative path from source directory
    resolved = (source_dir / rel_path).as_posix()

    # Normalize by resolving .. components
    # PurePosixPath doesn't resolve .., so we do it manually
    parts = resolved.split("/")
    normalized: list[str] = []
    for part in parts:
        if part == "..":
            if normalized:
                normalized.pop()
        elif part != ".":
            normalized.append(part)

    resolved_normalized = "/".join(normalized)
    return resolved_normalized == target_md


def _make_file_objects_with_ref(
    ref_text: str, target_str: str, ref_type: str, source_file: str
) -> tuple[list[str], list[dict]]:
    """Create synthetic lines and file_objects with a single reference.

    Returns (lines, file_objects) suitable for resolve_references().
    """
    line_content = f"- ref: {ref_text}\n"
    lines = [line_content]

    # Compute column positions
    start_col = line_content.index(ref_text)
    end_col = start_col + len(ref_text)

    ns = namespace_for_file(source_file) or ""
    file_objects = [
        {
            "__id": "test_obj",
            "__workspace": "myproject",
            "__namespace": ns,
            "__references": [
                {
                    "line": 1,
                    "start_col": start_col,
                    "end_col": end_col,
                    "raw": ref_text,
                    "target": target_str,
                    "type": ref_type,
                }
            ],
        }
    ]
    return lines, file_objects


# --- Property Tests ---


class TestReferenceResolutionCorrectness:
    """Property 2: Reference Resolution Correctness.

    **Validates: Requirements 8.1, 8.2, 8.3, 8.5**
    """

    @given(
        target=object_strategy,
        source_file=source_file_strategy,
    )
    @settings(max_examples=100, suppress_health_check=[HealthCheck.function_scoped_fixture])
    def test_simple_ref_produces_markdown_link(self, target, source_file, mock_workspace_db):
        """For any valid [[#id]] where target exists, resolve_references produces
        a Markdown link [label](path#anchor).

        **Validates: Requirements 8.1**
        """
        ref_text = f"[[#{target['id']}]]"
        target_str = f"#{target['id']}"
        lines, file_objects = _make_file_objects_with_ref(
            ref_text, target_str, "hash_local", source_file
        )

        result = resolve_references(lines, file_objects, source_file, mock_workspace_db)
        result_text = result[0]

        # The reference should be resolved to a Markdown link
        assert "[[#" not in result_text, (
            f"Reference {ref_text} was not resolved in output: {result_text}"
        )
        assert "broken-link" not in result_text, (
            f"Reference {ref_text} rendered as broken link: {result_text}"
        )

        # Extract the Markdown link
        links = MARKDOWN_LINK_RE.findall(result_text)
        assert len(links) == 1, f"Expected 1 link, got {len(links)} in: {result_text}"

        label, href = links[0]

        # Label should be the target's label
        assert label == target["label"]

        # Href should end with #anchor where anchor is the target id
        assert href.endswith(f"#{target['id']}")

        # The path part (before #) should be a valid relative path
        path_part = href.rsplit("#", 1)[0]
        assert verify_relative_path(source_file, path_part, target["file"]), (
            f"Relative path '{path_part}' from '{source_file}' does not reach "
            f"target '{target['file']}'"
        )

    @given(
        target=object_strategy,
        source_file=source_file_strategy,
    )
    @settings(max_examples=100, suppress_health_check=[HealthCheck.function_scoped_fixture])
    def test_kind_qualified_ref_resolves_correctly(self, target, source_file, mock_workspace_db):
        """Kind-qualified refs [[#Kind:id]] resolve correctly.

        **Validates: Requirements 8.3**
        """
        ref_text = f"[[#{target['kind']}:{target['id']}]]"
        target_str = f"#{target['kind']}:{target['id']}"
        lines, file_objects = _make_file_objects_with_ref(
            ref_text, target_str, "kind", source_file
        )

        result = resolve_references(lines, file_objects, source_file, mock_workspace_db)
        result_text = result[0]

        # Should resolve to a link (not broken)
        assert "broken-link" not in result_text, (
            f"Kind-qualified ref {ref_text} rendered as broken: {result_text}"
        )
        assert "[[#" not in result_text, (
            f"Kind-qualified ref {ref_text} was not resolved: {result_text}"
        )

        # Extract and verify the link
        links = MARKDOWN_LINK_RE.findall(result_text)
        assert len(links) == 1, f"Expected 1 link, got {len(links)} in: {result_text}"

        label, href = links[0]
        assert label == target["label"]
        assert href.endswith(f"#{target['id']}")

        path_part = href.rsplit("#", 1)[0]
        assert verify_relative_path(source_file, path_part, target["file"]), (
            f"Relative path '{path_part}' from '{source_file}' does not reach "
            f"target '{target['file']}'"
        )

    @given(
        target=object_strategy,
        source_file=source_file_strategy,
    )
    @settings(max_examples=100, suppress_health_check=[HealthCheck.function_scoped_fixture])
    def test_namespace_qualified_ref_resolves_correctly(
        self, target, source_file, mock_workspace_db
    ):
        """Namespace-qualified refs [[#ns:id]] resolve correctly.

        **Validates: Requirements 8.2**
        """
        # Only test objects that have a namespace
        assume(target["namespace"] is not None)

        ref_text = f"[[#{target['namespace']}:{target['id']}]]"
        target_str = f"#{target['namespace']}:{target['id']}"
        lines, file_objects = _make_file_objects_with_ref(
            ref_text, target_str, "namespace", source_file
        )

        result = resolve_references(lines, file_objects, source_file, mock_workspace_db)
        result_text = result[0]

        # Should resolve to a link (not broken)
        assert "broken-link" not in result_text, (
            f"Namespace-qualified ref {ref_text} rendered as broken: {result_text}"
        )
        assert "[[#" not in result_text, (
            f"Namespace-qualified ref {ref_text} was not resolved: {result_text}"
        )

        # Extract and verify the link
        links = MARKDOWN_LINK_RE.findall(result_text)
        assert len(links) == 1, f"Expected 1 link, got {len(links)} in: {result_text}"

        label, href = links[0]
        assert label == target["label"]
        assert href.endswith(f"#{target['id']}")

        path_part = href.rsplit("#", 1)[0]
        assert verify_relative_path(source_file, path_part, target["file"]), (
            f"Relative path '{path_part}' from '{source_file}' does not reach "
            f"target '{target['file']}'"
        )

    @given(
        target=object_strategy,
        source_file=source_file_strategy,
    )
    @settings(max_examples=100, suppress_health_check=[HealthCheck.function_scoped_fixture])
    def test_relative_path_is_valid_posix_path(self, target, source_file, mock_workspace_db):
        """The generated link's path is a valid relative path (no absolute paths,
        no backslashes, uses ../ for parent traversal).

        **Validates: Requirements 8.5**
        """
        ref_text = f"[[#{target['id']}]]"
        target_str = f"#{target['id']}"
        lines, file_objects = _make_file_objects_with_ref(
            ref_text, target_str, "hash_local", source_file
        )

        result = resolve_references(lines, file_objects, source_file, mock_workspace_db)
        result_text = result[0]

        links = MARKDOWN_LINK_RE.findall(result_text)
        assert len(links) == 1

        _, href = links[0]
        path_part = href.rsplit("#", 1)[0]

        # Path must be relative (no leading /)
        assert not path_part.startswith("/"), (
            f"Path should be relative, got absolute: {path_part}"
        )

        # Path must not contain backslashes (POSIX only)
        assert "\\" not in path_part, (
            f"Path contains backslashes: {path_part}"
        )

        # Path must end with .md extension
        assert path_part.endswith(".md"), (
            f"Path should end with .md: {path_part}"
        )

        # Path components should only be valid directory/file names or ..
        parts = path_part.split("/")
        for part in parts:
            assert part == ".." or re.match(r"^[a-zA-Z0-9_\-\.]+$", part), (
                f"Invalid path component '{part}' in path: {path_part}"
            )


# --- Strategies for Property 3: Broken Reference Rendering ---

# IDs that do NOT exist in the sample workspace
# We generate arbitrary identifiers that won't collide with fixture objects
_EXISTING_IDS = {obj["id"] for obj in FIXTURE_OBJECTS}

# Strategy for generating non-existent IDs
nonexistent_id_strategy = st.text(
    alphabet=st.characters(whitelist_categories=("Ll", "Lu", "Nd"), whitelist_characters="_"),
    min_size=3,
    max_size=20,
).filter(lambda s: s[0:1].isalpha() and s not in _EXISTING_IDS)

# Strategy for non-existent namespace-qualified refs
nonexistent_ns_strategy = st.text(
    alphabet=st.characters(whitelist_categories=("Ll",), whitelist_characters="_"),
    min_size=3,
    max_size=12,
).filter(lambda s: s[0:1].isalpha() and s not in ("storage", "api"))

# Strategy for non-existent kind-qualified refs
nonexistent_kind_strategy = st.text(
    alphabet=st.characters(whitelist_categories=("Lu", "Ll"), whitelist_characters=""),
    min_size=3,
    max_size=15,
).filter(lambda s: s[0:1].isupper() and s not in ("Table", "Endpoint"))


class TestBrokenReferenceRendering:
    """Property 3: Broken Reference Rendering.

    **Validates: Requirements 8.4**

    For any [[#id]] reference whose target does not exist in the WorkspaceDB,
    the converter SHALL render it as a <span class="broken-link"> element
    containing the original reference text, and the output SHALL NOT contain
    a Markdown link for that reference.
    """

    @given(
        nonexistent_id=nonexistent_id_strategy,
        source_file=source_file_strategy,
    )
    @settings(max_examples=100, suppress_health_check=[HealthCheck.function_scoped_fixture])
    def test_simple_broken_ref_renders_as_span(
        self, nonexistent_id, source_file, mock_workspace_db
    ):
        """For any [[#id]] where target does NOT exist, resolve_references renders
        a <span class="broken-link"> containing the original reference text.

        **Validates: Requirements 8.4**
        """
        ref_text = f"[[#{nonexistent_id}]]"
        target_str = f"#{nonexistent_id}"
        lines, file_objects = _make_file_objects_with_ref(
            ref_text, target_str, "hash_local", source_file
        )

        result = resolve_references(lines, file_objects, source_file, mock_workspace_db)
        result_text = result[0]

        # Must contain broken-link span
        assert 'class="broken-link"' in result_text, (
            f"Broken ref {ref_text} was not rendered with broken-link class: {result_text}"
        )

        # Must contain the original reference text inside the span
        assert f'<span class="broken-link">{ref_text}</span>' in result_text, (
            f"Broken ref span does not contain original text '{ref_text}': {result_text}"
        )

        # Must NOT contain a Markdown link
        links = MARKDOWN_LINK_RE.findall(result_text)
        assert len(links) == 0, (
            f"Broken ref {ref_text} should not produce a Markdown link, "
            f"but found: {links} in: {result_text}"
        )

    @given(
        nonexistent_ns=nonexistent_ns_strategy,
        nonexistent_id=nonexistent_id_strategy,
        source_file=source_file_strategy,
    )
    @settings(max_examples=100, suppress_health_check=[HealthCheck.function_scoped_fixture])
    def test_namespace_broken_ref_renders_as_span(
        self, nonexistent_ns, nonexistent_id, source_file, mock_workspace_db
    ):
        """For any [[#ns:id]] where the namespace:id combination does NOT exist,
        resolve_references renders a <span class="broken-link">.

        **Validates: Requirements 8.4**
        """
        ref_text = f"[[#{nonexistent_ns}:{nonexistent_id}]]"
        target_str = f"#{nonexistent_ns}:{nonexistent_id}"
        lines, file_objects = _make_file_objects_with_ref(
            ref_text, target_str, "namespace", source_file
        )

        result = resolve_references(lines, file_objects, source_file, mock_workspace_db)
        result_text = result[0]

        # Must contain broken-link span with original text
        assert f'<span class="broken-link">{ref_text}</span>' in result_text, (
            f"Broken namespace ref {ref_text} not rendered correctly: {result_text}"
        )

        # Must NOT contain a Markdown link
        links = MARKDOWN_LINK_RE.findall(result_text)
        assert len(links) == 0, (
            f"Broken namespace ref {ref_text} should not produce a Markdown link, "
            f"but found: {links} in: {result_text}"
        )

    @given(
        nonexistent_kind=nonexistent_kind_strategy,
        nonexistent_id=nonexistent_id_strategy,
        source_file=source_file_strategy,
    )
    @settings(max_examples=100, suppress_health_check=[HealthCheck.function_scoped_fixture])
    def test_kind_broken_ref_renders_as_span(
        self, nonexistent_kind, nonexistent_id, source_file, mock_workspace_db
    ):
        """For any [[#Kind:id]] where the kind:id combination does NOT exist,
        resolve_references renders a <span class="broken-link">.

        **Validates: Requirements 8.4**
        """
        ref_text = f"[[#{nonexistent_kind}:{nonexistent_id}]]"
        target_str = f"#{nonexistent_kind}:{nonexistent_id}"
        lines, file_objects = _make_file_objects_with_ref(
            ref_text, target_str, "kind", source_file
        )

        result = resolve_references(lines, file_objects, source_file, mock_workspace_db)
        result_text = result[0]

        # Must contain broken-link span with original text
        assert f'<span class="broken-link">{ref_text}</span>' in result_text, (
            f"Broken kind ref {ref_text} not rendered correctly: {result_text}"
        )

        # Must NOT contain a Markdown link
        links = MARKDOWN_LINK_RE.findall(result_text)
        assert len(links) == 0, (
            f"Broken kind ref {ref_text} should not produce a Markdown link, "
            f"but found: {links} in: {result_text}"
        )

    @given(
        nonexistent_id=nonexistent_id_strategy,
        source_file=source_file_strategy,
    )
    @settings(max_examples=100, suppress_health_check=[HealthCheck.function_scoped_fixture])
    def test_broken_ref_preserves_original_text_exactly(
        self, nonexistent_id, source_file, mock_workspace_db
    ):
        """The broken-link span must contain the EXACT original reference text
        (including brackets), not a modified version.

        **Validates: Requirements 8.4**
        """
        ref_text = f"[[#{nonexistent_id}]]"
        target_str = f"#{nonexistent_id}"
        lines, file_objects = _make_file_objects_with_ref(
            ref_text, target_str, "hash_local", source_file
        )

        result = resolve_references(lines, file_objects, source_file, mock_workspace_db)
        result_text = result[0]

        # Extract content between broken-link span tags
        span_start = '<span class="broken-link">'
        span_end = "</span>"
        start_idx = result_text.find(span_start)
        assert start_idx != -1, f"No broken-link span found in: {result_text}"

        content_start = start_idx + len(span_start)
        end_idx = result_text.find(span_end, content_start)
        assert end_idx != -1, f"No closing </span> found in: {result_text}"

        span_content = result_text[content_start:end_idx]
        assert span_content == ref_text, (
            f"Span content '{span_content}' does not match original ref '{ref_text}'"
        )
