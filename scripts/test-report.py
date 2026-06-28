#!/usr/bin/env python3
"""Unified, feature-based test-report aggregator for the qmdc monorepo.

Reads every JUnit XML in ``test-reports/`` (pytest, cargo-nextest, the bespoke TS
runners, and the Rust per-case canonical reports) and builds a **feature matrix**:
rows = canonical suites, columns = parser languages (py / ts / rs). Shared
data-driven suites (``parser``, ``workspace``, ``sql``, ``cli``) are expected to
run the *same* cases in every language; ``lsp`` / ``mcp`` are Rust-only by design.

It FAILS the build when:

* any testcase failed/errored;
* a (suite, language) pair that ``require_present`` lists ran 0 cases — the guard
  against a mispointed fixture path silently dropping a whole suite;
* a suite in ``enforce_parity`` has unequal case counts across its languages.

Config lives in ``scripts/test-baseline.json``. Stdlib only; run via
``uv run --no-project python scripts/test-report.py``.
"""

from __future__ import annotations

import glob
import json
import os
import sys
import xml.etree.ElementTree as ET

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
REPORT_DIR = os.path.join(ROOT, "test-reports")
CONFIG_FILE = os.path.join(ROOT, "scripts", "test-baseline.json")

LANGS = ["py", "ts", "rs"]
CONFORMANCE = ["parser", "workspace", "sql", "cli", "lsp", "mcp"]
COMPONENTS = ["mkdocs", "semantic", "vscode"]

# TypeScript JUnit report file stem -> canonical suite.
TS_SUITE = {
    "ts-parser": "parser",
    "ts-workspace": "workspace",
    "ts-sql": "sql",
    "ts-cliconf": "cli",
    "ts-cli": "unit-ts",
}

# Rust test functions whose per-case results come from canonical rs-*.xml reports;
# their (function-level) entries in the nextest rs.xml are skipped to avoid double-count.
RS_CANONICAL_TESTS = {
    "test_all_microtests",
    "test_all_microtests_rebuild",
    "test_all_microtests_rebuild_text",
    "test_all_sql_queries",
    "test_parse_all_workspaces",
    "test_workspace_conformance",
    "test_all_lsp_microtests",
    "mcp_fixture_tests",
    "mcp_resource_fixture_tests",
    "test_cli_conformance",
}

# Map a Rust nextest binary (classname after the "qmdc::" prefix; "" = lib) to a suite.
# Leftover (non-canonical) functions in a mixed binary fall here.
RS_BINARY_SUITE = {
    # cli_unit.rs holds impl-specific CLI tests; the shared corpus lives in
    # cli_conformance.rs (canonical "cli").
    "cli_unit": "unit-rs",
    # workspace_unit.rs holds impl-specific workspace tests; the data-driven
    # conformance corpus lives in workspace_conformance.rs (canonical "workspace").
    "workspace_unit": "unit-rs",
    "tree_modes": "unit-rs",
    "lsp": "lsp",
    "lsp_smoke": "lsp",
    "mcp": "mcp",
    "mcp_force_root": "mcp",
    # sql.rs's shared SQL loop is canonical; its remaining fns are impl-unit.
    "sql": "unit-rs",
    # SQL-rewrite is a Rust-only (LSP) concern, not part of the shared sql corpus.
    "sql_rewrite": "unit-rs",
}


def _int(v: str | None) -> int:
    try:
        return int(v) if v else 0
    except ValueError:
        return 0


def py_suite(classname: str) -> str:
    table = [
        ("tests.test_parser", "parser"),
        ("tests.test_sql", "sql"),
        ("tests.test_workspace.TestWorkspace", "workspace"),
        ("tests.test_workspace", "unit-py"),
        ("tests.test_cli_conformance", "cli"),
        ("tests.test_cli", "unit-py"),
        ("tests.test_db", "unit-py"),
    ]
    for prefix, suite in table:
        if classname.startswith(prefix):
            return suite
    return "unit-py"


def rs_suite(classname: str, name: str) -> str | None:
    if name in RS_CANONICAL_TESTS:
        return None  # represented by rs-*.xml canonical reports
    binary = classname[len("qmdc::") :] if classname.startswith("qmdc::") else ""
    return RS_BINARY_SUITE.get(binary, "unit-rs")


def iter_cases(path: str):
    """Yield (classname, name, failed:bool) for each testcase in a JUnit file."""
    root = ET.parse(path).getroot()
    for tc in root.iter("testcase"):
        failed = tc.find("failure") is not None or tc.find("error") is not None
        yield tc.get("classname", ""), tc.get("name", ""), failed


