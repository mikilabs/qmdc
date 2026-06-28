#!/bin/bash
# Build ALL release artifacts across the full platform matrix:
#   - 7 native qmdc binaries (cargo zigbuild cross-compile)
#   - 7 npm per-platform packages (@qmdc/cli-<platform>) + the main @qmdc/qmdc tarball
#   - 7 PyPI platform-tagged wheels for `qmdc` (pure-Python + bundled binary)
#   - pure-Python sdist+wheel for qmdc-semantic and qmdc-mkdocs
#   - 6 VS Code VSIX packages (qmdc-vscode, one per platform; no musl)
#
# Output: ./dist-release/{bin,npm,pypi,vscode}/
#
# Requires: cargo-zigbuild + zig, and the Rust targets (this script adds any missing).
# This is the local equivalent of the release CI matrix; it does NOT publish.
set -euo pipefail
cd "$(dirname "$0")/.."
ROOT="$(pwd)"
OUT="$ROOT/dist-release"

# target triple | bin-dir (py launcher) | npm suffix | npm os | npm cpu | PyPI plat tag
TARGETS=(
  "aarch64-apple-darwin|macos-arm64|darwin-arm64|darwin|arm64|macosx_11_0_arm64"
  "x86_64-apple-darwin|macos-x64|darwin-x64|darwin|x64|macosx_10_12_x86_64"
  "x86_64-unknown-linux-gnu|linux-x64|linux-x64|linux|x64|manylinux_2_17_x86_64"
  "aarch64-unknown-linux-gnu|linux-arm64|linux-arm64|linux|arm64|manylinux_2_28_aarch64"
  "x86_64-unknown-linux-musl|linux-x64|linux-x64-musl|linux|x64|musllinux_1_1_x86_64"
  "x86_64-pc-windows-gnu|windows-x64|win32-x64|win32|x64|win_amd64"
  "aarch64-pc-windows-gnullvm|windows-arm64|win32-arm64|win32|arm64|win_arm64"
)

VERSION="$(grep '^version = ' qmdc-rs/Cargo.toml | head -1 | sed 's/.*"\(.*\)".*/\1/')"
echo "=== release-build: qmdc v$VERSION ==="

rm -rf "$OUT"
mkdir -p "$OUT/bin" "$OUT/npm" "$OUT/pypi"

# --- 1. cross-build all binaries -------------------------------------------
# Apple targets build with the native toolchain (zig has no macOS SDK, so it
# can't link CoreFoundation); linux/windows cross-compile via cargo-zigbuild.
# Staging is keyed by the unique npm suffix (glibc vs musl share a py bindir
# but must NOT share a staging slot, or one overwrites the other).
for entry in "${TARGETS[@]}"; do
    IFS='|' read -r triple bindir suffix _ _ _ <<< "$entry"
    rustup target add "$triple" >/dev/null 2>&1 || true
    echo "--- building $triple ---"
    if [[ "$triple" == *-apple-darwin ]]; then
        (cd qmdc-rs && cargo build --release --target "$triple")
    else
        (cd qmdc-rs && cargo zigbuild --release --target "$triple")
    fi
    ext=""; [[ "$triple" == *windows* ]] && ext=".exe"
    mkdir -p "$OUT/bin/$suffix"
    cp "qmdc-rs/target/$triple/release/qmdc$ext" "$OUT/bin/$suffix/qmdc$ext"
done

# --- 2. npm: per-platform packages + main tarball --------------------------
# Assemble the per-platform packages in a scratch dir; only the versioned
# .tgz tarballs land in the output.
NPM_BUILD="$ROOT/qmdc-ts/build/npm"
rm -rf "$NPM_BUILD"; mkdir -p "$NPM_BUILD"
for entry in "${TARGETS[@]}"; do
    IFS='|' read -r triple bindir suffix npmos npmcpu _ <<< "$entry"
    ext=""; [[ "$triple" == *windows* ]] && ext=".exe"
    pkg="$NPM_BUILD/cli-$suffix"
    mkdir -p "$pkg"
    cp "$OUT/bin/$suffix/qmdc$ext" "$pkg/qmdc$ext"
    cat > "$pkg/package.json" <<EOF
{
  "name": "@qmdc/cli-$suffix",
  "version": "$VERSION",
  "description": "qmdc native CLI binary for $suffix",
  "license": "AGPL-3.0-or-later",
  "repository": { "type": "git", "url": "https://github.com/mikilabs/qmdc" },
  "os": ["$npmos"],
  "cpu": ["$npmcpu"],
  "files": ["qmdc$ext"]
}
EOF
    (cd "$pkg" && npm pack --pack-destination "$OUT/npm" >/dev/null)
done
# Pack the main launcher, pinning its optionalDependencies to THIS build's cli
# version (the @qmdc/cli-* packages are versioned by the crate). Without this the
# published launcher would pin stale/non-existent cli versions and fail to install.
(
    cd qmdc-ts
    cp package.json package.json.bak
    node -e '
      const fs = require("fs");
      const v = process.argv[1];
      const p = JSON.parse(fs.readFileSync("package.json", "utf8"));
      for (const k of Object.keys(p.optionalDependencies || {})) p.optionalDependencies[k] = v;
      fs.writeFileSync("package.json", JSON.stringify(p, null, 2) + "\n");
    ' "$VERSION"
    npm pack --pack-destination "$OUT/npm" >/dev/null
    mv package.json.bak package.json
)
rm -rf "$NPM_BUILD"
echo "✓ npm: $(ls "$OUT/npm"/*.tgz | wc -l | tr -d ' ') tarballs"

# --- 3. PyPI: one platform wheel per target for `qmdc` ----------------------
for entry in "${TARGETS[@]}"; do
    IFS='|' read -r triple bindir suffix _ _ tag <<< "$entry"
    ext=""; [[ "$triple" == *windows* ]] && ext=".exe"
    rm -rf qmdc-py/qmdc/bin qmdc-py/dist qmdc-py/build
    mkdir -p "qmdc-py/qmdc/bin/$bindir"
    cp "$OUT/bin/$suffix/qmdc$ext" "qmdc-py/qmdc/bin/$bindir/qmdc$ext"
    (cd qmdc-py && uv build --wheel >/dev/null \
        && uv run --with wheel python -m wheel tags --platform-tag "$tag" --remove dist/*-none-any.whl >/dev/null)
    cp qmdc-py/dist/*.whl "$OUT/pypi/"
done
rm -rf qmdc-py/qmdc/bin qmdc-py/dist qmdc-py/build
echo "✓ pypi qmdc: $(ls "$OUT/pypi"/qmdc-*.whl | wc -l | tr -d ' ') wheels"

# --- 4. pure-Python packages (sdist + wheel) -------------------------------
for p in qmdc-semantic qmdc-mkdocs; do
    (cd "$p" && rm -rf dist && uv build >/dev/null)
    cp "$p"/dist/* "$OUT/pypi/"
done

# --- 5. VS Code extension: per-platform VSIX (6 targets, no musl) -----------
echo "=== building VS Code VSIX (6 platforms) ==="
(cd qmdc-vscode && npm run package:all)
mkdir -p "$OUT/vscode"
cp qmdc-vscode/*.vsix "$OUT/vscode/"
echo "✓ vscode: $(ls "$OUT/vscode"/*.vsix | wc -l | tr -d ' ') VSIX"

echo ""
echo "✅ release-build complete → $OUT"
ls -1 "$OUT/bin" "$OUT/npm" "$OUT/pypi" "$OUT/vscode"
