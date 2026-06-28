"""Tests for QMD SQLite database."""

from qmdc.db import QmdcDatabase


def test_level_stored_in_sqlite():
    """Test that __level is properly stored in SQLite."""
    db = QmdcDatabase()

    # Object at level 1
    obj1 = {
        "__id": "root",
        "__kind": "__Namespace",
        "__label": "Root",
        "__level": 1,
    }

    # Object at level 2
    obj2 = {
        "__id": "parent",
        "__kind": "Container",
        "__label": "Parent",
        "__level": 2,
    }

    # Object at level 3
    obj3 = {
        "__id": "child",
        "__kind": "Item",
        "__label": "Child",
        "__level": 3,
    }

    db.upsert_object(obj1)
    db.upsert_object(obj2)
    db.upsert_object(obj3)

    # Query __level column
    result = db.query("SELECT __id, __level FROM objects ORDER BY __level")

    assert len(result.rows) == 3
    assert result.rows[0] == ["root", 1]
    assert result.rows[1] == ["parent", 2]
    assert result.rows[2] == ["child", 3]
