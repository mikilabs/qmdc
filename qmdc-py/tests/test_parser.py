"""Test QMD parser against microtests."""

import json
import re
from pathlib import Path
from typing import Literal

import pytest

from qmdc.parser import parse

MICROTESTS_DIR = Path(__file__).parent.parent.parent / "tests/parser"

ParseFormat = Literal["minimal", "standard", "full"]


def get_microtest_files():
    """Get all microtest pairs (qmd.md, expected.json) with format info."""
    qmdc_files = sorted(MICROTESTS_DIR.glob("*.qmd.md"))
    tests = []

    for qmdc_file in qmdc_files:
        base_name = qmdc_file.stem  # e.g., "031-array-of-objects.qmd"
        base_name = base_name.replace(".qmd", "")  # e.g., "031-array-of-objects"

        # Check for format-specific expected files
        formats = [
            (".expected.json", "standard"),
            (".expected.minimal.json", "minimal"),
            (".expected.full.json", "full"),
        ]

        for suffix, fmt in formats:
            expected_file = MICROTESTS_DIR / (base_name + suffix)
            if expected_file.exists():
                test_id = base_name if fmt == "standard" else f"{base_name}[{fmt}]"
                tests.append((test_id, qmdc_file, expected_file, fmt))

    return tests


def get_standard_tests():
    """Get only standard format tests (for rebuild testing)."""
    return [
        (name, qmd, exp, fmt) for name, qmd, exp, fmt in get_microtest_files() if fmt == "standard"
    ]


@pytest.mark.parametrize("test_name,qmdc_file,expected_file,fmt", get_microtest_files())
def test_microtest_parse(test_name, qmdc_file, expected_file, fmt: ParseFormat):
    """Test parser against a single microtest."""
    # Read input
    markdown = qmdc_file.read_text()

    # Parse with specified format
    result = parse(markdown, format=fmt)

    # Read expected
    expected = json.loads(expected_file.read_text())

    # Compare arrays
    assert result == expected, f"Mismatch in {test_name}"


def normalize_for_rebuild_comparison(data: list) -> list:
    """Normalize JSON for rebuild comparison - remove line numbers from ParsingErrors
    and filter out __ParsingError objects entirely (they can't survive round-trip)."""
    result = []
    for obj in data:
        if isinstance(obj, dict):
            if obj.get("__kind") == "__ParsingError":
                # ParsingErrors can't survive round-trip — skip entirely
                continue
            else:
                result.append(obj)
        else:
            result.append(obj)
    return result


@pytest.mark.parametrize("test_name,qmdc_file,expected_file,fmt", get_standard_tests())
def test_microtest_rebuild(test_name, qmdc_file, expected_file, fmt):
    """Test round-trip: parse -> rebuild -> parse should preserve all data."""
    from qmdc.parser import rebuild

    # Original markdown
    original_markdown = qmdc_file.read_text()

    # Parse original
    json1 = parse(original_markdown)

    # Skip if parsing errors - rebuild of invalid docs is undefined
    if any(obj.get("__kind") == "__ParsingError" for obj in json1 if isinstance(obj, dict)):
        pytest.skip("has parsing errors")

    # Rebuild to canonical form
    canonical = rebuild(json1)

    # Parse canonical
    json2 = parse(canonical)

    # Normalize for comparison (remove line numbers from ParsingErrors)
    json1_normalized = normalize_for_rebuild_comparison(json1)
    json2_normalized = normalize_for_rebuild_comparison(json2)

    # Data must be preserved (JSON should match, except line numbers in errors)
    assert json1_normalized == json2_normalized, (
        f"Data loss in {test_name}\n"
        f"=== ORIGINAL MD ===\n{original_markdown}\n\n"
        f"=== CANONICAL MD ===\n{canonical}\n\n"
        f"=== JSON1 ===\n{json.dumps(json1, indent=2)}\n\n"
        f"=== JSON2 ===\n{json.dumps(json2, indent=2)}"
    )


