#!/bin/bash
# Goal-5 file-size discipline lint.
# Checks that no NEW source file (src/core/*, src/mcp/*) exceeds the threshold.
# Pre-existing files (parser.rs, server.rs, workspace.rs, etc.) are excluded.
#
# Threshold: 500 lines for new modules. Files above this should be split.

set -euo pipefail

THRESHOLD=500
ERRORS=0

# New modules introduced by this intent (u1-u4)
NEW_DIRS="src/core src/mcp src/lsp/handlers"

cd "$(dirname "$0")/.."

echo "=== File-size discipline lint (goal-5) ==="
echo "Threshold: ${THRESHOLD} lines for new modules"
echo ""

for dir in $NEW_DIRS; do
    if [ ! -d "$dir" ]; then
        continue
    fi
    while IFS= read -r file; do
        lines=$(wc -l < "$file" | tr -d ' ')
        if [ "$lines" -gt "$THRESHOLD" ]; then
            echo "FAIL: $file ($lines lines > $THRESHOLD)"
            ERRORS=$((ERRORS + 1))
        fi
    done < <(find "$dir" -name '*.rs' -type f)
done

if [ $ERRORS -eq 0 ]; then
    echo "✅ All new modules within ${THRESHOLD}-line limit"
    # Show top 5 for info
    echo ""
    echo "Largest new modules:"
    find $NEW_DIRS -name '*.rs' -type f 2>/dev/null | xargs wc -l | sort -rn | head -5
else
    echo ""
    echo "✗ $ERRORS file(s) exceed the ${THRESHOLD}-line limit"
    exit 1
fi
