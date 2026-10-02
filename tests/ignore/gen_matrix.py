"""Regenerate `gitignore-matrix.json`: the `.qmdcignore` acceptance data (QMD-73).

Every expected answer comes from git itself. The tree below is created in a throwaway
repository; for each case the case's `content` is written, byte for byte, as `.gitignore`,
and `git ls-files --others --exclude-standard` lists what git keeps. `ignored` is the rest
of the tree. Each parser's test feeds `content` to its own matcher and must ignore exactly
the same paths.

To add a case, append it to CASES and run from the repository root:

    uv run --no-project python tests/ignore/gen_matrix.py
"""

import json
import subprocess
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent

TREE = [
    "a.qmd.md",
    "tests/a.qmd.md",
    "tests/sub/b.qmd.md",
    "tests/sub/deep/c.qmd.md",
    "docs/tests/d.qmd.md",
    "docs/x.qmd.md",
    "docs/sub/y.qmd.md",
    "docs/café.qmd.md",
    "build/e.qmd.md",
    "src/build/f.qmd.md",
    "guild/h.qmd.md",
    "test_one.qmd.md",
    "src/test_two.qmd.md",
    "temp/g.qmd.md",
    "notes.draft.qmd.md",
    "src/notes.draft.qmd.md",
    "notes/with space.qmd.md",
    "keep/important.qmd.md",
    "keep/other.qmd.md",
    "tracking/artifacts/w.qmd.md",
    "tracking/a/artifacts/z.qmd.md",
    "tracking/a/b/artifacts/v.qmd.md",
    "tracking/a/notes.qmd.md",
    "lit/[x].qmd.md",
    "lit/x.qmd.md",
    "2024.qmd.md",
    "#hash.qmd.md",
]

