#!/bin/bash
# Publish qmdc-semantic to PyPI (pure-Python, no bundled binary).
#
#   ./scripts/publish.sh             # dry-run (default): build + twine check
#   ./scripts/publish.sh --publish   # real publish (needs UV_PUBLISH_TOKEN)
set -euo pipefail
cd "$(dirname "$0")/.."

DRY_RUN=1
[[ "${1:-}" == "--publish" ]] && DRY_RUN=0

rm -rf dist
uv build
uv run --with twine twine check dist/*

if [[ "$DRY_RUN" == "1" ]]; then
    echo "✓ qmdc-semantic dry-run OK (built + twine check)"
else
    : "${UV_PUBLISH_TOKEN:?Set UV_PUBLISH_TOKEN to publish}"
    uv publish
fi
