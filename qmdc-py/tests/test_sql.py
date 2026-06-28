"""
Data-driven SQL tests for QMD workspace.

Automatically discovers all directories with tests/ subdirectory containing .sql files.
"""

import json
from pathlib import Path

import pytest

from qmdc.db import QmdcDatabase
from qmdc.workspace import parse_all_workspaces


def find_project_root() -> Path:
    """Find project root by looking for the qmdc-rs/ package directory."""
    current = Path(__file__).resolve()
    while current != current.parent:
        if (current / "qmdc-rs").exists():
            return current
        current = current.parent
    raise RuntimeError("Could not find project root (qmdc-rs/ directory)")


_root = find_project_root()

# Path to scan for test workspaces
SCAN_PATHS = [
    _root / "tests/workspace",
]


def load_workspace_objects(workspace_path: Path) -> list:
    """Load all objects from a workspace (handles multi-workspace folders)."""
    result = parse_all_workspaces(str(workspace_path))
    return result.objects


def get_sql_tests(workspace_path: Path) -> list[tuple[str, Path, Path]]:
    """Find all SQL tests in workspace/tests directory."""
    tests_dir = workspace_path / "tests"
    tests = []

    if not tests_dir.exists():
        return tests

    sql_files = sorted(tests_dir.glob("*.sql"))

    for sql_file in sql_files:
        name = sql_file.stem
        expected_file = tests_dir / f"{name}.expected.json"

        if expected_file.exists():
            tests.append((name, sql_file, expected_file))

    return tests


def find_test_workspaces(directory: Path, prefix: str = "") -> list[tuple[str, Path, Path, Path]]:
    """Recursively find all directories containing tests/ with .sql files."""
    test_cases = []

    if not directory.exists():
        return test_cases

    for entry in sorted(directory.iterdir()):
        if not entry.is_dir():
            continue

        tests_dir = entry / "tests"
        workspace_name = f"{prefix}/{entry.name}" if prefix else entry.name

        # Check if this directory has tests/
        if tests_dir.exists() and tests_dir.is_dir():
            sql_files = list(tests_dir.glob("*.sql"))
            if sql_files:
                for name, sql_file, expected_file in get_sql_tests(entry):
                    test_cases.append((f"{workspace_name}/{name}", entry, sql_file, expected_file))

        # Also check subdirectories (but not tests/ itself)
        if entry.name != "tests":
            test_cases.extend(find_test_workspaces(entry, workspace_name))

    return test_cases


def collect_test_cases():
    """Collect all SQL test cases from all scan paths."""
    test_cases = []

    for scan_path in SCAN_PATHS:
        test_cases.extend(find_test_workspaces(scan_path))

    return sorted(test_cases, key=lambda x: x[0])


TEST_CASES = collect_test_cases()


@pytest.mark.parametrize(
    "test_id,workspace_path,sql_file,expected_file", TEST_CASES, ids=[t[0] for t in TEST_CASES]
)
def test_sql_query(test_id: str, workspace_path: Path, sql_file: Path, expected_file: Path):
    """Run SQL test and compare with expected results."""
    objects = load_workspace_objects(workspace_path)

    db = QmdcDatabase()
    db.sync_objects(objects)

    sql = sql_file.read_text().strip()

    expected = json.loads(expected_file.read_text())
    expected_columns = expected["columns"]
    expected_rows = expected["rows"]

    result = db.query(sql)

    assert result.columns == expected_columns, (
        f"Columns mismatch: expected {expected_columns}, got {result.columns}"
    )

    assert result.rows == expected_rows, (
        f"Rows mismatch:\nexpected: {expected_rows}\ngot: {result.rows}"
    )


def test_parse_all_workspaces():
    """Test that parse_all_workspaces correctly finds all workspaces."""
    test_dir = SCAN_PATHS[0]

    print("\n=== Testing parse_all_workspaces ===")
    print(f"Test directory: {test_dir}")

    result = parse_all_workspaces(str(test_dir))

    workspace_count = sum(1 for obj in result.objects if obj.get("__kind") == "__Workspace")

    print(f"Found {workspace_count} workspace objects")

    assert workspace_count >= 3, f"Expected at least 3 workspaces, found {workspace_count}"

    workspace_ids = [
        obj.get("__id")
        for obj in result.objects
        if obj.get("__kind") == "__Workspace" and obj.get("__id")
    ]

    print(f"Workspace IDs: {workspace_ids}")

    for expected in ["ecommerce", "backend", "frontend"]:
        assert expected in workspace_ids, f"Should find '{expected}' workspace"

    print("✓ parse_all_workspaces test passed")