def main() -> int:
    if not os.path.isdir(REPORT_DIR):
        print(f"✗ no test-reports/ directory at {REPORT_DIR}", file=sys.stderr)
        return 1

    cfg = {"enforce_parity": [], "require_present": {}}
    if os.path.exists(CONFIG_FILE):
        with open(CONFIG_FILE) as fh:
            cfg.update(json.load(fh))

    # matrix[suite][lang] = passed-case count;  failures accumulates everywhere.
    matrix: dict[str, dict[str, int]] = {}
    components: dict[str, int] = {}
    failures = 0
    problems: list[str] = []

    def add(suite: str, lang: str, n: int = 1) -> None:
        matrix.setdefault(suite, {}).setdefault(lang, 0)
        matrix[suite][lang] += n

    for path in sorted(glob.glob(os.path.join(REPORT_DIR, "*.xml"))):
        fname = os.path.splitext(os.path.basename(path))[0]
        for classname, name, failed in iter_cases(path):
            if failed:
                failures += 1
            if fname == "py":
                add(py_suite(classname), "py")
            elif fname == "rs":
                suite = rs_suite(classname, name)
                if suite is not None:
                    add(suite, "rs")
            elif fname.startswith("rs-"):  # rust canonical per-case (classname = suite)
                add(classname, "rs")
            elif fname.startswith("ts-"):
                add(TS_SUITE.get(fname, "unit-ts"), "ts")
            elif fname in COMPONENTS:
                components[fname] = components.get(fname, 0) + 1
            else:
                problems.append(
                    f"unrecognized report file 'test-reports/{fname}.xml' — "
                    "not mapped to any suite (rename/typo?)"
                )
                break

    # ---- render ----
    print()
    print("=" * 64)
    print("  UNIFIED TEST REPORT — conformance matrix (cases per language)")
    print("=" * 64)
    print(f"  {'suite':<12} {'py':>6} {'ts':>6} {'rs':>6}   parity")
    print("  " + "-" * 60)
    enforce = set(cfg.get("enforce_parity", []))
    require = cfg.get("require_present", {})
    total = 0
    for suite in CONFORMANCE:
        row = matrix.get(suite, {})
        cells = {lang: row.get(lang, 0) for lang in LANGS}
        total += sum(cells.values())
        present = {lang: cells[lang] for lang in LANGS if cells[lang] > 0}
        # presence guard
        for lang in require.get(suite, []):
            if cells[lang] == 0:
                problems.append(f"suite '{suite}' missing in {lang} (0 cases)")
        # parity status
        vals = set(present.values())
        if len(present) <= 1:
            parity = "rs-only" if present else "—"
        elif len(vals) == 1:
            parity = "✓"
        else:
            parity = "✗ ENFORCED" if suite in enforce else "✗ (shown)"
            if suite in enforce:
                problems.append(
                    f"parity mismatch in '{suite}': "
                    + ", ".join(f"{k}={v}" for k, v in cells.items())
                )

        def cell(lang: str) -> str:
            return str(cells[lang]) if cells[lang] else "—"

        print(f"  {suite:<12} {cell('py'):>6} {cell('ts'):>6} {cell('rs'):>6}   {parity}")

    print("  " + "-" * 60)
    print("\n  unit / component suites (no parity):")
    for suite in sorted(s for s in matrix if s.startswith("unit-")):
        cnt = sum(matrix[suite].values())
        total += cnt
        print(f"    {suite:<14} {cnt}")
    for comp in COMPONENTS:
        if comp in components:
            total += components[comp]
            print(f"    {comp:<14} {components[comp]}")
    # presence guard for component suites (anti-vacuous: a listed component that
    # produced 0 cases — e.g. its report never got written — fails the gate).
    for comp in cfg.get("require_component", []):
        if components.get(comp, 0) == 0:
            problems.append(f"component suite '{comp}' missing (0 cases)")
    print("=" * 64)
    print(f"  TOTAL cases: {total}   failures: {failures}")
    print("=" * 64)

    if failures:
        problems.append(f"{failures} testcase failure(s)/error(s)")
    if problems:
        print("\n✗ TEST REPORT FAILED:")
        for p in problems:
            print(f"  - {p}")
        return 1

    print(f"\n✅ Unified report OK: {total} cases, 0 failures.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
