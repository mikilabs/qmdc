"""Click CLI entry point — init, build, serve, regenerate subcommands."""

from __future__ import annotations

import contextlib
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import TYPE_CHECKING

import click

from .config import generate_build_config, generate_mkdocs_config, generate_nav_file
from .converter import convert_workspace
from .database import WorkspaceLoadError, load_workspace
from .hints import load_hints
from .navigation import generate_nav
from .validation import validate_workspace

if TYPE_CHECKING:
    from .database import WorkspaceData


def _load_or_exit(workspace: Path) -> WorkspaceData:
    """Load a workspace, converting WorkspaceLoadError into a clean CLI error.

    Keeps the SystemExit/exit-code policy at the CLI boundary instead of in the
    library (database.load_workspace raises a domain exception).
    """
    try:
        return load_workspace(workspace)
    except WorkspaceLoadError as exc:
        raise click.ClickException(str(exc)) from exc




def _read_hidden_kinds(workspace: Path) -> list[str]:
    """Read hidden_kinds from the qmdc plugin config in mkdocs.yml.

    NOTE: this must stay aligned with ``QmdcPlugin.config_scheme`` in plugin.py
    (the option name and shape). The plugin declares ``hidden_kinds`` so MkDocs
    doesn't warn about an unrecognised option, but the value the build actually
    consumes is read here — the converter runs in this process, before/outside
    the mkdocs subprocess that parses the plugin config.

    Limitation: this is a regex scrape, so it only matches a flow-style list
    (``hidden_kinds: [A, B]``); a block-style YAML list yields ``[]``.
    """
    import re as _re
    mkdocs_yml = workspace / 'mkdocs.yml'
    if not mkdocs_yml.exists():
        return []
    text = mkdocs_yml.read_text(encoding='utf-8')
    # Look for hidden_kinds: [Kind1, Kind2] in the qmdc plugin section
    m = _re.search(r'hidden_kinds:\s*\[([^\]]*)\]', text)
    if m:
        return [k.strip() for k in m.group(1).split(',') if k.strip()]
    return []

def _get_site_name(ws_data: WorkspaceData, ns_prefix: str | None) -> str:
    """Get site name from parsed workspace data.

    If building a namespace: use the first object's __label from the namespace readme.
    If building a workspace: use the __Workspace object's __label.
    Falls back to directory name or 'QMDC Documentation'.
    """
    if ns_prefix:
        # Find the namespace readme object
        ns_readme = ns_prefix + "/readme.qmd.md"
        for obj in ws_data.result.objects:
            if obj.get("__file") == ns_readme and obj.get("__label"):
                return obj["__label"]
        return ns_prefix.replace("-", " ").replace("_", " ").title()
    else:
        # Find workspace object
        for obj in ws_data.result.objects:
            if obj.get("__kind") == "__Workspace" and obj.get("__label"):
                return obj["__label"]
        return "QMDC Documentation"


def _resolve_workspace_and_prefix(ws: Path) -> tuple[Path, str | None]:
    """Resolve workspace root and namespace prefix.

    If ws is a workspace root (parse_workspace finds it directly), returns (ws, None).
    If ws is a namespace within a workspace, walks up to find the root and returns
    (root, relative_prefix) so only files under the prefix are converted.
    """
    from qmdc.workspace import parse_workspace

    # Try parsing from ws directly — if it works and has a workspace_id, it's a root
    try:
        result = parse_workspace(str(ws))
        if result.workspace_id:
            # Check if any files are directly in ws (not in a subdirectory relative to ws)
            # If the workspace root is ws itself, files will have paths without a prefix
            ws_readme = ws / "readme.qmd.md"
            if ws_readme.exists() and "__Workspace" in ws_readme.read_text(encoding="utf-8"):
                return ws, None
    except Exception:
        pass

    # Walk up to find workspace root
    current = ws.parent
    while current != current.parent:
        readme = current / "readme.qmd.md"
        if readme.exists():
            try:
                content = readme.read_text(encoding="utf-8")
                if "__Workspace" in content:
                    # Found workspace root
                    prefix = str(ws.relative_to(current))
                    return current, prefix
            except Exception:
                pass
        current = current.parent

    # No workspace root found — treat ws as standalone
    return ws, None


@click.group()
@click.option("--workspace", "-w", default=".", type=click.Path(exists=True))
@click.option("--output", "-o", default=None, type=click.Path())
@click.pass_context
def cli(ctx: click.Context, workspace: str, output: str | None) -> None:
    """Build documentation sites from QMDC workspaces.

    --workspace can point to either:
    - A workspace root (directory with readme.qmd.md containing __Workspace)
    - A namespace directory within a workspace (will find root automatically,
      but only build files within this namespace)
    """
    ws = Path(workspace).resolve()
    ctx.ensure_object(dict)
    ctx.obj["workspace"] = ws
    ctx.obj["output"] = Path(output).resolve() if output else ws / "_site"
    # namespace_prefix: if set, only convert files under this prefix
    ctx.obj["namespace_prefix"] = None


