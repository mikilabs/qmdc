"""Generate the QMD-73 .qmdcignore regression fixtures.

Every `_expected.json` file list is computed by git itself: the fixture is copied to a
scratch directory, its `.qmdcignore` is installed as `.gitignore`, and
`git ls-files --others --exclude-standard` reports what git keeps. Nothing in the
expectations is typed by hand.

Usage: python gen_fixtures.py <repo_root> <scratch_dir>
"""

import json
import shutil
import subprocess
import sys
from pathlib import Path

REPO = Path(sys.argv[1])
SCRATCH = Path(sys.argv[2])
GROUP = REPO / "tests" / "workspace" / "qmdcignore-gitignore"

FIXTURES = [
    {
        "dir": "01-directory-star-hides-descendants",
        "ws": "qmd73_dir_star",
        "title": "Ignore: a star that matches a directory hides its descendants",
        "prose": (
            "QMD-73 regression. The repository root ignores its fixtures with the line\n"
            "`tests/*`; this fixture uses the same shape as `examples/*`. In git the star\n"
            "matches the directory `examples/sub` itself, and a matched directory hides\n"
            "everything below it, so `examples/sub/mid.qmd.md` is ignored too."
        ),
        "ignore": ["examples/*"],
        "files": {
            "kept.qmd.md": "kept",
            "examples/top.qmd.md": "examples_top",
            "examples/sub/mid.qmd.md": "examples_mid",
        },
    },
    {
        "dir": "02-directory-match-hides-contents",
        "ws": "qmd73_dir_match",
        "title": "Ignore: a line that names a directory hides its contents",
        "prose": (
            "QMD-73 regression. Neither line has a trailing slash or a glob, and both name\n"
            "a directory: `generated` (a bare name) and `examples/sub` (a path). In git a\n"
            "line that matches a directory excludes everything inside it, while a sibling\n"
            "of the named directory stays."
        ),
        "ignore": ["generated", "examples/sub"],
        "files": {
            "generated/a.qmd.md": "generated_a",
            "examples/sub/c.qmd.md": "examples_sub_c",
            "examples/kept.qmd.md": "examples_kept",
        },
    },
    {
        "dir": "03-star-stays-in-one-directory",
        "ws": "qmd73_star_one_level",
        "title": "Ignore: a star never crosses a slash",
        "prose": (
            "QMD-73 regression. `docs/*.qmd.md` names the files directly inside `docs/`.\n"
            "In git a star never matches a slash and `docs/sub` is not matched by the\n"
            "line, so `docs/sub/nested.qmd.md` stays."
        ),
        "ignore": ["docs/*.qmd.md"],
        "files": {
            "docs/top.qmd.md": "docs_top",
            "docs/sub/nested.qmd.md": "docs_nested",
        },
    },
    {
        "dir": "04-anchoring",
        "ws": "qmd73_anchoring",
        "title": "Ignore: where a line is anchored",
        "prose": (
            "QMD-73 regression. A line with no slash except a trailing one matches at any\n"
            "depth: `build/` hides `src/build`, `notes.qmd.md` hides `src/notes.qmd.md`.\n"
            "A leading slash anchors the line to the directory of the ignore file:\n"
            "`/top.qmd.md` hides the root `top.qmd.md` and keeps `src/top.qmd.md`."
        ),
        "ignore": ["build/", "notes.qmd.md", "/top.qmd.md"],
        "files": {
            "src/build/b.qmd.md": "src_build_b",
            "src/notes.qmd.md": "src_notes",
            "top.qmd.md": "top",
            "src/top.qmd.md": "src_top",
        },
    },
    {
        "dir": "05-negation-re-includes",
        "ws": "qmd73_negation",
        "title": "Ignore: a negated line re-includes a path",
        "prose": (
            "QMD-73 regression. `keep/*` hides both files in `keep/`, and the negated line\n"
            "that follows brings `keep/important.qmd.md` back. The directory `keep/` itself\n"
            "is not excluded, so git allows the re-include. `outside.qmd.md` is named by\n"
            "neither line and stays."
        ),
        "ignore": ["keep/*", "!keep/important.qmd.md"],
        "files": {
            "keep/important.qmd.md": "keep_important",
            "keep/other.qmd.md": "keep_other",
            "outside.qmd.md": "outside",
        },
    },
    {
        "dir": "06-no-re-include-under-excluded-directory",
        "ws": "qmd73_no_reinclude",
        "title": "Ignore: a negated line cannot re-include below an excluded directory",
        "prose": (
            "QMD-73 guard. `keep/` excludes the directory itself, and git does not look\n"
            "inside an excluded directory, so the negated line that follows cannot bring\n"
            "`keep/important.qmd.md` back: both files in `keep/` stay hidden. A matcher\n"
            "that decides each file by the last matching line alone gets this wrong."
        ),
        "ignore": ["keep/", "!keep/important.qmd.md"],
        "files": {
            "keep/important.qmd.md": "keep_important",
            "keep/other.qmd.md": "keep_other",
            "outside.qmd.md": "outside",
        },
    },
]


