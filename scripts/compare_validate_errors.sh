#!/bin/bash
# Compare workspace validation errors across all three parsers (Rust, Python, TypeScript)
# Usage: ./scripts/compare_validate_errors.sh [workspace_path]
#
# This script runs `workspace validate` on each parser and compares:
# 1. Number of errors
# 2. Error types
# 3. Individual errors (normalized for comparison)
#
# Exit codes:
# 0 - All parsers produce identical errors
# 1 - Parsers produce different errors (diff shown)
# 2 - Script error (parser failed to run, etc.)

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"

WORKSPACE_PATH="${1:-docs}"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}═══════════════════════════════════════════════════════════════${NC}"
echo -e "${BLUE}  Comparing Workspace Validate Errors${NC}"
echo -e "${BLUE}  Workspace: ${WORKSPACE_PATH}${NC}"
echo -e "${BLUE}═══════════════════════════════════════════════════════════════${NC}"
echo ""

# Create temp directory for outputs
TMP_DIR=$(mktemp -d)
trap "rm -rf $TMP_DIR" EXIT

# Run each parser
echo -e "${YELLOW}Running Rust parser...${NC}"
"$ROOT_DIR/bin/qmdc-rs" workspace validate "$WORKSPACE_PATH" > "$TMP_DIR/rs_raw.json" 2>&1 || true

echo -e "${YELLOW}Running Python parser...${NC}"
"$ROOT_DIR/bin/qmdc-py" workspace validate "$WORKSPACE_PATH" > "$TMP_DIR/py_raw.json" 2>&1 || true

echo -e "${YELLOW}Running TypeScript parser...${NC}"
"$ROOT_DIR/bin/qmdc-ts" workspace validate "$WORKSPACE_PATH" > "$TMP_DIR/ts_raw.json" 2>&1 || true

echo ""

# Count errors in each output
RS_COUNT=$(grep -c '"type":' "$TMP_DIR/rs_raw.json" 2>/dev/null || echo "0")
PY_COUNT=$(grep -c '"type":' "$TMP_DIR/py_raw.json" 2>/dev/null || echo "0")
TS_COUNT=$(grep -c '"type":' "$TMP_DIR/ts_raw.json" 2>/dev/null || echo "0")

echo -e "${BLUE}Error counts:${NC}"
echo "  Rust:       $RS_COUNT"
echo "  Python:     $PY_COUNT"
echo "  TypeScript: $TS_COUNT"
echo ""

# Extract unique error types
echo -e "${BLUE}Error types by parser:${NC}"

echo -e "  ${YELLOW}Rust:${NC}"
grep '"type":' "$TMP_DIR/rs_raw.json" 2>/dev/null | sed 's/.*"type": "\([^"]*\)".*/\1/' | sort | uniq -c | sort -rn | head -10 | while read count type; do
    echo "    $count × $type"
done

echo -e "  ${YELLOW}Python:${NC}"
grep '"type":' "$TMP_DIR/py_raw.json" 2>/dev/null | sed 's/.*"type": "\([^"]*\)".*/\1/' | sort | uniq -c | sort -rn | head -10 | while read count type; do
    echo "    $count × $type"
done

echo -e "  ${YELLOW}TypeScript:${NC}"
grep '"type":' "$TMP_DIR/ts_raw.json" 2>/dev/null | sed 's/.*"type": "\([^"]*\)".*/\1/' | sort | uniq -c | sort -rn | head -10 | while read count type; do
    echo "    $count × $type"
done

echo ""

# Normalize errors for comparison (sort by file, line, type)
# Extract key fields: type, file, line, objectId
normalize_errors() {
    local input="$1"
    python3 << EOF
import json
import sys

try:
    with open("$input", "r") as f:
        data = json.load(f)
except:
    print("[]", file=sys.stderr)
    sys.exit(0)

if not isinstance(data, list):
    data = []

# Normalize each error to comparable format
normalized = []
for err in data:
    if not isinstance(err, dict):
        continue
    key = {
        "type": err.get("type", ""),
        "file": err.get("file", ""),
        "line": err.get("line", 0),
        "objectId": err.get("objectId", ""),
        "message": err.get("message", ""),
    }
    normalized.append(key)

# Sort by file, line, type for consistent comparison
normalized.sort(key=lambda x: (x["file"], x["line"], x["type"]))

for err in normalized:
    print(f'{err["type"]}|{err["file"]}:{err["line"]}|{err["objectId"]}')
EOF
}