@cli.command()
@click.pass_context
def init(ctx: click.Context) -> None:
    """Scaffold MkDocs project from workspace."""
    ws: Path = ctx.obj["workspace"]

    # Load workspace data to generate navigation
    ws_data = _load_or_exit(ws)
    try:
        nav = generate_nav(ws_data)

        # Get site name from workspace label
        ws_rows = ws_data.query(
            "SELECT __label FROM objects WHERE __kind = '__Workspace' LIMIT 1"
        )
        site_name = ws_rows[0]["__label"] if ws_rows else "QMDC Documentation"

        # Generate mkdocs.yml only if absent (never overwrite)
        generate_mkdocs_config(ws, nav, site_name)

        # Always write nav.yml reference file
        generate_nav_file(ws, nav)
    finally:
        ws_data.close()

    click.echo("Init complete.")


@cli.command()
@click.pass_context
def build(ctx: click.Context) -> None:
    """Full pipeline: init + convert + mkdocs build.

    Search is provided by MkDocs Material's built-in search plugin; no separate
    indexing step is run.
    """
    ws: Path = ctx.obj["workspace"]
    output: Path = ctx.obj["output"]

    # Resolve workspace root and namespace prefix
    ws_root, ns_prefix = _resolve_workspace_and_prefix(ws)
    if ns_prefix:
        click.echo(f"Building namespace '{ns_prefix}' from workspace '{ws_root}'", err=True)

    # Load workspace data (always from root for full graph)
    ws_data = _load_or_exit(ws_root)
    tmpdir = Path(tempfile.mkdtemp(prefix="qmdc-mkdocs-"))
    try:
        # Load semantic hints
        hints = load_hints(ws_root)

        # Generate navigation and init config
        nav = generate_nav(ws_data, namespace_prefix=ns_prefix)

        # Determine site_name
        site_name = _get_site_name(ws_data, ns_prefix)

        generate_mkdocs_config(ws, nav, site_name)
        generate_nav_file(ws, nav)

        # Validate workspace (non-blocking)
        validate_workspace(ws_root)

        # Scaffold overrides: copy templates/ contents into tmpdir/overrides/
        # (layered over the workspace's optional .mkdocs_theme/ — plugin wins)
        _scaffold_overrides(tmpdir, ws_root)

        # Convert QMDC → Markdown into tmpdir/docs/
        page_count = convert_workspace(
            ws_root,
            tmpdir,
            ws_data,
            hints,
            namespace_prefix=ns_prefix,
            hidden_kinds=_read_hidden_kinds(ws),
        )

        # Generate build config (merges user mkdocs.yml with tmpdir paths)
        # Use a staging dir inside tmpdir for mkdocs output, then move to final output
        staging_dir = tmpdir / "site_staging"
        build_config = generate_build_config(ws, tmpdir, staging_dir)

        # Run mkdocs build. Do NOT capture output: MkDocs logs its diagnostics
        # (INFO/WARNING about nav, broken links, plugin options, etc.) to stderr,
        # and we want them surfaced — same as `serve` streams them. Capturing
        # would silently swallow warnings on a successful build.
        result = subprocess.run(
            [sys.executable, "-m", "mkdocs", "build", "-f", str(build_config)],
        )
        if result.returncode != 0:
            click.echo("mkdocs build failed (see output above)", err=True)
            sys.exit(result.returncode)

        # Atomically replace old output: move old to temp, move new in, delete old
        if output.exists():
            old_output = output.with_name(output.name + ".old")
            if old_output.exists():
                shutil.rmtree(old_output)
            output.rename(old_output)
            try:
                shutil.move(str(staging_dir), str(output))
            except Exception:
                # Restore old output on failure
                old_output.rename(output)
                raise
            shutil.rmtree(old_output, ignore_errors=True)
        else:
            shutil.move(str(staging_dir), str(output))

        # Print build summary
        click.echo(f"Built {page_count} pages → {output}")
    finally:
        ws_data.close()
        shutil.rmtree(tmpdir, ignore_errors=True)


