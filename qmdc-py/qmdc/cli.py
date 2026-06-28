"""CLI commands using Click."""

import json
import sys

import click

from .db import execute_query
from .parser import parse as qmdc_parse
from .parser import rebuild as qmdc_rebuild
from .workspace import (
    resolve_workspace,
    workspace_to_json,
)


@click.group()
@click.version_option(version="0.1.0")
def cli():
    """QMDC Parser - Convert QMD.md to JSON and back."""
    pass


@cli.command()
@click.option(
    "-i",
    "--input",
    "input_file",
    type=click.File("r"),
    default=sys.stdin,
    help="Input QMD.md file (default: stdin)",
)
@click.option(
    "-o",
    "--output",
    "output_file",
    type=click.File("w"),
    default=sys.stdout,
    help="Output JSON file (default: stdout)",
)
@click.option("-v", "--verbose", count=True, help="Increase verbosity")
@click.option("--strict", is_flag=True, help="Fail-fast mode")
@click.option("--no-comments", is_flag=True, help="Exclude __comments from output")
@click.option("--no-syntax", is_flag=True, help="Exclude __syntax from output")
@click.option("--pretty/--no-pretty", default=True, help="Format JSON with indents")
@click.option(
    "--format",
    "output_format",
    type=click.Choice(["minimal", "standard", "full"]),
    default="standard",
    help="Output format (minimal, standard, full)",
)
def parse(input_file, output_file, verbose, strict, no_comments, no_syntax, pretty, output_format):
    """Parse QMD.md to JSON."""
    try:
        markdown = input_file.read()
        result = qmdc_parse(markdown, format=output_format)

        # Remove metadata if requested
        if no_comments:
            for obj in result:
                obj.pop("__comments", None)

        if no_syntax:
            for obj in result:
                obj.pop("__syntax", None)

        # Output
        indent = 2 if pretty else None
        json.dump(result, output_file, indent=indent, ensure_ascii=False)

        if output_file == sys.stdout:
            output_file.write("\n")

    except Exception as e:
        click.echo(f"Error: {e}", err=True)
        sys.exit(1)


@cli.command()
@click.option(
    "-i",
    "--input",
    "input_file",
    type=click.File("r"),
    default=sys.stdin,
    help="Input JSON file (default: stdin)",
)
@click.option(
    "-o",
    "--output",
    "output_file",
    type=click.File("w"),
    default=sys.stdout,
    help="Output QMD.md file (default: stdout)",
)
@click.option("-v", "--verbose", count=True, help="Increase verbosity")
def rebuild(input_file, output_file, verbose):
    """Rebuild QMD.md from JSON."""
    try:
        json_text = input_file.read()
        data = json.loads(json_text)

        result = qmdc_rebuild(data)

        output_file.write(result)

    except json.JSONDecodeError as e:
        click.echo(f"Invalid JSON: {e}", err=True)
        sys.exit(1)
    except Exception as e:
        click.echo(f"Error: {e}", err=True)
        sys.exit(1)


# === Workspace Commands ===


@cli.group()
def workspace():
    """Workspace commands for multi-file QMDC projects."""
    pass


