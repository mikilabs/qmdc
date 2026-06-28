"""Smoke tests verifying the test infrastructure works."""


def test_workspace_parses(sample_ws_data):
    """Verify the sample workspace parses and loads into DB."""
    assert sample_ws_data.result.workspace_id == "myproject"
    assert len(sample_ws_data.result.files) == 5
    assert len(sample_ws_data.result.objects) >= 7


def test_db_query_works(sample_ws_data):
    """Verify SQL queries work against the loaded DB."""
    rows = sample_ws_data.query("SELECT __id, __kind FROM objects WHERE __kind = 'Table'")
    ids = {r["__id"] for r in rows}
    assert "users" in ids
    assert "orders" in ids


def test_edges_exist(sample_ws_data):
    """Verify edges are populated from references."""
    rows = sample_ws_data.query("SELECT source_id, target_id, edge_type FROM edges")
    assert len(rows) > 0
    # The get_users endpoint references storage:users
    target_ids = [r["target_id"] for r in rows]
    assert any("users" in t for t in target_ids)


def test_objects_by_file(sample_ws_data):
    """Verify objects are grouped by file correctly."""
    assert "storage/tables.qmd.md" in sample_ws_data.objects_by_file
    table_objs = sample_ws_data.objects_by_file["storage/tables.qmd.md"]
    kinds = {obj.get("__kind") for obj in table_objs}
    assert "Table" in kinds


def test_objects_have_references(sample_ws_data):
    """Verify parser provides __references with position data."""
    # Find an object with references
    for obj in sample_ws_data.result.objects:
        refs = obj.get("__references", [])
        if refs:
            ref = refs[0]
            assert "line" in ref
            assert "start_col" in ref
            assert "end_col" in ref
            assert "raw" in ref
            assert "target" in ref
            return
    # At least one object should have references
    assert False, "No objects with __references found"