def write_fixture(root: Path, fx: dict) -> None:
    root.mkdir(parents=True, exist_ok=False)
    (root / "readme.qmd.md").write_text(
        f"# {fx['title']} [[{fx['ws']}: __Workspace]]\n\n{fx['prose']}\n", encoding="utf-8"
    )
    (root / ".qmdcignore").write_text("\n".join(fx["ignore"]) + "\n", encoding="utf-8")
    for n, (rel, obj_id) in enumerate(sorted(fx["files"].items()), start=1):
        p = root / rel
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_text(f"## Item {obj_id} [[{obj_id}: Thing]]\n\n- value: {n}\n", encoding="utf-8")


def git_kept(fixture: Path, name: str) -> list[str]:
    copy = SCRATCH / name
    if copy.exists():
        shutil.rmtree(copy)
    shutil.copytree(fixture, copy)
    shutil.copyfile(copy / ".qmdcignore", copy / ".gitignore")
    subprocess.run(["git", "init", "-q"], cwd=copy, check=True)
    out = subprocess.run(
        [
            "git",
            "-c",
            "core.excludesFile=/dev/null",
            "-c",
            "core.ignorecase=false",
            "ls-files",
            "-z",
            "--others",
            "--exclude-standard",
        ],
        cwd=copy,
        check=True,
        capture_output=True,
    ).stdout
    # `-z`, as in tests/ignore/gen_matrix.py: without it git quotes a name that is not plain
    # ASCII (`"docs/caf\303\251.qmd.md"`) and a newline in a name would split into two entries.
    names = (p.decode("utf-8") for p in out.split(b"\0") if p)
    return sorted(name for name in names if name.endswith(".qmd.md"))


SCRATCH.mkdir(parents=True, exist_ok=True)
for fx in FIXTURES:
    root = GROUP / fx["dir"]
    if root.exists():
        # Existing fixtures are left alone; delete one to regenerate it.
        continue
    write_fixture(root, fx)
    kept = git_kept(root, fx["dir"])
    ids = sorted(fx["files"][f] for f in kept if f != "readme.qmd.md")
    expected = {
        "workspace_id": fx["ws"],
        "files": kept,
        "objects": {"Thing": ids, "__Workspace": [fx["ws"]]},
        "errors": [],
        "_comment": (
            "QMD-73: `files` is what `git ls-files --others --exclude-standard` keeps "
            "when this .qmdcignore is installed as .gitignore."
        ),
    }
    (root / "_expected.json").write_text(json.dumps(expected, indent=2) + "\n", encoding="utf-8")
    dropped = sorted(set(fx["files"]) - set(kept))
    print(f"{fx['dir']}: git keeps {kept} | git ignores {dropped}")