@workspace.command("parse")
@click.argument("path", type=click.Path(exists=True), default=".")
@click.option(
    "-o",
    "--output",
    "output_file",
    type=click.File("w"),
    default=sys.stdout,
    help="Output JSON file (default: stdout)",
)
@click.option("-v", "--verbose", count=True, help="Increase verbosity")
@click.option("--pretty/--no-pretty", default=True, help="Format JSON with indents")
@click.option("--errors-only", is_flag=True, help="Output only errors (for CI)")
def workspace_parse(path, output_file, verbose, pretty, errors_only):
    """Parse workspace directory to JSON.

    Examples:
        qmdc workspace parse .
        qmdc workspace parse ./spec -o workspace.json
        qmdc workspace parse . --errors-only
    """
    try:
        # QMD-59: unified resolver — walk-up to an ancestor workspace, else
        # walk-down into contained sub-workspaces. No "No workspace found" bail.
        result = resolve_workspace(path)

        if verbose:
            click.echo(f"Root: {result.root}", err=True)
            click.echo(f"Files: {len(result.files)}", err=True)
            click.echo(f"Objects: {len(result.objects)}", err=True)
            click.echo(f"Errors: {len(result.errors)}", err=True)

        # Output
        if errors_only:
            output = {
                "errors": [
                    {
                        "type": e.type,
                        "message": e.message,
                        "file": e.file,
                        "line": e.line,
                        "object": e.object_id,
                        "field": e.field_name,
                        "reference": e.reference,
                        "severity": e.severity,
                    }
                    for e in result.errors
                ]
            }
        else:
            output = workspace_to_json(result)

        indent = 2 if pretty else None
        json.dump(output, output_file, indent=indent, ensure_ascii=False)

        if output_file == sys.stdout:
            output_file.write("\n")

        # Exit with error code if there are errors
        if result.errors:
            sys.exit(1)

    except Exception as e:
        click.echo(f"Error: {e}", err=True)
        sys.exit(1)


@workspace.command("validate")
@click.argument("path", type=click.Path(exists=True), default=".")
def workspace_validate(path):
    """Validate workspace for errors.

    Returns JSON array of errors (empty array if no errors).

    Checks:
    - Broken links
    - Duplicate Kind:Id in same namespace
    - Ambiguous references

    Examples:
        qmdc workspace validate .
        qmdc workspace validate ./spec
    """
    try:
        # QMD-59: unified resolver — walk-up then walk-down (see resolve_workspace).
        result = resolve_workspace(path)

        # Output only errors array as JSON
        errors_array = [
            {
                "type": e.type,
                "message": e.message,
                "file": e.file,
                "line": e.line,
                "objectId": e.object_id,
                "fieldName": e.field_name,
                "reference": e.reference,
                "candidates": e.candidates,
                "severity": e.severity,
            }
            for e in result.errors
        ]
        json.dump(errors_array, sys.stdout, indent=2, ensure_ascii=False)
        sys.stdout.write("\n")

        # Exit with error code if there are errors
        if result.errors:
            sys.exit(1)

    except Exception as e:
        click.echo(f"Error: {e}", err=True)
        sys.exit(1)


@workspace.command("files")
@click.argument("path", type=click.Path(exists=True), default=".")
def workspace_files(path):
    """List all QMD.md files in workspace.

    Examples:
        qmdc workspace files .
        qmdc workspace files ./spec
    """
    try:
        # QMD-59: unified resolver — walk-up then walk-down.
        result = resolve_workspace(path)
        for f in result.files:
            click.echo(f)

    except Exception as e:
        click.echo(f"Error: {e}", err=True)
        sys.exit(1)


# === Query Command ===


@cli.command()
@click.argument("workspace_path", type=click.Path(exists=True))
@click.argument("query")
@click.option(
    "-f",
    "--format",
    "output_format",
    type=click.Choice(["table", "json"]),
    default="table",
    help="Output format (table or json)",
)
def query(workspace_path, query, output_format):
    """Execute SQL query against workspace.

    QUERY can be:
    - SQL query: "SELECT * FROM objects"
    - Query object reference: "#get_tables"

    Examples:
        qmdc query . "SELECT __id, __kind FROM objects LIMIT 5"
        qmdc query ./spec "#get_all_tables"
        qmdc query . "SELECT * FROM objects" --format json
    """
    try:
        # QMD-59: unified resolver — walk-up to an ancestor workspace, else
        # walk-down into contained sub-workspaces (so query works from any dir).
        ws = resolve_workspace(workspace_path)
        ws_dict = {
            "objects": ws.objects,
        }

        # Execute query
        result = execute_query(ws_dict, query)

        # Output
        if output_format == "json":
            output = {"columns": result.columns, "rows": result.rows}
            json.dump(output, sys.stdout, indent=2, ensure_ascii=False)
            sys.stdout.write("\n")
        else:
            sys.stdout.write(result.to_table_string())

    except Exception as e:
        click.echo(f"Error: {e}", err=True)
        sys.exit(1)
