"""
Data-driven tests for QMD workspace functionality.

Tests are auto-discovered from workspace directories containing _expected.json.
Add new workspace test by creating a directory with:
  - readme.qmd.md (with __Workspace object)
  - _expected.json (test expectations)

Format of _expected.json:
{
  "workspace_id": "my_workspace",
  "files": ["readme.qmd.md", "other.qmd.md"],
  "objects": {
    "Kind": ["id1", "id2"],
    "__Workspace": ["my_workspace"]
  },
  "errors": [
    {"type": "broken_link", "object": "obj_id", "reference": "[[#ref]]"}
  ]
}
"""

import json
from pathlib import Path

import pytest

from qmdc.workspace import parse_workspace, scan_workspace

# Root directory for workspace microtests
WORKSPACE_TESTS_ROOT = Path(__file__).parent.parent.parent / "tests/workspace"


def find_workspace_tests() -> list[tuple[str, Path, dict]]:
    """Find all workspace test directories."""
    tests = []

    def scan_dir(dir_path: Path, prefix: str = ""):
        for item in sorted(dir_path.iterdir()):
            if not item.is_dir():
                continue

            expected_file = item / "_expected.json"
            readme_file = item / "readme.qmd.md"

            if expected_file.exists() and readme_file.exists():
                test_name = f"{prefix}{item.name}" if prefix else item.name
                with open(expected_file) as f:
                    expected = json.load(f)
                tests.append((test_name, item, expected))
            else:
                new_prefix = f"{prefix}{item.name}/" if prefix else f"{item.name}/"
                scan_dir(item, new_prefix)

    if WORKSPACE_TESTS_ROOT.exists():
        scan_dir(WORKSPACE_TESTS_ROOT)

    return tests


WORKSPACE_TESTS = find_workspace_tests()


@pytest.mark.parametrize(
    "test_name,workspace_path,expected",
    WORKSPACE_TESTS,
    ids=[t[0] for t in WORKSPACE_TESTS],
)
class TestWorkspace:
    """Data-driven workspace tests."""

    def test_workspace_id(self, test_name: str, workspace_path: Path, expected: dict):
        """Workspace ID should match exactly."""
        result = parse_workspace(str(workspace_path))
        assert result.workspace_id == expected["workspace_id"]

    def test_files(self, test_name: str, workspace_path: Path, expected: dict):
        """Files list should match exactly."""
        files = scan_workspace(str(workspace_path))
        assert sorted(files) == sorted(expected["files"])

    def test_objects_by_kind(self, test_name: str, workspace_path: Path, expected: dict):
        """Objects grouped by kind should match exactly."""
        result = parse_workspace(str(workspace_path))

        # Build actual objects dict
        actual: dict[str, list[str]] = {}
        for obj in result.objects:
            kind = obj.get("__kind", "")
            obj_id = obj.get("__id", "")
            if kind not in actual:
                actual[kind] = []
            actual[kind].append(obj_id)

        # Sort for comparison
        for k in actual:
            actual[k] = sorted(actual[k])

        expected_objects = {k: sorted(v) for k, v in expected["objects"].items()}

        assert actual == expected_objects

    def test_errors(self, test_name: str, workspace_path: Path, expected: dict):
        """Errors should match exactly (type, object, reference)."""
        if expected.get("errors") is None:
            pytest.skip("No errors defined in expected.json")
        result = parse_workspace(str(workspace_path))

        # Build actual errors list
        actual_errors = []
        for e in result.errors:
            err = {"type": e.type}
            if e.object_id:
                err["object"] = e.object_id
            if e.reference:
                err["reference"] = e.reference
            if e.file:
                err["file"] = e.file
            if e.line:
                err["line"] = e.line
            if e.candidates:
                err["candidates"] = e.candidates
            actual_errors.append(err)

        # Sort for comparison
        def sort_key(x):
            return (
                x["type"],
                x.get("object", ""),
                x.get("reference", ""),
                x.get("file", ""),
                x.get("line", 0),
                ",".join(x.get("candidates", [])),
            )

        actual_sorted = sorted(actual_errors, key=sort_key)
        expected_sorted = sorted(expected["errors"], key=sort_key)

        assert actual_sorted == expected_sorted

    def test_objects_have_metadata(self, test_name: str, workspace_path: Path, expected: dict):
        """All objects should have __file and __line metadata."""
        result = parse_workspace(str(workspace_path))

        for obj in result.objects:
            kind = obj.get("__kind", "")
            if kind.startswith("__") and kind not in ("__Workspace", "__Namespace"):
                continue

            assert "__file" in obj, f"Object {obj.get('__id')} missing __file"
            assert "__line" in obj, f"Object {obj.get('__id')} missing __line"


