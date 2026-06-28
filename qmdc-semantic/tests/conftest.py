"""Pytest configuration and fixtures."""

from pathlib import Path

import pytest


@pytest.fixture
def mini_workspace() -> Path:
    """Path to mini test workspace."""
    return Path(__file__).parent / "fixtures" / "mini-workspace"


@pytest.fixture
def golden_dir() -> Path:
    """Path to golden files directory."""
    return Path(__file__).parent / "golden"


@pytest.fixture
def temp_workspace(tmp_path) -> Path:
    """Create a temporary workspace for tests."""
    workspace = tmp_path / "test-workspace"
    workspace.mkdir()

    # Create minimal workspace structure
    readme = workspace / "readme.qmd.md"
    readme.write_text("""# Test Workspace [[test_workspace: __Workspace]]

A test workspace for unit tests.

## Feature 1 [[feature1: Feature]]

A test feature.

- status: planned
- priority: high

### Description

This is a test feature for unit testing the semantic search.
It has some text content that should be chunked and embedded.

## Feature 2 [[feature2: Feature]]

Another test feature.

- status: done
- priority: low

### Notes

Short notes about this feature.
""")

    return workspace
