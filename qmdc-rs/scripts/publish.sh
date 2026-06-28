#!/bin/bash
# Publish the qmdc crate to crates.io.
#
#   ./scripts/publish.sh             # dry-run (default, safe)
#   ./scripts/publish.sh --publish   # real publish (needs CARGO_REGISTRY_TOKEN)
set -euo pipefail
cd "$(dirname "$0")/.."

DRY_RUN=1
[[ "${1:-}" == "--publish" ]] && DRY_RUN=0

if [[ "$DRY_RUN" == "1" ]]; then
    echo "=== qmdc (crates.io): dry-run ==="
    cargo publish --dry-run --allow-dirty
    echo "✓ crate dry-run OK"
else
    : "${CARGO_REGISTRY_TOKEN:?Set CARGO_REGISTRY_TOKEN to publish}"
    if [[ -n "$(git status --porcelain . 2>/dev/null)" ]]; then
        echo "Refusing to publish: working tree is dirty." >&2
        exit 1
    fi
    echo "=== qmdc (crates.io): publish ==="
    cargo publish
fi