def test_workspace_validate_command():
    """Test workspace validate CLI command returns JSON array of errors."""
    import json
    import subprocess

    tests = find_workspace_tests()
    assert len(tests) > 0, "Should find at least one workspace test"

    for test_name, workspace_path, _expected in tests:
        # Get errors from workspace parse (may exit with code 1 if errors exist)
        parse_result = subprocess.run(
            ["qmdc", "workspace", "parse", str(workspace_path)],
            capture_output=True,
            text=True,
            cwd=Path(__file__).parent.parent,
        )

        # Skip tests where workspace is not found (some test workspaces may not have __Workspace)
        if "No workspace found" in parse_result.stderr or not parse_result.stdout:
            continue

        parse_output = json.loads(parse_result.stdout)
        parse_errors = parse_output.get("errors", [])

        # Get errors from workspace validate
        validate_result = subprocess.run(
            ["qmdc", "workspace", "validate", str(workspace_path)],
            capture_output=True,
            text=True,
            cwd=Path(__file__).parent.parent,
        )

        # Skip if workspace not found
        if "No workspace found" in validate_result.stderr or not validate_result.stdout:
            continue

        # Validate should return JSON array directly (not wrapped in object)
        validate_errors = json.loads(validate_result.stdout)
        assert isinstance(validate_errors, list), f"Test {test_name}: validate should return array"

        # Check that validate returns same number of errors as parse
        assert len(validate_errors) == len(parse_errors), (
            f"Test {test_name}: validate returned {len(validate_errors)} errors, "
            f"but parse returned {len(parse_errors)} errors"
        )

        # Check that validate returns correct format
        for error in validate_errors:
            assert "type" in error, f"Test {test_name}: Error should have 'type' field: {error}"
            assert "message" in error, (
                f"Test {test_name}: Error should have 'message' field: {error}"
            )
            assert "severity" in error, (
                f"Test {test_name}: Error should have 'severity' field: {error}"
            )
            # Check optional fields exist (can be null)
            assert "file" in error or error.get("file") is None, (
                f"Test {test_name}: Error should have 'file' field: {error}"
            )
            assert "line" in error or error.get("line") is None, (
                f"Test {test_name}: Error should have 'line' field: {error}"
            )
            assert "objectId" in error or error.get("objectId") is None, (
                f"Test {test_name}: Error should have 'objectId' field: {error}"
            )
            assert "fieldName" in error or error.get("fieldName") is None, (
                f"Test {test_name}: Error should have 'fieldName' field: {error}"
            )
            assert "reference" in error or error.get("reference") is None, (
                f"Test {test_name}: Error should have 'reference' field: {error}"
            )
            assert "candidates" in error or error.get("candidates") is None, (
                f"Test {test_name}: Error should have 'candidates' field: {error}"
            )

        # Check exit code: 0 if no errors, 1 if errors
        expected_exit_code = 0 if len(validate_errors) == 0 else 1
        assert validate_result.returncode == expected_exit_code, (
            f"Test {test_name}: validate should exit with code {expected_exit_code}, "
            f"but exited with {validate_result.returncode}"
        )


