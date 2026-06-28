#!/bin/bash
# Round-trip check with normalization
# Usage: ./scripts/roundtrip_check.sh [max_diffs] [max_files]

MAX_DIFFS=${1:-3}
MAX_FILES=${2:-30}

diff_count=0
pass_count=0

normalize() {
  # 1. Normalize spaces around : in [[...]]
  # 2. Normalize table separators
  # 3. Remove all blank lines
  # 4. Remove trailing whitespace
  # 5. Normalize empty table cells
  # 6. Remove quotes around single references
  # 7. Normalize escaped quotes in arrays
  # 8. Normalize empty string values
  # 9. Normalize ordered lists to unordered
  # 10. Normalize escape sequences
  # 11. Remove standalone dash lines
  # 12. Remove all leading spaces from list items (normalize nesting)
  # 13. Normalize 4 backticks to 3
  # 14. Normalize :text suffix in [[id:text]] to [[id]]
  # 15. Normalize array values with commas (add quotes)
  # 16. Normalize indentation (2 or 3 spaces -> none for comparison)
  sed -E 's/\[\[([^]:]+)[[:space:]]*:[[:space:]]*([^]]+)\]\]/[[\1:\2]]/g' | \
  sed -E 's/\|[[:space:]]*-+[[:space:]]*/|---/g' | \
  grep -v '^[[:space:]]*$' | \
  sed 's/[[:space:]]*$//' | \
  sed -E 's/\| \|/|  |/g' | \
  sed -E 's/: "\[\[/: [[/g' | \
  sed -E 's/\]\]"/]]/g' | \
  sed -E 's/\\"/"/g' | \
  sed -E 's/: ""$/:/g' | \
  sed -E 's/^([[:space:]]*)([0-9]+)\. /\1- /g' | \
  sed -E 's/\\_/_/g' | \
  grep -v '^-$' | \
  sed -E 's/^[[:space:]]+//' | \
  sed -E 's/^````/```/g' | \
  sed -E 's/\[\[([^]:]+):text\]\]/[[\1]]/g' | \
  sed -E 's/: \[([^]"]+)\]/: ["\1"]/g' | \
  sed -E 's/\["/[/g' | \
  sed -E 's/"\]/]/g' | \
  sed -E 's/", "/, /g'
}

for f in $(find ../docs -name "*.qmd.md" -type f | head -$MAX_FILES); do
  ./qmdc parse -i "$f" > /tmp/test.json 2>/dev/null
  ./qmdc rebuild < /tmp/test.json > /tmp/test_rebuilt.qmd.md 2>/dev/null
  
  cat "$f" | normalize > /tmp/orig_norm.txt
  cat /tmp/test_rebuilt.qmd.md | normalize > /tmp/rebuilt_norm.txt
  
  if ! diff -q /tmp/orig_norm.txt /tmp/rebuilt_norm.txt > /dev/null 2>&1; then
    diff_count=$((diff_count + 1))
    echo "=== DIFF $diff_count: $f ==="
    diff /tmp/orig_norm.txt /tmp/rebuilt_norm.txt | head -25
    echo ""
    if [ $diff_count -ge $MAX_DIFFS ]; then
      echo "Stopped after $MAX_DIFFS diffs"
      break
    fi
  else
    pass_count=$((pass_count + 1))
  fi
done

echo "---"
echo "Pass: $pass_count, Diff: $diff_count"
