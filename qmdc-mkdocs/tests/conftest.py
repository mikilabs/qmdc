"""Shared test fixtures for qmdc-mkdocs tests.

Uses the qmd Python parser directly (no subprocess mocking).
"""

import tempfile
from pathlib import Path

import pytest

from qmdc.db import QmdcDatabase
from qmdc.workspace import parse_workspace


# --- Sample workspace content for testing ---

SAMPLE_WORKSPACE_FILES = {
    "readme.qmd.md": """\
# My Project [[myproject: __Workspace]]

- version: 1.0
- namespaces: [[#storage]], [[#api]]
""",
    "storage/readme.qmd.md": """\
# Storage Layer [[storage: __Namespace]]

- description: Database schema and storage
""",
    "storage/tables.qmd.md": """\
# Tables [[tables_doc: __Document]]

## Users [[users: Table]]

- name: users
- description: User accounts table

## Orders [[orders: Table]]

- name: orders
- user_ref: [[#users]]
""",
    "api/readme.qmd.md": """\
# API Layer [[api: __Namespace]]

- description: REST API endpoints
""",
    "api/endpoints.qmd.md": """\
# Endpoints [[endpoints_doc: __Document]]

## Get Users [[get_users: Endpoint]]

- method: GET
- path: /api/users
- returns: [[#storage:users]]

## Get Orders [[get_orders: Endpoint]]

- method: GET
- path: /api/orders
- returns: [[#storage:orders]]
""",
}


@pytest.fixture
def sample_workspace(tmp_path):
    """Create a real workspace on disk for testing with the qmd parser."""
    for rel_path, content in SAMPLE_WORKSPACE_FILES.items():
        file_path = tmp_path / rel_path
        file_path.parent.mkdir(parents=True, exist_ok=True)
        file_path.write_text(content, encoding="utf-8")
    return tmp_path


@pytest.fixture
def sample_ws_data(sample_workspace):
    """Parse the sample workspace and return WorkspaceData-like object.

    This mimics what database.load_workspace() produces, using the real parser.
    """
    ws_result = parse_workspace(str(sample_workspace))

    db = QmdcDatabase()
    db.sync_objects(ws_result.objects)

    # Group objects by file
    objects_by_file: dict[str, list] = {}
    for obj in ws_result.objects:
        f = obj.get("__file", "")
        if f:
            objects_by_file.setdefault(f, []).append(obj)

    class _WorkspaceData:
        def __init__(self):
            self.result = ws_result
            self.db = db
            self.objects_by_file = objects_by_file

        def query(self, sql, params=()):
            if params:
                cur = self.db.conn.execute(sql, params)
                cols = [d[0] for d in cur.description] if cur.description else []
                return [dict(zip(cols, row)) for row in cur.fetchall()]
            qr = self.db.query(sql)
            return [dict(zip(qr.columns, row)) for row in qr.rows]

        def close(self):
            self.db.close()

    data = _WorkspaceData()
    yield data
    data.close()


@pytest.fixture
def mock_workspace_db(sample_workspace):
    """Create a WorkspaceData from the sample workspace for testing.

    Uses the real qmd parser to load data, matching the real
    load_workspace() behavior.
    """
    from qmdc_mkdocs.database import load_workspace

    ws_data = load_workspace(sample_workspace)
    yield ws_data
    ws_data.close()


@pytest.fixture
def sample_hints():
    """Sample hints.json data for testing semantic hints."""
    return {
        "myproject:storage:users": [
            {
                "label": "Orders",
                "kind": "Table",
                "file": "storage/tables.qmd.md",
                "score": 0.85,
            },
            {
                "label": "Get Users",
                "kind": "Endpoint",
                "file": "api/endpoints.qmd.md",
                "score": 0.72,
            },
        ],
        "field:name@storage/tables.qmd.md": [
            {
                "label": "User Name Field",
                "kind": "Column",
                "file": "storage/columns.qmd.md",
                "score": 0.68,
            },
        ],
    }