def get_all_qmdc_files():
    """Get all .qmd.md files (no expected JSON needed). For text round-trip test."""
    qmdc_files = sorted(MICROTESTS_DIR.glob("*.qmd.md"))
    tests = []
    for qmdc_file in qmdc_files:
        base_name = qmdc_file.stem.replace(".qmd", "")
        tests.append((base_name, qmdc_file))
    return tests


def _normalize_for_content_comparison(s: str) -> str:
    """Normalize a string for content-loss comparison.

    Strips:
    - [[...]] bracket tokens (ID/Kind normalization is not content loss)
    - Quotes around values
    - HTML comments (<!-- ... -->)
    - Heading markers (# prefixes)
    - All whitespace
    """
    result = []
    chars = list(s)
    length = len(chars)
    i = 0

    while i < length:
        # Skip HTML comments <!-- ... -->
        if (
            i + 3 < length
            and chars[i] == "<"
            and chars[i + 1] == "!"
            and chars[i + 2] == "-"
            and chars[i + 3] == "-"
        ):
            j = i + 4
            while j + 2 < length:
                if chars[j] == "-" and chars[j + 1] == "-" and chars[j + 2] == ">":
                    j += 3
                    break
                j += 1
            else:
                j = length
            i = j
            continue

        # Skip [[...]] bracket tokens
        if i + 1 < length and chars[i] == "[" and chars[i + 1] == "[":
            j = i + 2
            depth = 1
            while j < length and depth > 0:
                if j + 1 < length and chars[j] == "[" and chars[j + 1] == "[":
                    depth += 1
                    j += 2
                elif j + 1 < length and chars[j] == "]" and chars[j + 1] == "]":
                    depth -= 1
                    j += 2
                else:
                    j += 1
            i = j
            continue

        # Skip quotes
        if chars[i] == '"':
            i += 1
            continue

        result.append(chars[i])
        i += 1

    # Strip heading markers
    text = "".join(result)
    lines = [line.lstrip("#") for line in text.split("\n")]
    text = "\n".join(lines)

    # Normalize table separator rows (|---|---|, |-------|-----|, etc.) to just pipes
    text = re.sub(r"\|[-:]+(?:\|[-:]+)*\|", lambda m: "|" * m.group().count("|"), text)

    # Strip all whitespace
    return "".join(c for c in text if not c.isspace())


