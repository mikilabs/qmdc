"""MkDocs YAML generation and theme configuration."""

import re
from pathlib import Path

import yaml


def generate_mkdocs_config(
    workspace: Path,
    nav: list,
    site_name: str = "QMDC Documentation",
) -> None:
    """Generate minimal mkdocs.yml ONLY if one doesn't already exist.

    This is the user-owned config in the workspace root. It contains theme
    preferences, palette, plugins, etc. At build time, the CLI generates a
    temporary build config that extends this with the correct docs_dir,
    site_dir, and custom_dir paths pointing into the temp directory.

    The generated config includes:
    - site_name (from workspace label or default)
    - theme.name: material with dark slate palette as sensible default
    - plugins: [search, qmdc]
    - nav: auto-generated navigation tree
    """
    mkdocs_yml = workspace / "mkdocs.yml"
    if mkdocs_yml.exists():
        return  # Never overwrite user config

    config = {
        "site_name": site_name,
        "theme": {
            "name": "material",
            "palette": {"scheme": "slate", "primary": "indigo"},
            # navigation.footer enables Material's prev/next footer bar; the qmdc
            # plugin's semantic `next` link renders into it (see partials/footer.html).
            "features": ["navigation.footer"],
        },
        "plugins": ["search", "glightbox", "qmdc"],
        "nav": nav,
    }

    mkdocs_yml.write_text(yaml.dump(config, default_flow_style=False, sort_keys=False))


def generate_build_config(
    workspace: Path,
    tmpdir: Path,
    output: Path,
) -> Path:
    """Generate a temporary mkdocs.yml for the actual build.

    Reads the user's mkdocs.yml as raw text, appends build-time paths
    (docs_dir, site_dir, custom_dir) as YAML overrides. This preserves
    !!python/name tags and other special YAML constructs that safe_load
    cannot handle.

    Returns path to the temporary config file.
    """
    user_config_path = workspace / "mkdocs.yml"
    if user_config_path.exists():
        raw_config = user_config_path.read_text(encoding="utf-8")
    else:
        raw_config = "site_name: QMDC Documentation\ntheme:\n  name: material\n"

    custom_dir = tmpdir / "overrides"

    # Inject the build-time custom_dir (theme.custom_dir) so MkDocs finds the
    # scaffolded overrides. Three cases:
    #   1. custom_dir already present  -> replace its value
    #   2. a `theme:` block exists     -> insert custom_dir into it
    #   3. no `theme:` block at all    -> append a fresh theme block (a bare
    #      `custom_dir:` at top level would be ignored by MkDocs)
    has_theme_block = re.search(r"^theme:\s*$|^theme:\s*\n", raw_config, re.MULTILINE) is not None
    if "custom_dir:" in raw_config:
        # Replace existing custom_dir
        raw_config = re.sub(
            r"custom_dir:.*",
            f"custom_dir: {custom_dir}",
            raw_config,
        )
        theme_override = ""
    elif has_theme_block:
        # Add custom_dir after the theme: line (inside the theme block)
        raw_config = re.sub(
            r"(theme:\s*\n(?:[ \t]+\S.*\n)*)",
            lambda m: m.group(0) + f"  custom_dir: {custom_dir}\n",
            raw_config,
            count=1,
        )
        theme_override = ""
    else:
        # No theme block — append a complete one in the overrides section
        theme_override = f"theme:\n  name: material\n  custom_dir: {custom_dir}\n"

    # Append build-time overrides (these take precedence in MkDocs)
    overrides = (
        f"\n# Build-time overrides (auto-generated)\n"
        f"{theme_override}"
        f"docs_dir: {tmpdir / 'docs'}\n"
        f"site_dir: {output}\n"
    )

    build_config_path = tmpdir / "mkdocs.yml"
    build_config_path.write_text(raw_config + overrides, encoding="utf-8")
    return build_config_path


def generate_nav_file(workspace: Path, nav: list) -> None:
    """Write nav.yml to workspace root — a reference nav tree the user can copy into mkdocs.yml."""
    nav_yml = workspace / "nav.yml"
    nav_yml.write_text(yaml.dump(nav, default_flow_style=False, sort_keys=False))