echo -e "${BLUE}Normalizing errors for comparison...${NC}"
normalize_errors "$TMP_DIR/rs_raw.json" > "$TMP_DIR/rs_normalized.txt" 2>/dev/null
normalize_errors "$TMP_DIR/py_raw.json" > "$TMP_DIR/py_normalized.txt" 2>/dev/null  
normalize_errors "$TMP_DIR/ts_raw.json" > "$TMP_DIR/ts_normalized.txt" 2>/dev/null

# Compare Python vs TypeScript (should be identical or very close)
echo ""
echo -e "${BLUE}Comparing Python vs TypeScript:${NC}"
if diff -q "$TMP_DIR/py_normalized.txt" "$TMP_DIR/ts_normalized.txt" > /dev/null 2>&1; then
    echo -e "  ${GREEN}✅ IDENTICAL${NC}"
    PY_TS_MATCH=true
else
    echo -e "  ${RED}❌ DIFFERENT${NC}"
    echo ""
    echo "  Differences (Python vs TypeScript):"
    diff "$TMP_DIR/py_normalized.txt" "$TMP_DIR/ts_normalized.txt" | head -30 || true
    PY_TS_MATCH=false
fi

# Compare Rust vs Python
echo ""
echo -e "${BLUE}Comparing Rust vs Python:${NC}"
if diff -q "$TMP_DIR/rs_normalized.txt" "$TMP_DIR/py_normalized.txt" > /dev/null 2>&1; then
    echo -e "  ${GREEN}✅ IDENTICAL${NC}"
    RS_PY_MATCH=true
else
    echo -e "  ${RED}❌ DIFFERENT${NC}"
    RS_PY_MATCH=false
    
    echo ""
    echo "  Errors ONLY in Rust (not in Python):"
    comm -23 <(sort "$TMP_DIR/rs_normalized.txt") <(sort "$TMP_DIR/py_normalized.txt") | head -20
    
    echo ""
    echo "  Errors ONLY in Python (not in Rust):"
    comm -13 <(sort "$TMP_DIR/rs_normalized.txt") <(sort "$TMP_DIR/py_normalized.txt") | head -20
fi

# Compare Rust vs TypeScript
echo ""
echo -e "${BLUE}Comparing Rust vs TypeScript:${NC}"
if diff -q "$TMP_DIR/rs_normalized.txt" "$TMP_DIR/ts_normalized.txt" > /dev/null 2>&1; then
    echo -e "  ${GREEN}✅ IDENTICAL${NC}"
    RS_TS_MATCH=true
else
    echo -e "  ${RED}❌ DIFFERENT${NC}"
    RS_TS_MATCH=false
    
    echo ""
    echo "  Errors ONLY in Rust (not in TypeScript):"
    comm -23 <(sort "$TMP_DIR/rs_normalized.txt") <(sort "$TMP_DIR/ts_normalized.txt") | head -20
    
    echo ""
    echo "  Errors ONLY in TypeScript (not in Rust):"
    comm -13 <(sort "$TMP_DIR/rs_normalized.txt") <(sort "$TMP_DIR/ts_normalized.txt") | head -20
fi

echo ""
echo -e "${BLUE}═══════════════════════════════════════════════════════════════${NC}"

# Final summary
if $PY_TS_MATCH && $RS_PY_MATCH && $RS_TS_MATCH; then
    echo -e "${GREEN}✅ All parsers produce IDENTICAL validation errors${NC}"
    exit 0
else
    echo -e "${RED}❌ Parsers produce DIFFERENT validation errors${NC}"
    echo ""
    echo "Summary:"
    $PY_TS_MATCH && echo -e "  ${GREEN}✅ Python = TypeScript${NC}" || echo -e "  ${RED}❌ Python ≠ TypeScript${NC}"
    $RS_PY_MATCH && echo -e "  ${GREEN}✅ Rust = Python${NC}" || echo -e "  ${RED}❌ Rust ≠ Python${NC}"
    $RS_TS_MATCH && echo -e "  ${GREEN}✅ Rust = TypeScript${NC}" || echo -e "  ${RED}❌ Rust ≠ TypeScript${NC}"
    exit 1
fi