def _check_content_loss(original: str, rebuilt: str) -> list[str]:
    """Check if difference between original and rebuilt is normalization-only.

    Returns list of problem descriptions. Empty list means no content loss.

    Detects:
    1. Content loss/reordering: actual text disappears or moves
    2. Heading level changes: ### becomes #### (structural bug)
    """
    orig_lines = original.splitlines()
    rebuilt_lines = rebuilt.splitlines()
    problems = []

    # Check 1: Heading level changes (positional matching)
    orig_headings = []
    for line in orig_lines:
        if line.startswith("#"):
            level = len(line) - len(line.lstrip("#"))
            label = line.lstrip("#").strip()
            orig_headings.append((level, label))

    rebuilt_headings = []
    for line in rebuilt_lines:
        if line.startswith("#"):
            level = len(line) - len(line.lstrip("#"))
            label = line.lstrip("#").strip()
            rebuilt_headings.append((level, label))

    for idx in range(min(len(orig_headings), len(rebuilt_headings))):
        oh = orig_headings[idx]
        rh = rebuilt_headings[idx]
        ol = _normalize_for_content_comparison(oh[1])
        rl = _normalize_for_content_comparison(rh[1])
        if ol == rl and ol and oh[0] != rh[0]:
            problems.append(
                f'  HEADING LEVEL CHANGE: "{oh[1]}" was h{oh[0]}, now h{rh[0]} ("{rh[1]}")'
            )

    # Check 2: Content loss via LCS diff
    n, m = len(orig_lines), len(rebuilt_lines)
    dp = [[0] * (m + 1) for _ in range(n + 1)]
    for i in range(1, n + 1):
        for j in range(1, m + 1):
            if orig_lines[i - 1] == rebuilt_lines[j - 1]:
                dp[i][j] = dp[i - 1][j - 1] + 1
            else:
                dp[i][j] = max(dp[i - 1][j], dp[i][j - 1])

    # Backtrack
    ops = []  # list of ('equal',) | ('removed', line) | ('added', line)
    i, j = n, m
    while i > 0 or j > 0:
        if i > 0 and j > 0 and orig_lines[i - 1] == rebuilt_lines[j - 1]:
            ops.append(("equal",))
            i -= 1
            j -= 1
        elif i > 0 and (j == 0 or dp[i - 1][j] >= dp[i][j - 1]):
            ops.append(("removed", orig_lines[i - 1]))
            i -= 1
        else:
            ops.append(("added", rebuilt_lines[j - 1]))
            j -= 1
    ops.reverse()

    # Group into hunks
    hunks = []
    cur_removed, cur_added = [], []
    for op in ops:
        if op[0] == "equal":
            if cur_removed or cur_added:
                hunks.append((cur_removed[:], cur_added[:]))
                cur_removed.clear()
                cur_added.clear()
        elif op[0] == "removed":
            cur_removed.append(op[1])
        else:
            cur_added.append(op[1])
    if cur_removed or cur_added:
        hunks.append((cur_removed, cur_added))

    for removed, added in hunks:
        removed_text = "\n".join(removed)
        added_text = "\n".join(added)
        rn = _normalize_for_content_comparison(removed_text)
        an = _normalize_for_content_comparison(added_text)
        if rn != an:
            problems.append(
                f"  CONTENT LOSS:\n    REMOVED: {removed_text!r}\n    ADDED:   {added_text!r}"
                f"\n    (normalized: {rn!r} vs {an!r})"
            )

    return problems


@pytest.mark.parametrize("test_name,qmdc_file", get_all_qmdc_files())
def test_microtest_rebuild_text(test_name, qmdc_file):
    """Text-level round-trip test: detects real content loss (not just whitespace/normalization).

    Allowed normalizations (not flagged):
    - Whitespace changes (extra/missing blank lines)
    - [[...]] bracket content changes (ID/Kind normalization)
    - Quote changes around field values
    - HTML comment removal

    Flagged as bugs:
    - Content lines disappearing
    - Content lines reordering
    - Heading level changes (### -> ####)
    """
    from qmdc.parser import rebuild

    markdown = qmdc_file.read_text()
    parsed = parse(markdown)

    # Skip if parsing errors
    if any(obj.get("__kind") == "__ParsingError" for obj in parsed if isinstance(obj, dict)):
        pytest.skip("has parsing errors")

    rebuilt_text = rebuild(parsed)
    problems = _check_content_loss(markdown, rebuilt_text)

    if problems:
        msg = f"Content loss in {test_name}:\n" + "\n\n".join(problems)
        msg += f"\n\n=== ORIGINAL ===\n{markdown}\n=== REBUILT ===\n{rebuilt_text}"
        pytest.fail(msg)


if __name__ == "__main__":
    # Run tests manually
    for test_name, qmdc_file, expected_file, fmt in get_microtest_files():
        print(f"Testing {test_name} (parse, format={fmt})...")
        try:
            test_microtest_parse(test_name, qmdc_file, expected_file, fmt)
            print("  ✓ PASS")
        except AssertionError as e:
            print(f"  ✗ FAIL: {e}")

    for test_name, qmdc_file, expected_file, fmt in get_standard_tests():
        print(f"Testing {test_name} (rebuild)...")
        try:
            test_microtest_rebuild(test_name, qmdc_file, expected_file, fmt)
            print("  ✓ PASS")
        except AssertionError as e:
            print(f"  ✗ FAIL: {e}")
