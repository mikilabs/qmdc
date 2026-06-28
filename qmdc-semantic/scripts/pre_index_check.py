#!/usr/bin/env python3
"""Pre-index validation for qmdc-semantic.

Checks a QMDC workspace for issues that would break or degrade semantic indexing:
1. Runs qmdc workspace validate (broken links, duplicates, etc.)
2. Parses all objects and flags oversized fields (likely YAML | bleed-through)
3. Reports objects with suspiciously large text content

Usage:
    uv run python scripts/pre_index_check.py <workspace_path>
    uv run python scripts/pre_index_check.py ../docs
"""

import json
import sys
from pathlib import Path

import click

# Add parent to path so we can import qmdc
sys.path.insert(0, str(Path(__file__).resolve().parent.parent))


MAX_FIELD_CHARS = 4000  # nomic-embed-text context is ~8192 tokens ≈ 6000 chars
MAX_OBJECT_CHARS = 8000


@click.command()
@click.argument("workspace_path", type=click.Path(exists=True))
@click.option("--max-field", default=MAX_FIELD_CHARS, help="Max chars per field before warning")
@click.option("--max-object", default=MAX_OBJECT_CHARS, help="Max total chars per object before warning")
@click.option("--fix-hint/--no-fix-hint", default=True, help="Show fix hints")
def main(workspace_path: str, max_field: int, max_object: int, fix_hint: bool):
    """Check workspace for indexing issues."""
    ws = Path(workspace_path).resolve()

    # --- Step 1: workspace validate ---
    click.echo(f"Checking workspace: {ws}\n")
    click.echo("=" * 60)
    click.echo("Step 1: qmdc workspace validate")
    click.echo("=" * 60)

    from qmdc import parse_workspace, validate_workspace

    result = parse_workspace(str(ws))
    errors = validate_workspace(result.objects, result.index, str(ws))
    if errors:
        click.echo(f"  ❌ {len(errors)} validation error(s):")
        for err in errors[:20]:
            if isinstance(err, dict):
                click.echo(f"    {err.get('type', '?')}: {err.get('message', '?')}")
                click.echo(f"      file: {err.get('file', '?')} line: {err.get('line', '?')}")
            else:
                click.echo(f"    {err}")
        if len(errors) > 20:
            click.echo(f"    ... and {len(errors) - 20} more")
    else:
        click.echo("  ✅ No validation errors")

    # --- Step 2: oversized fields ---
    click.echo()
    click.echo("=" * 60)
    click.echo("Step 2: Oversized fields (likely YAML | bleed-through)")
    click.echo("=" * 60)

    objects = result.objects

    issues = []
    for obj in objects:
        obj_id = obj.get("__id", "?")
        obj_kind = obj.get("__kind", "?")
        obj_file = obj.get("__file", "?")
        obj_line = obj.get("__line", "?")

        total_chars = 0
        big_fields = []

        for key, value in obj.items():
            if key.startswith("__"):
                continue
            if isinstance(value, str):
                total_chars += len(value)
                if len(value) > max_field:
                    big_fields.append((key, len(value)))

        if big_fields or total_chars > max_object:
            issues.append({
                "id": obj_id,
                "kind": obj_kind,
                "file": obj_file,
                "line": obj_line,
                "total_chars": total_chars,
                "big_fields": big_fields,
            })

    if issues:
        click.echo(f"  ⚠️  {len(issues)} object(s) with oversized fields:\n")
        for iss in issues:
            click.echo(f"  {iss['kind']}:{iss['id']}  ({iss['file']}:{iss['line']})")
            click.echo(f"    total text: {iss['total_chars']} chars")
            for fname, flen in iss["big_fields"]:
                click.echo(f"    field \"{fname}\": {flen} chars (limit {max_field})")
            if fix_hint:
                click.echo(f"    💡 Check for YAML | fields that bleed into next heading")
            click.echo()
    else:
        click.echo(f"  ✅ All fields under {max_field} chars, all objects under {max_object} chars")

    # --- Summary ---
    click.echo("=" * 60)
    total_objects = len(objects)
    total_issues = len(errors) + len(issues)
    if total_issues == 0:
        click.echo(f"✅ Workspace OK: {total_objects} objects, ready for indexing")
    else:
        click.echo(f"⚠️  {total_issues} issue(s) found across {total_objects} objects")
        click.echo("Fix these before running: qmdc-semantic index")

    sys.exit(1 if total_issues > 0 else 0)


if __name__ == "__main__":
    main()