def test_parser_consistency_validation_parser_consistency():
    """
    Test that all three parsers (Rust, Python, TypeScript) produce identical validation errors
    for the validation-parser-consistency workspace.

    This test specifically checks for QMD-38 bugs:
    1. TypeScript line numbers for duplicate_id
    2. Rust parsing_error_* objects causing false duplicate_id
    3. Python/TypeScript broken_link in inline code
    """
    import subprocess

    root_dir = Path(__file__).parent.parent.parent
    workspace_path = root_dir / "tests/workspace/validation-parser-consistency"

    if not workspace_path.exists():
        pytest.skip("validation-parser-consistency workspace not found")

    # Run all three parsers
    parsers = {
        "rust": root_dir / "bin/qmdc-rs",
        "python": root_dir / "bin/qmdc-py",
        "typescript": root_dir / "bin/qmdc-ts",
    }

    results = {}
    for name, parser_path in parsers.items():
        if not parser_path.exists():
            pytest.skip(f"{name} parser not found at {parser_path}")

        result = subprocess.run(
            [str(parser_path), "workspace", "validate", str(workspace_path)],
            capture_output=True,
            text=True,
            cwd=root_dir,
        )

        if result.returncode not in (0, 1):
            pytest.fail(f"{name} parser failed: {result.stderr}")

        try:
            errors = json.loads(result.stdout)
            if not isinstance(errors, list):
                pytest.fail(f"{name} parser returned non-list: {type(errors)}")
        except json.JSONDecodeError as e:
            pytest.fail(f"{name} parser returned invalid JSON: {e}\n{result.stdout}")

        # Normalize errors for comparison
        normalized = []
        for err in errors:
            normalized.append(
                {
                    "type": err.get("type", ""),
                    "file": err.get("file", ""),
                    "line": err.get("line", 0),
                    "objectId": err.get("objectId", ""),
                }
            )

        # Sort for consistent comparison
        normalized.sort(key=lambda x: (x["file"], x["line"], x["type"], x["objectId"]))
        results[name] = normalized

    # Compare all parsers
    python_errors = results["python"]
    rust_errors = results["rust"]
    ts_errors = results["typescript"]

    # Python vs TypeScript should be identical
    if python_errors != ts_errors:
        diff_py_ts = []
        for i, (py_err, ts_err) in enumerate(zip(python_errors, ts_errors, strict=False)):
            if py_err != ts_err:
                diff_py_ts.append(f"  Index {i}: Python={py_err}, TypeScript={ts_err}")
        pytest.fail(
            "Python and TypeScript parsers produce different errors:\n" + "\n".join(diff_py_ts[:10])
        )

    # Rust vs Python should be identical
    if rust_errors != python_errors:
        # Find differences
        rust_only = [e for e in rust_errors if e not in python_errors]
        python_only = [e for e in python_errors if e not in rust_errors]

        msg = "Rust and Python parsers produce different errors:\n"
        if rust_only:
            msg += f"  Errors ONLY in Rust ({len(rust_only)}):\n"
            for err in rust_only[:10]:
                msg += f"    {err}\n"
        if python_only:
            msg += f"  Errors ONLY in Python ({len(python_only)}):\n"
            for err in python_only[:10]:
                msg += f"    {err}\n"
        pytest.fail(msg)


