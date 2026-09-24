#!/usr/bin/env python3
"""Compare the FULL `parse` output of all three parsers over a corpus of real documents.

Why this exists
---------------
`scripts/compare_validate_errors.sh` compares only the validation-error LIST -- type,
file:line, objectId. Everything else a parser produces is invisible to it: object fields,
`__comments` and their anchors, `__syntax`, `__types`, field ordering, and the
`__Document` / `__TextBlock` fallback shape.

That blindness let real divergences ship. Two examples from QMD-70, both found by review
rather than by CI: Rust composed a table child's id as `root_items_0` where the other two
produced `items_0`, and TypeScript dropped the separator row of a header-only table. Neither
changed the validation-error list, so `validate-compare` reported the three parsers as
identical while their parse output differed.

Parity over `tests/parser/**` and `tests/cli/**` is already enforced by construction: all
three runners compare against one shared `expected.json`, so a divergence there fails at
least one of them. What was never checked is the REAL corpus -- the project's own `.qmd.md`
documents, which exercise shapes no fixture was written for.

The gate
--------
QMD-71 drove the corpus to ZERO divergent documents, so the gate is now absolute: with no
baseline file present, ANY divergence fails. That is the intended end state, and deleting
`scripts/parse-parity-baseline.json` is what expresses it -- a baseline of `0` would still
invite a future `--update-baseline` to raise it again.

The ratchet remains available for the case it was built for: a known-bad state that cannot be
fixed in one change. Recreate it with `--update-baseline`, and the script then fails only when
the count RISES or a document diverges in a way the file does not record.

Usage
-----
    python3 scripts/compare_parse_output.py [corpus_dir ...]
    python3 scripts/compare_parse_output.py --update-baseline

Stdlib only, to match `scripts/test-report.py`.
"""

from __future__ import annotations

import json
import os
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
BASELINE_FILE = os.path.join(ROOT, "scripts", "parse-parity-baseline.json")
PARSERS = ["rs", "py", "ts"]
DEFAULT_CORPORA = ["docs"]


def find_documents(corpora: list[str]) -> list[str]:
    """Every `.qmd.md` under the given directories, relative to the repo root, sorted."""
    found: list[str] = []
    for corpus in corpora:
        base = os.path.join(ROOT, corpus)
        for dirpath, _dirnames, filenames in os.walk(base):
            for name in sorted(filenames):
                if name.endswith(".qmd.md"):
                    found.append(os.path.relpath(os.path.join(dirpath, name), ROOT))
    return sorted(found)


def parse_with(parser: str, path: str) -> object | None:
    """Parsed JSON from one parser, or None when it fails or emits non-JSON."""
    try:
        proc = subprocess.run(
            [os.path.join(ROOT, "bin", f"qmdc-{parser}"), "parse", "-i", path],
            cwd=ROOT,
            capture_output=True,
            text=True,
            timeout=60,
        )
    except (OSError, subprocess.TimeoutExpired):
        return None
    if proc.returncode != 0:
        return None
    try:
        return json.loads(proc.stdout)
    except json.JSONDecodeError:
        return None


def describe_divergence(outputs: dict[str, object | None]) -> str:
    """Short, stable description of HOW the three disagree, for the report."""
    groups: dict[str, list[str]] = {}
    for parser, out in outputs.items():
        key = "FAILED" if out is None else json.dumps(out, sort_keys=True)
        groups.setdefault(key, []).append(parser)
    if len(groups) == 1:
        return ""
    sides = sorted(groups.values(), key=lambda g: (-len(g), g))
    return " vs ".join("+".join(side) for side in sides)


