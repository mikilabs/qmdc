"""Measure .qmdcignore semantics: three parsers vs git's own .gitignore engine.

For every pattern set, build a throwaway workspace containing the same file tree,
write the patterns as both .qmdcignore and .gitignore, then ask:
  - each parser: which .qmd.md files does `workspace parse` list?
  - git: which files does `git ls-files --others --exclude-standard` list?
git is the oracle because the spec says .qmdcignore "works like .gitignore".
"""

import json
import os
import shutil
import subprocess
import sys
from pathlib import Path

REPO = Path(sys.argv[1])
WORK = Path(sys.argv[2])
IMPLS = sys.argv[3].split(",") if len(sys.argv) > 3 else ["rs", "py", "ts"]

TREE = [
    "a.qmd.md",
    "tests/a.qmd.md",
    "tests/sub/b.qmd.md",
    "tests/sub/deep/c.qmd.md",
    "docs/tests/d.qmd.md",
    "docs/x.qmd.md",
    "docs/sub/y.qmd.md",
    "build/e.qmd.md",
    "src/build/f.qmd.md",
    "test_one.qmd.md",
    "src/test_two.qmd.md",
    "temp/g.qmd.md",
    "notes.draft.qmd.md",
    "src/notes.draft.qmd.md",
    "keep/important.qmd.md",
    "keep/other.qmd.md",
    "tracking/artifacts/w.qmd.md",
    "tracking/a/artifacts/z.qmd.md",
    "tracking/a/b/artifacts/v.qmd.md",
    "tracking/a/notes.qmd.md",
]

CASES = [
    ["tests/*"],
    ["tests/"],
    ["tests"],
    ["/tests"],
    ["tests/**"],
    ["**/tests"],
    ["**/tests/**"],
    ["tests/*/"],
    ["tests/sub"],
    ["docs/*.qmd.md"],
    ["*.draft.qmd.md"],
    ["a.qmd.md"],
    ["/a.qmd.md"],
    ["build/"],
    ["**/test_*.qmd.md"],
    ["test_*.qmd.md"],
    ["temp/"],
    ["src/"],
    ["sub/"],
    ["deep"],
    ["*/sub/"],
    ["docs/**/y.qmd.md"],
    ["te?ts/"],
    ["[bt]uild/"],
    ["tracking/**/artifacts/**"],
    ["keep/*", "!keep/important.qmd.md"],
    ["# only a comment"],
    ["tests/* "],
    ["keep/", "!keep/important.qmd.md"],
    ["keep/**", "!keep/important.qmd.md"],
    ["tests/*", "!tests/a.qmd.md"],
    ["tests/*", "!tests/sub/b.qmd.md"],
    ["!keep/important.qmd.md"],
    ["Tests/"],
    ["src/*.qmd.md"],
    ["**/*.draft.qmd.md"],
    ["tracking/*/artifacts/"],
    ["tests/**/*.qmd.md"],
]


def make_ws(root: Path, patterns: list[str]) -> None:
    if root.exists():
        shutil.rmtree(root)
    root.mkdir(parents=True)
    (root / "readme.qmd.md").write_text("# Matrix [[matrix_ws: __Workspace]]\n", encoding="utf-8")
    for i, rel in enumerate(TREE):
        p = root / rel
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_text(f"## Item [[item_{i}: Thing]]\n\n- value: {i}\n", encoding="utf-8")
    body = "\n".join(patterns) + "\n"
    (root / ".qmdcignore").write_text(body, encoding="utf-8")
    (root / ".gitignore").write_text(body, encoding="utf-8")


def git_kept(root: Path) -> set[str]:
    subprocess.run(["git", "init", "-q"], cwd=root, check=True)
    out = subprocess.run(
        [
            "git",
            "-c",
            "core.excludesFile=/dev/null",
            "-c",
            "core.ignorecase=false",
            "ls-files",
            "--others",
            "--exclude-standard",
        ],
        cwd=root,
        check=True,
        capture_output=True,
        text=True,
    ).stdout
    shutil.rmtree(root / ".git")
    return {line for line in out.splitlines() if line.endswith(".qmd.md") and line != "readme.qmd.md"}


def parser_kept(impl: str, root: Path) -> set[str] | str:
    proc = subprocess.run(
        [str(REPO / "bin" / f"qmdc-{impl}"), "workspace", "parse", str(root)],
        capture_output=True,
        text=True,
    )
    try:
        env = json.loads(proc.stdout)
    except json.JSONDecodeError:
        return f"exit {proc.returncode}: {proc.stderr.strip()[-200:]}"
    files = set(env.get("files", []))
    return {f for f in files if f != "readme.qmd.md"}


def show(kept: set[str]) -> str:
    dropped = sorted(set(TREE) - kept)
    return "{" + ", ".join(dropped) + "}"


results = []
for idx, patterns in enumerate(CASES):
    root = WORK / f"case{idx:02d}"
    make_ws(root, patterns)
    oracle = git_kept(root)
    row = {"patterns": patterns, "git": sorted(set(TREE) - oracle), "impl": {}}
    for impl in IMPLS:
        kept = parser_kept(impl, root)
        row["impl"][impl] = kept if isinstance(kept, str) else sorted(set(TREE) - kept)
    results.append(row)
    agree = [impl for impl in IMPLS if row["impl"][impl] == row["git"]]
    wrong = [impl for impl in IMPLS if row["impl"][impl] != row["git"]]
    print(f"[{idx:02d}] {patterns!r}")
    print(f"      git ignores: {row['git']}")
    print(f"      agree with git: {agree}   wrong: {wrong}")
    for impl in wrong:
        print(f"      {impl} ignores: {row['impl'][impl]}")

(WORK / "matrix.json").write_text(json.dumps(results, indent=2), encoding="utf-8")
total_wrong = {impl: sum(1 for r in results if r["impl"][impl] != r["git"]) for impl in IMPLS}
print("CASES", len(results), "WRONG VS GIT", total_wrong)