@pytest.mark.parametrize("parser", ["rust", "python", "typescript"])
def test_qmd59_container_root_no_false_nested_workspace(parser: str):
    """QMD-59: `workspace validate` on a NON-workspace container dir holding a single
    sub-workspace must NOT report `nested_workspace`.

    Repro of the release bug: running `qmdc workspace validate <dir>` where `<dir>`
    has no `readme.qmd.md` with `[[__Workspace]]` but contains exactly one real
    sub-workspace.

    Decided contract (QMD-59 open question #2 — answered by the operator):
    validate each contained sub-workspace INDEPENDENTLY. The container itself is
    not a workspace, so:
      - it must NOT be flagged as a nested-workspace violation, and
      - the contained sub-workspace must be validated; since `docs_ws` here is
        itself valid, `workspace validate` must return an empty error array `[]`
        and exit 0.

    Today every parser violates this, each differently (cross-parser divergence,
    not just a single shared bug):
      - Rust: false `nested_workspace` error (parse_workspace treats the
        container as a virtual workspace and flags `docs_ws` as nested).
      - TypeScript: same false `nested_workspace` error.
      - Python: bails with "No workspace found" on stderr, empty stdout, exit 1
        (does not descend into the sub-workspace at all).

    So this is NOT simply "route validate through parse_all_workspaces" — only
    Rust's parse_all_workspaces resolves a contained sub-workspace; Python/TS do
    not. The fix must make all three converge on contract (a).

    EXPECTED TO FAIL until QMD-59 is fixed; must then pass identically across all
    three parsers (cross-parser parity of `workspace validate` output).
    """
    import subprocess

    root_dir = Path(__file__).parent.parent.parent
    workspace_path = root_dir / "tests/workspace/container-root-single-workspace"

    if not workspace_path.exists():
        pytest.fail(f"container-root-single-workspace fixture missing at {workspace_path}")

    parser_path = {
        "rust": root_dir / "bin/qmdc-rs",
        "python": root_dir / "bin/qmdc-py",
        "typescript": root_dir / "bin/qmdc-ts",
    }[parser]

    if not parser_path.exists():
        pytest.fail(
            f"{parser} parser wrapper not found at {parser_path} — cross-parser "
            f"validation cannot run; build/wire up the parser instead of skipping"
        )

    result = subprocess.run(
        [str(parser_path), "workspace", "validate", str(workspace_path)],
        capture_output=True,
        text=True,
        cwd=root_dir,
    )

    # Contract (a): the container is not a workspace, its single sub-workspace
    # (`docs_ws`) is valid → validate must emit a JSON error array, and it must be
    # empty. This rejects all three current behaviours: the false nested_workspace
    # error (Rust/TS) and the "No workspace found" bail-out (Python).
    assert result.stdout.strip(), (
        f"{parser}: expected a JSON error array on stdout, got empty "
        f"(stderr: {result.stderr.strip()!r}) — the container's sub-workspace "
        f"must be validated independently, not skipped with 'No workspace found'"
    )
    errors = json.loads(result.stdout)
    assert isinstance(errors, list), f"{parser}: validate should return a JSON array"
    assert errors == [], (
        f"{parser}: validating a non-workspace container that holds a single "
        f"VALID sub-workspace must return no errors, got: {errors}"
    )
    assert result.returncode == 0, (
        f"{parser}: validate should exit 0 when there are no errors, exited {result.returncode}"
    )


