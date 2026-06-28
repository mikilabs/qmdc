#!/usr/bin/env python3
"""Chunk docs/*.qmd.md into JSONL for tantivy-cli ingest.

One document per markdown heading section.
Fields: path, title, body. Annotation [[id:Kind]] is stripped from the title
but the bare id is appended to the body so it stays searchable.
"""

import json
import re
import sys
from pathlib import Path

HEADING = re.compile(r"^(#{1,6})\s+(.*)$")
ANNOT = re.compile(r"\[\[\s*([^\]:]+?)\s*(?::\s*[^\]]+)?\]\]")


def strip_annot(text: str) -> tuple[str, list[str]]:
    ids = [m.group(1).strip() for m in ANNOT.finditer(text)]
    clean = ANNOT.sub("", text).strip()
    return clean, ids


def chunk_file(path: Path, docs_root: Path) -> list[dict]:
    rel = str(path.relative_to(docs_root))
    lines = path.read_text(encoding="utf-8").splitlines()
    sections: list[dict] = []
    cur_title = rel
    cur_ids: list[str] = []
    cur_line = 1
    buf: list[str] = []

    def flush(start_line: int):
        body = "\n".join(buf).strip()
        title_clean, _ = strip_annot(cur_title)
        # keep ids searchable in body
        extra = " ".join(cur_ids)
        if body or (title_clean and title_clean != rel):
            sections.append(
                {
                    "path": f"{rel}:{start_line}",
                    "title": title_clean,
                    "body": (body + "\n" + extra).strip(),
                }
            )

    for i, line in enumerate(lines, start=1):
        m = HEADING.match(line)
        if m:
            flush(cur_line)
            heading_text = m.group(2)
            cur_title, cur_ids = strip_annot(heading_text)
            cur_ids = [m2.group(1).strip() for m2 in ANNOT.finditer(heading_text)]
            cur_line = i
            buf = []
        else:
            buf.append(line)
    flush(cur_line)
    return sections


def main():
    docs_root = Path(sys.argv[1]).resolve()
    out = []
    for path in sorted(docs_root.rglob("*.qmd.md")):
        if "/tracking/" in str(path):
            continue
        out.extend(chunk_file(path, docs_root))
    for d in out:
        print(json.dumps(d, ensure_ascii=False))
    print(f"chunked {len(out)} sections", file=sys.stderr)


if __name__ == "__main__":
    main()