def main() -> int:
    args = [a for a in sys.argv[1:] if a != "--update-baseline"]
    update = "--update-baseline" in sys.argv[1:]
    corpora = args or DEFAULT_CORPORA

    documents = find_documents(corpora)
    if not documents:
        print(f"❌ No .qmd.md documents found under {', '.join(corpora)}", file=sys.stderr)
        return 2

    divergent: list[tuple[str, str]] = []
    for path in documents:
        outputs = {p: parse_with(p, path) for p in PARSERS}
        detail = describe_divergence(outputs)
        if detail:
            divergent.append((path, detail))

    print("=== Parse-output parity across the three parsers ===")
    print(f"  corpus:    {', '.join(corpora)}")
    print(f"  documents: {len(documents)}")
    print(f"  divergent: {len(divergent)}")

    if divergent:
        print("")
        for path, detail in divergent:
            print(f"    {path}  [{detail}]")

    if update:
        with open(BASELINE_FILE, "w", encoding="utf-8") as handle:
            json.dump(
                {
                    "_comment": (
                        "Documents under the compared corpora whose full `parse` output "
                        "differs between the three parsers. A ratchet: this number may go "
                        "DOWN but never up. See scripts/compare_parse_output.py."
                    ),
                    "corpora": corpora,
                    "max_divergent": len(divergent),
                    # path -> how the three split. Keying on the SHAPE of the divergence, not
                    # only the path, is deliberate: keying on the path alone let a NEW divergence
                    # hide inside an already-divergent file, which is the same blindness this
                    # script exists to remove. Verified by planting one.
                    "known_divergent": {path: detail for path, detail in divergent},
                },
                handle,
                indent=2,
            )
            handle.write("\n")
        print(f"\n✅ Baseline written: max_divergent = {len(divergent)}")
        return 0

    if not os.path.exists(BASELINE_FILE):
        # No baseline: parity is absolute. Reached in QMD-71 and kept that way deliberately --
        # see the module docstring for why the file is deleted rather than set to 0.
        print("")
        if divergent:
            print(f"❌ {len(divergent)} document(s) diverge, and parity is required to be exact:")
            for path, detail in divergent:
                print(f"    {path}  [{detail}]")
            print(
                "\n   Fix the divergence. Only if it genuinely cannot be fixed in this change,"
                "\n   record it with --update-baseline and say why in the task."
            )
            return 1
        print(f"✅ Parse parity is exact across all {len(documents)} documents.")
        return 0

    try:
        with open(BASELINE_FILE, encoding="utf-8") as handle:
            baseline = json.load(handle)
    except (OSError, json.JSONDecodeError):
        print(
            f"\n❌ Unreadable baseline {BASELINE_FILE}."
            "\n   Fix or delete it: with no baseline, parity is required to be exact.",
            file=sys.stderr,
        )
        return 2

    allowed = baseline.get("max_divergent", 0)
    known = baseline.get("known_divergent", {})
    if isinstance(known, list):  # tolerate the older path-only form
        known = {path: None for path in known}

    new = [
        (path, detail)
        for path, detail in divergent
        if path not in known or (known[path] is not None and known[path] != detail)
    ]

    print("")
    if new:
        print(f"❌ {len(new)} document(s) diverge in a way the baseline does not record:")
        for path, detail in new:
            was = known.get(path)
            if was is None:
                print(f"     {path}  [{detail}]  (newly divergent)")
            else:
                print(f"     {path}  [{detail}]  (was: {was})")
        print(
            "\n   A parse-output divergence is a parity defect even when `workspace validate`"
            "\n   agrees. Fix it, or -- if the document is new and the divergence is"
            "\n   pre-existing elsewhere -- record it with --update-baseline and say why."
        )
        return 1

    if len(divergent) > allowed:
        print(f"❌ Divergent count rose: {len(divergent)} > baseline {allowed}")
        return 1

    if len(divergent) < allowed:
        print(
            f"✅ Parse parity improved: {len(divergent)} < baseline {allowed}."
            "\n   Lower the baseline: python3 scripts/compare_parse_output.py --update-baseline"
        )
        return 0

    print(f"✅ Parse parity holds at the baseline ({allowed} known divergent).")
    return 0


if __name__ == "__main__":
    sys.exit(main())