@pytest.mark.parametrize("parser", ["rust", "python", "typescript"])
def test_qmd59_walkup_resolves_parent_workspace(parser: str):
    """QMD-59 walk-up parity: pointing `query` at a SUBDIRECTORY of a real
    workspace must walk UP and resolve the parent workspace, in all three parsers.

    Decided contract (operator): the user must be able to run query/validate from
    ANY directory without it breaking — both from a subdir of a workspace
    (walk-up) and from a container above workspaces (walk-down). This test pins
    the walk-up half.

    Fixture `walkup-subdir-of-workspace/` is a real workspace (`walkup_ws`) with a
    nested `sub/` directory that has NO `readme.qmd.md` of its own. Querying
    `.../walkup-subdir-of-workspace/sub` must resolve `walkup_ws`.

    Before the fix this diverged: Python walked up to the parent workspace, while
    Rust/TS synthesized a virtual workspace named after the subdir (`sub`). After
    the fix all three must resolve `walkup_ws`.
    """
    import subprocess

    root_dir = Path(__file__).parent.parent.parent
    sub_path = root_dir / "tests/workspace/walkup-subdir-of-workspace/sub"

    if not sub_path.exists():
        pytest.fail(f"walkup-subdir-of-workspace/sub fixture missing at {sub_path}")

    parser_path = {
        "rust": root_dir / "bin/qmdc-rs",
        "python": root_dir / "bin/qmdc-py",
        "typescript": root_dir / "bin/qmdc-ts",
    }[parser]

    if not parser_path.exists():
        pytest.fail(
            f"{parser} parser wrapper not found at {parser_path} — cross-parser "
            f"walk-up test cannot run; build/wire up the parser instead of skipping"
        )

    result = subprocess.run(
        [
            str(parser_path),
            "query",
            str(sub_path),
            "SELECT __id FROM objects WHERE __kind='__Workspace'",
            "--format",
            "json",
        ],
        capture_output=True,
        text=True,
        cwd=root_dir,
    )

    assert result.returncode == 0, (
        f"{parser}: query on a subdir of a workspace should exit 0, "
        f"exited {result.returncode} (stderr: {result.stderr.strip()!r})"
    )
    assert result.stdout.strip(), (
        f"{parser}: expected JSON query output on stdout, got empty "
        f"(stderr: {result.stderr.strip()!r})"
    )
    payload = json.loads(result.stdout)
    rows = payload.get("rows", [])
    # rows is a list of rows; each row is a list of column values (single column here)
    resolved_ids = {str(cell) for row in rows for cell in row}
    assert resolved_ids == {"walkup_ws"}, (
        f"{parser}: query on a subdir of a workspace must walk UP and resolve the "
        f"parent workspace 'walkup_ws', got: {sorted(resolved_ids)}"
    )


@pytest.mark.parametrize("parser", ["rust", "python", "typescript"])
def test_qmd59_walkup_relative_dot_resolves_parent_workspace(parser: str):
    """QMD-59 walk-up parity for a RELATIVE path (`.`).

    Regression for the cross-parser divergence found in code review: with cwd set
    to a workspace subdir and the path argument `.`, walk-up must still resolve the
    parent workspace. Before the fix, Rust resolved a virtual `workspace` and TS a
    virtual `tracking` (named after the subdir) because they did not canonicalize
    the relative path before walking up; only Python (which used Path.resolve())
    walked up. After canonicalizing in all three, `.` from a subdir resolves the
    parent workspace everywhere.
    """
    import subprocess

    root_dir = Path(__file__).parent.parent.parent
    sub_path = root_dir / "tests/workspace/walkup-subdir-of-workspace/sub"

    if not sub_path.exists():
        pytest.fail(f"walkup-subdir-of-workspace/sub fixture missing at {sub_path}")

    parser_path = {
        "rust": root_dir / "bin/qmdc-rs",
        "python": root_dir / "bin/qmdc-py",
        "typescript": root_dir / "bin/qmdc-ts",
    }[parser]

    if not parser_path.exists():
        pytest.fail(
            f"{parser} parser wrapper not found at {parser_path} — cross-parser "
            f"walk-up test cannot run; build/wire up the parser instead of skipping"
        )

    # Run with cwd = the subdir and pass the relative path "." as the argument.
    result = subprocess.run(
        [
            str(parser_path),
            "query",
            ".",
            "SELECT __id FROM objects WHERE __kind='__Workspace'",
            "--format",
            "json",
        ],
        capture_output=True,
        text=True,
        cwd=sub_path,
    )

    assert result.returncode == 0, (
        f"{parser}: query '.' from a workspace subdir should exit 0, "
        f"exited {result.returncode} (stderr: {result.stderr.strip()!r})"
    )
    assert result.stdout.strip(), (
        f"{parser}: expected JSON query output on stdout, got empty "
        f"(stderr: {result.stderr.strip()!r})"
    )
    payload = json.loads(result.stdout)
    rows = payload.get("rows", [])
    resolved_ids = {str(cell) for row in rows for cell in row}
    assert resolved_ids == {"walkup_ws"}, (
        f"{parser}: query '.' from a workspace subdir must walk UP and resolve "
        f"'walkup_ws', got: {sorted(resolved_ids)}"
    )
