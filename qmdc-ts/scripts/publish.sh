#!/bin/bash
# Publish the qmdc npm package + its per-platform binary package.
#
#   ./scripts/publish.sh             # dry-run (default): assemble + npm pack --dry-run
#   ./scripts/publish.sh --publish   # real publish (needs NODE_AUTH_TOKEN)
#
# The main `qmdc` package is a thin launcher (bin/qmdc.cjs) that resolves the
# matching @qmdc/cli-<platform> optionalDependency and execs its native binary.
# This script builds + packs the CURRENT platform's package; release CI builds
# the full matrix (see the platform-matrix finding).
set -euo pipefail
cd "$(dirname "$0")/.."

DRY_RUN=1
[[ "${1:-}" == "--publish" ]] && DRY_RUN=0

VERSION="$(node -p "require('./package.json').version")"

# --- detect current platform -> (npm pkg suffix, os, cpu) ---
OS="$(uname -s)"; ARCH="$(uname -m)"
case "$OS-$ARCH" in
    Darwin-arm64)  SUFFIX="darwin-arm64"; NPM_OS="darwin"; NPM_CPU="arm64" ;;
    Darwin-x86_64) SUFFIX="darwin-x64";   NPM_OS="darwin"; NPM_CPU="x64" ;;
    Linux-x86_64)  SUFFIX="linux-x64";    NPM_OS="linux";  NPM_CPU="x64" ;;
    Linux-aarch64) SUFFIX="linux-arm64";  NPM_OS="linux";  NPM_CPU="arm64" ;;
    *) echo "Unsupported build host $OS-$ARCH; use cargo install qmdc" >&2; exit 1 ;;
esac

# --- build the native binary and assemble the platform package ---
echo "=== building Rust binary for $SUFFIX ==="
(cd ../qmdc-rs && cargo build --release)

PKG_DIR="build/npm/cli-$SUFFIX"
rm -rf "$PKG_DIR" && mkdir -p "$PKG_DIR"
cp ../qmdc-rs/target/release/qmdc "$PKG_DIR/qmdc"
cat > "$PKG_DIR/package.json" <<EOF
{
  "name": "@qmdc/cli-$SUFFIX",
  "version": "$VERSION",
  "description": "qmdc native CLI binary for $SUFFIX",
  "license": "AGPL-3.0-or-later",
  "repository": { "type": "git", "url": "https://github.com/mikilabs/qmdc" },
  "os": ["$NPM_OS"],
  "cpu": ["$NPM_CPU"],
  "files": ["qmdc"]
}
EOF

echo "=== npm pack --dry-run (platform package) ==="
npm pack --dry-run --pack-destination /tmp "$PKG_DIR"
echo "=== npm pack --dry-run (main package) ==="
npm pack --dry-run --pack-destination /tmp

if [[ "$DRY_RUN" == "1" ]]; then
    echo "✓ qmdc npm dry-run OK (main + @qmdc/cli-$SUFFIX packed)"
else
    : "${NODE_AUTH_TOKEN:?Set NODE_AUTH_TOKEN to publish}"
    npm publish --access public "$PKG_DIR"
    npm publish --access public
fi