@cli.command()
@click.option("--port", default=8000, type=int)
@click.option("--host", default="127.0.0.1", help="Host to bind to (default: localhost)")
@click.pass_context
def serve(ctx: click.Context, port: int, host: str) -> None:
    """Start local preview server."""
    ws: Path = ctx.obj["workspace"]
    output: Path = ctx.obj["output"]

    # Resolve workspace root and namespace prefix
    ws_root, ns_prefix = _resolve_workspace_and_prefix(ws)

    # Load workspace data (always from root for full graph)
    ws_data = _load_or_exit(ws_root)
    tmpdir = Path(tempfile.mkdtemp(prefix="qmdc-mkdocs-"))
    try:
        # Load semantic hints
        hints = load_hints(ws_root)

        # Generate navigation and init config
        nav = generate_nav(ws_data, namespace_prefix=ns_prefix)

        # Determine site_name from parsed workspace data
        site_name = _get_site_name(ws_data, ns_prefix)

        generate_mkdocs_config(ws, nav, site_name)
        generate_nav_file(ws, nav)

        # Validate workspace (non-blocking)
        validate_workspace(ws_root)

        # Scaffold overrides into tmpdir (layered over the workspace's optional
        # .mkdocs_theme/ — plugin wins)
        _scaffold_overrides(tmpdir, ws_root)

        # Convert QMDC → Markdown into tmpdir/docs/
        convert_workspace(
            ws_root,
            tmpdir,
            ws_data,
            hints,
            namespace_prefix=ns_prefix,
            hidden_kinds=_read_hidden_kinds(ws),
        )

        # Generate build config pointing site_dir to output
        build_config = generate_build_config(ws, tmpdir, output)

        # Run mkdocs serve (blocks until Ctrl+C)
        click.echo(f"Starting dev server on port {port}...")
        with contextlib.suppress(KeyboardInterrupt):
            subprocess.run(
                [
                    sys.executable, "-m", "mkdocs", "serve",
                    "-f", str(build_config),
                    "--dev-addr", f"{host}:{port}",
                ],
            )
    finally:
        ws_data.close()
        shutil.rmtree(tmpdir, ignore_errors=True)


def _scaffold_overrides(tmpdir: Path, workspace: Path | None = None) -> None:
    """Copy Material theme overrides into ``tmpdir/overrides/`` (the build ``custom_dir``).

    Layering uses **safe precedence — the plugin always wins**:

    1. The workspace's optional ``.mkdocs_theme/`` directory is copied first. This
       lets users ADD their own branding assets — CSS, JS, icons, or extra
       partials — which land in the theme ``custom_dir`` so MkDocs ships them and
       they can be referenced from ``mkdocs.yml`` (e.g. ``extra_css``,
       ``extra_javascript``, ``theme.logo``).
    2. The plugin's own ``templates/`` (main.html, partials/, css/, js/) is copied
       second with ``dirs_exist_ok=True``, so it OVERWRITES any user file of the
       same name. This guarantees the QMDC features baked into those files (graph
       sidebar, mermaid renderer, hint popovers) can never be disabled by a
       user override — users extend the theme, they do not replace it.
    """
    templates_dir = Path(__file__).parent / "templates"
    overrides_dir = tmpdir / "overrides"
    overrides_dir.mkdir(parents=True, exist_ok=True)

    # 1. User theme overlay (additive; the plugin files below take precedence).
    if workspace is not None:
        user_theme = workspace / ".mkdocs_theme"
        if user_theme.is_dir():
            shutil.copytree(user_theme, overrides_dir, dirs_exist_ok=True)

    # 2. Plugin templates (authoritative — overwrite any conflicting user file).
    if templates_dir.is_dir():
        shutil.copytree(templates_dir, overrides_dir, dirs_exist_ok=True)


@cli.command()
@click.argument("file", required=False)
@click.option("--dry-run", is_flag=True, help="Show what would be regenerated without doing it")
@click.option("--force", is_flag=True, help="Regenerate even if sources_hash matches")
@click.pass_context
def regenerate(ctx: click.Context, file: str | None, dry_run: bool, force: bool) -> None:
    """Regenerate content for pages with ContentGenerator objects.

    If FILE is given, regenerate only that file (workspace-relative path).
    Otherwise, find and regenerate all stale ContentGenerator targets.
    """
    from .regenerate import compute_sources_hash, find_generators, regenerate_file

    ws: Path = ctx.obj["workspace"]
    ws_root, ns_prefix = _resolve_workspace_and_prefix(ws)

    ws_data = _load_or_exit(ws_root)
    try:
        generators = find_generators(ws_data)

        if not generators:
            click.echo("No ContentGenerator objects found in workspace.")
            return

        # Filter to specific file if given
        if file:
            generators = [g for g in generators if g["file"] == file]
            if not generators:
                click.echo(f"No ContentGenerator found in {file}")
                return

        # Process each generator
        total = len(generators)
        regenerated = 0
        skipped = 0

        for gen in generators:
            # Check hash
            current_hash = compute_sources_hash(gen, ws_data, ws_root)
            stored_hash = gen["sources_hash"]

            if not force and stored_hash == current_hash and stored_hash != "pending":
                click.echo(f"  {gen['file']}: unchanged, skip")
                skipped += 1
                continue

            if dry_run:
                click.echo(
                    f"  {gen['file']}: would regenerate "
                    f"(hash {stored_hash} → {current_hash})"
                )
                continue

            click.echo(f"  {gen['file']}: regenerating...")
            result = regenerate_file(gen["file"], ws_root, ws_data, force=force)

            if result["regenerated"]:
                click.echo(f"    ✓ done (hash: {result.get('hash', '?')})")
                regenerated += 1
            else:
                status = result.get("status", "unknown")
                error = result.get("error", "")
                click.echo(f"    ✗ {status}: {error}", err=True)

        click.echo(f"\nSummary: {regenerated}/{total} regenerated, {skipped} unchanged")
    finally:
        ws_data.close()


if __name__ == "__main__":
    cli()