# (name, exact .qmdcignore content). The name is the test-case name in every parser.
CASES = [
    # The line forms measured during triage.
    ("dir-star", "tests/*\n"),
    ("trailing-slash-any-depth", "tests/\n"),
    ("bare-name", "tests\n"),
    ("leading-slash-dir", "/tests\n"),
    ("trailing-double-star", "tests/**\n"),
    ("leading-double-star", "**/tests\n"),
    ("double-star-both-sides", "**/tests/**\n"),
    ("star-slash-dirs-only", "tests/*/\n"),
    ("plain-path", "tests/sub\n"),
    ("star-one-level", "docs/*.qmd.md\n"),
    ("star-file-any-depth", "*.draft.qmd.md\n"),
    ("file-name-any-depth", "a.qmd.md\n"),
    ("leading-slash-file", "/a.qmd.md\n"),
    ("dir-name-any-depth", "build/\n"),
    ("double-star-file-glob", "**/test_*.qmd.md\n"),
    ("file-glob-any-depth", "test_*.qmd.md\n"),
    ("dir-at-root", "temp/\n"),
    ("dir-with-children", "src/\n"),
    ("nested-dir-name", "sub/\n"),
    ("deep-bare-name", "deep\n"),
    ("star-then-dir", "*/sub/\n"),
    ("double-star-middle", "docs/**/y.qmd.md\n"),
    ("question-mark", "te?ts/\n"),
    ("bracket", "[bt]uild/\n"),
    ("double-star-both-ends", "tracking/**/artifacts/**\n"),
    ("negation", "keep/*\n!keep/important.qmd.md\n"),
    ("comment-only", "# only a comment\n"),
    ("trailing-space", "tests/* \n"),
    ("no-reinclude-under-excluded-dir", "keep/\n!keep/important.qmd.md\n"),
    ("reinclude-under-double-star", "keep/**\n!keep/important.qmd.md\n"),
    ("reinclude-direct-child", "tests/*\n!tests/a.qmd.md\n"),
    ("no-reinclude-in-excluded-child-dir", "tests/*\n!tests/sub/b.qmd.md\n"),
    ("lone-negation", "!keep/important.qmd.md\n"),
    ("case-sensitive", "Tests/\n"),
    ("star-one-level-src", "src/*.qmd.md\n"),
    ("double-star-file-any-depth", "**/*.draft.qmd.md\n"),
    ("star-segment-dir", "tracking/*/artifacts/\n"),
    ("double-star-then-glob", "tests/**/*.qmd.md\n"),
    # Forms added for the implementation: git's own edge cases.
    ("literal-prefix-then-double-star", "te**/c.qmd.md\n"),
    ("leading-space-is-significant", " tests/\n"),
    ("escaped-trailing-space", "tests/*\\ \n"),
    ("tab-is-not-trimmed", "tests/*\t\n"),
    ("hash-is-a-comment", "#hash.qmd.md\n"),
    ("escaped-hash", "\\#hash.qmd.md\n"),
    ("question-never-matches-slash", "tests?sub\n"),
    ("posix-class", "[[:digit:]]*.qmd.md\n"),
    ("range-any-depth", "[0-9]*\n"),
    ("negated-bracket-bang", "[!b]uild/\n"),
    ("negated-bracket-caret", "[^g]uild/\n"),
    ("bracket-range", "[a-c]*.qmd.md\n"),
    ("escaped-star", "\\*.qmd.md\n"),
    ("space-in-name", "notes/with space.qmd.md\n"),
    ("escaped-space-in-name", "notes/with\\ space.qmd.md\n"),
    ("escaped-brackets", "lit/\\[x\\].qmd.md\n"),
    ("bracket-is-a-class", "lit/[x].qmd.md\n"),
    ("question-is-one-byte", "docs/caf?.qmd.md\n"),
    ("two-questions-for-two-bytes", "docs/caf??.qmd.md\n"),
    ("double-star-matches-no-dir", "docs/**/x.qmd.md\n"),
    ("double-star-around-dir", "**/sub/**\n"),
    ("double-star-alone", "**\n"),
    ("star-alone", "*\n"),
    ("glob-then-negated-name", "*.qmd.md\n!a.qmd.md\n"),
    ("excluded-dir-wins-over-negation", "tests/\n!tests/a.qmd.md\n"),
    ("everything-at-root-but-one-dir", "/*\n!/tests\n"),
    ("all-dirs-but-one", "*/\n!docs/\n"),
    ("crlf-line-endings", "tests/*\r\n!tests/a.qmd.md\r\n"),
    ("byte-order-mark", "\ufefftests/*\n"),
    ("blank-and-space-only-lines", "\n   \n# c\n"),
    ("bang-alone", "!\n"),
    ("slash-alone", "/\n"),
    ("dir-only-never-matches-a-file", "a.qmd.md/\n"),
    ("every-dir", "**/\n"),
    ("dirs-under-one-dir", "docs/*/\n"),
    ("no-final-newline", "keep/*"),
    # Forms added after review: each one fails for a matcher that gets `**`, case,
    # trailing-space trimming or the byte-order mark wrong. A matcher where EVERY `**`
    # crosses `/` passes every case above this line, which is what these pin down.
    ("double-star-not-followed-by-slash", "tracking/**artifacts/*\n"),
    ("double-star-after-glob-segment", "t*s/su**/c.qmd.md\n"),
    ("case-sensitive-file", "A.qmd.md\n"),
    ("trailing-space-on-negation", "keep/*\n!keep/important.qmd.md  \n"),
    ("byte-order-mark-then-anchored", "\ufeff/a.qmd.md\n"),
]


def git_ignored(root: Path, content: str) -> list[str]:
    (root / ".gitignore").write_bytes(content.encode("utf-8"))
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
        cwd=root,
        check=True,
        capture_output=True,
    ).stdout
    kept = {p.decode("utf-8") for p in out.split(b"\0") if p}
    return sorted(p for p in TREE if p not in kept)


def main() -> None:
    names = [name for name, _ in CASES]
    assert len(names) == len(set(names)), "case names must be unique"
    version = subprocess.run(
        ["git", "--version"], check=True, capture_output=True, text=True
    ).stdout.strip()
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        subprocess.run(["git", "init", "-q"], cwd=root, check=True)
        for rel in TREE:
            path = root / rel
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text("## Item\n", encoding="utf-8")
        cases = [
            {"name": name, "content": content, "ignored": git_ignored(root, content)}
            for name, content in CASES
        ]
    doc = {
        "_comment": (
            "Generated by gen_matrix.py; do not edit by hand. `ignored` is what git ignores "
            "for `content` as .gitignore (git ls-files --others --exclude-standard, "
            f"core.ignorecase=false, {version})."
        ),
        "tree": TREE,
        "cases": cases,
    }
    out = HERE / "gitignore-matrix.json"
    out.write_text(json.dumps(doc, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    print(f"wrote {out.name}: {len(TREE)} paths, {len(cases)} cases ({version})")


if __name__ == "__main__":
    main()
