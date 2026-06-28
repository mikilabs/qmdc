"""Data-driven CLI conformance tests (shared corpus in tests/cli/).

Every parser runs the same corpus so the `cli` suite reaches parity by
construction. Impl-specific CLI tests live in test_cli.py (-> unit-py).
See tests/cli/README.md for the fixture format.
"""

import json
import subprocess
from pathlib import Path

import pytest

CORPUS = Path(__file__).parent.parent.parent / "tests/cli"


def discover_cases():
    if not CORPUS.exists():
        return []
    return sorted(p for p in CORPUS.iterdir() if p.is_dir() and (p / "cmd").exists())


CASES = discover_cases()


@pytest.mark.parametrize("case", CASES, ids=[c.name for c in CASES])
def test_cli_case(case: Path):
    args = (case / "cmd").read_text().split()
    stdin = (case / "stdin").read_text() if (case / "stdin").exists() else None
    exit_expected = int((case / "exit").read_text().strip()) if (case / "exit").exists() else 0

    result = subprocess.run(
        ["qmdc", *args],
        input=stdin,
        capture_output=True,
        text=True,
        cwd=str(case),
    )

    assert result.returncode == exit_expected, (
        f"exit {result.returncode} != expected {exit_expected}\nstderr: {result.stderr}"
    )

    exp_json = case / "expected.json"
    exp_txt = case / "expected.txt"
    if exp_json.exists():
        expected = json.loads(exp_json.read_text())
        actual = json.loads(result.stdout)
        assert actual == expected, f"stdout JSON mismatch for {case.name}"
    elif exp_txt.exists():
        assert (
            result.stdout.replace("\r\n", "\n").strip()
            == exp_txt.read_text().replace("\r\n", "\n").strip()
        ), f"stdout text mismatch for {case.name}"
