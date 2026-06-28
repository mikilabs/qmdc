"""Shared .qmdc-mkdocs.ignore pattern loading and matching."""

import fnmatch
from pathlib import Path, PurePosixPath


def load_siteignore(workspace: Path) -> list[str]:
    """Load .qmdc-mkdocs.ignore patterns from the workspace root only.

    Gitignore-style patterns (one per line). Lines starting with # are comments.
    Empty lines are skipped.

    Note: only the workspace root's ``.qmdc-mkdocs.ignore`` is read. We do NOT
    walk up into parent directories — which pages get published must depend only
    on the workspace being built, not on unrelated files elsewhere on the machine
    (a footgun on shared / CI checkouts).
    """
    patterns: list[str] = []
    ignore_file = workspace.resolve() / ".qmdc-mkdocs.ignore"
    if ignore_file.exists():
        for raw_line in ignore_file.read_text(encoding="utf-8").splitlines():
            line = raw_line.strip()
            if not line or line.startswith("#"):
                continue
            if line not in patterns:
                patterns.append(line)
    return patterns


def is_ignored(source_file: str, patterns: list[str]) -> bool:
    """Check whether a workspace-relative path matches any ignore pattern.

    Matching is recursive by design — a directory pattern excludes everything
    beneath it, at any depth:

    - ``tracking/**`` and ``tracking/*`` both match ``tracking/x.md`` AND
      ``tracking/done/x.md``. (``fnmatch`` ``*`` crosses ``/``, and the
      directory-prefix branch below matches arbitrarily deep.) There is
      intentionally no shallow-only form.
    - ``*.sop.md`` (no ``/``) matches by filename at any depth.
    - Any other glob is matched against the full relative path via ``fnmatch``.

    Prefer ``is_excluded`` for the build pipeline: it also handles the
    namespace-prefix-stripped path so exclusion and link-resolution agree.
    """
    for pattern in patterns:
        # Match against the full path
        if fnmatch.fnmatch(source_file, pattern):
            return True
        # Directory pattern: tracking/** or tracking/* → everything under tracking/
        if pattern.endswith("/**") or pattern.endswith("/*"):
            dir_prefix = pattern.rstrip("/*")
            if source_file.startswith(dir_prefix + "/"):
                return True
        # Match against just the filename
        if "/" not in pattern and fnmatch.fnmatch(PurePosixPath(source_file).name, pattern):
            return True
    return False


def is_excluded(
    source_file: str,
    patterns: list[str],
    namespace_prefix: str | None = None,
) -> bool:
    """Whether a page is excluded from the built site.

    A file is excluded if it matches an ignore pattern as its full
    workspace-relative path, OR — when building a single namespace — as its
    namespace-prefix-stripped path (so a namespace-relative pattern like
    ``done/**`` works while building ``tracking/`` with prefix ``tracking``).

    This is the single source of truth used by BOTH page generation (converter)
    and reference resolution (references / regex fallback / graph sidebar), so a
    page that is dropped is never linked to with a live href (which would 404 /
    warn in MkDocs).
    """
    if not patterns:
        return False
    if is_ignored(source_file, patterns):
        return True
    if namespace_prefix:
        prefix = namespace_prefix + "/"
        if source_file.startswith(prefix):
            stripped = source_file[len(prefix):]
            if stripped != source_file and is_ignored(stripped, patterns):
                return True
    return False
