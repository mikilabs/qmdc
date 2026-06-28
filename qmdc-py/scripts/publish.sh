#!/bin/bash
# Publish the qmdc PyPI package: pure-Python parser + bundled native Rust CLI.
#
#   ./scripts/publish.sh             # dry-run (default): build platform wheel + twine check
#   ./scripts/publish.sh --publish   # real publish (needs UV_PUBLISH_TOKEN)
#
# Builds the Rust binary for the CURRENT platform and bundles it into a
# platform-tagged wheel. The full N-platform matrix (see the platform-matrix
# finding) is produced by release CI (zigbuild + cibuildwheel); this script
# does the current platform, which is enough to verify the mechanism.
set -euo pipefail
cd "$(dirname "$0")/.."

DRY_RUN=1
[[ "${1:-}" == "--publish" ]] && DRY_RUN=0

# --- detect current platform -> (bin dir name, PyPI platform tag) ---
OS="$(uname -s)"; ARCH="$(uname -m)"
case "$OS-$ARCH" in
    Darwin-arm64)  PLAT_DIR="macos-arm64";  PLAT_TAG="macosx_11_0_arm64" ;;
    Darwin-x86_64) PLAT_DIR="macos-x64";    PLAT_TAG="macosx_10_12_x86_64" ;;
    Linux-x86_64)  PLAT_DIR="linux-x64";    PLAT_TAG="manylinux_2_17_x86_64" ;;
    Linux-aarch64) PLAT_DIR="linux-arm64";  PLAT_TAG="manylinux_2_28_aarch64" ;;
    *) echo "Unsupported build host $OS-$ARCH; use cargo install qmdc" >&2; exit 1 ;;
esac

# --- build + bundle the native binary ---
echo "=== building Rust binary for $PLAT_DIR ==="
(cd ../qmdc-rs && cargo build --release)
mkdir -p "qmdc/bin/$PLAT_DIR"
cp ../qmdc-rs/target/release/qmdc "qmdc/bin/$PLAT_DIR/qmdc"

# --- build wheel and retag to the platform tag ---
rm -rf dist
uv build --wheel
# The wheel is built as py3-none-any but contains a platform binary — retag it.
uv run --with wheel python -m wheel tags --platform-tag "$PLAT_TAG" --remove dist/*-none-any.whl
uv run --with twine twine check dist/*

if [[ "$DRY_RUN" == "1" ]]; then
    echo "✓ qmdc dry-run OK (platform wheel $PLAT_TAG built + twine check)"
else
    : "${UV_PUBLISH_TOKEN:?Set UV_PUBLISH_TOKEN to publish}"
    uv publish
fi
