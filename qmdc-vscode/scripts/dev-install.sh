#!/bin/bash

# Quick dev install script for testing QMDC extension changes

set -e  # Exit on error

echo "🔧 QMDC Extension Dev Install"
echo "================================"

# Detect installed extension path
EXTENSION_PATH="$HOME/.cursor/extensions/mikilabs.qmdc-vscode-1.0.0"

if [ ! -d "$EXTENSION_PATH" ]; then
    echo "❌ Extension not found at $EXTENSION_PATH"
    echo "Please install the extension first via Cursor marketplace"
    exit 1
fi

echo "✓ Found extension at $EXTENSION_PATH"
echo ""

# Build LSP if needed
echo "🦀 Building Rust LSP..."
cd ../qmdc-rs
cargo build --quiet
echo "✓ LSP built"
echo ""

# Build extension
echo "📦 Building TypeScript extension..."
cd ../qmdc-vscode
npm run compile
echo "✓ Extension compiled"
echo ""

# Copy files
echo "📋 Copying files to installed extension..."

# Backup first time
if [ ! -d "$EXTENSION_PATH/out.backup" ]; then
    echo "  Creating backup..."
    cp -r "$EXTENSION_PATH/out" "$EXTENSION_PATH/out.backup"
    cp "$EXTENSION_PATH/bin/qmdc" "$EXTENSION_PATH/bin/qmdc.backup" 2>/dev/null || true
fi

# Copy compiled extension
cp -r out/* "$EXTENSION_PATH/out/"
echo "  ✓ Copied extension files"

# Copy LSP binary
cp ../qmdc-rs/target/debug/qmdc "$EXTENSION_PATH/bin/"
echo "  ✓ Copied qmdc binary"

echo ""
echo "✅ Installation complete!"
echo ""
echo "Next steps:"
echo "  1. In Cursor: Cmd+Shift+P → 'Developer: Reload Window'"
echo "  2. Check QMDC Explorer panel"
echo "  3. Check Output → 'QMDC Explorer' for logs"
echo ""
echo "To restore backup:"
echo "  cp -r $EXTENSION_PATH/out.backup $EXTENSION_PATH/out"
echo "  cp $EXTENSION_PATH/bin/qmdc.backup $EXTENSION_PATH/bin/qmdc"



