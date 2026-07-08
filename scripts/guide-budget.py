#!/usr/bin/env python3
"""Guard the agent guide's size against its token budget — fully offline.

The guide (docs/guides/qmdc-guide.qmd.md) is served into every agent context via
MCP `qmdc_get_guide`, so unbounded growth is a per-session tax. This check keeps
it under a hard token budget without calling any API: tokens are ESTIMATED from
character counts with separate densities for prose and fenced code blocks
(prose ~3.9 chars/token, code/tables ~2.9 — calibrated pessimistically so the
estimate errs high, never low).

Exit 1 when the estimate exceeds BUDGET_TOKENS; warn (exit 0) above the soft
threshold so growth is visible before it blocks.
"""

import re
import sys
from pathlib import Path

GUIDE = Path(__file__).resolve().parent.parent / "docs" / "guides" / "qmdc-guide.qmd.md"

BUDGET_TOKENS = 20_000   # hard ceiling — make test fails above this
SOFT_TOKENS = 16_000     # warning threshold — trim before adding more

PROSE_CHARS_PER_TOKEN = 3.9
CODE_CHARS_PER_TOKEN = 2.9

FENCE_RE = re.compile(r"^```.*?^```", re.S | re.M)


def estimate_tokens(text: str) -> tuple[int, int, int]:
    code_chars = sum(len(m) for m in FENCE_RE.findall(text))
    prose_chars = len(text) - code_chars
    tokens = round(prose_chars / PROSE_CHARS_PER_TOKEN + code_chars / CODE_CHARS_PER_TOKEN)
    return tokens, prose_chars, code_chars


def main() -> int:
    text = GUIDE.read_text(encoding="utf-8")
    tokens, prose_chars, code_chars = estimate_tokens(text)
    pct = 100 * tokens / BUDGET_TOKENS
    detail = (
        f"~{tokens:,} tokens estimated ({pct:.0f}% of {BUDGET_TOKENS:,} budget; "
        f"{prose_chars:,} prose chars + {code_chars:,} code chars)"
    )

    if tokens > BUDGET_TOKENS:
        print(f"❌ Guide over token budget: {detail}")
        print(f"   {GUIDE.relative_to(Path.cwd()) if GUIDE.is_relative_to(Path.cwd()) else GUIDE}")
        print("   Trim before adding content — candidates: Practical Examples, long CLI samples.")
        return 1
    if tokens > SOFT_TOKENS:
        print(f"⚠️  Guide approaching token budget: {detail}")
        return 0
    print(f"✅ Guide within token budget: {detail}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
