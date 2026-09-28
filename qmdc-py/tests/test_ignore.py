"""Data-driven check of the `.qmdcignore` matcher against git's own answers (QMD-73).

Reads `tests/ignore/gitignore-matrix.json`, whose `ignored` lists were produced by git
(`tests/ignore/gen_matrix.py`). The Rust and TypeScript parsers run the same cases through
their ports of the same matcher, so the `ignore` suite has one case per matrix entry in
every language.
"""

import json
from pathlib import Path

import pytest

from qmdc.ignore import is_ignored_relative, parse_qmdcignore

MATRIX = Path(__file__).parent.parent.parent / "tests" / "ignore" / "gitignore-matrix.json"
_DOC = json.loads(MATRIX.read_text(encoding="utf-8"))
_TREE: list[str] = _DOC["tree"]
_CASES: list[dict] = _DOC["cases"]


@pytest.mark.parametrize("case", _CASES, ids=[c["name"] for c in _CASES])
def test_matches_git(case: dict) -> None:
    rules = parse_qmdcignore(case["content"].encode("utf-8"))
    got = sorted(p for p in _TREE if is_ignored_relative(rules, p, False))
    assert got == sorted(case["ignored"])
