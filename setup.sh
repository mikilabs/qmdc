#!/bin/bash
# setup.sh - QMDC Parser Setup Script
# Installs dependencies and builds all parsers

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

# Colors
GREEN='\033[32m'
RED='\033[31m'
BLUE='\033[34m'
NC='\033[0m'

success() { echo -e "  ${GREEN}✓${NC} $1"; }
error() { echo -e "  ${RED}✗${NC} $1" >&2; }
info() { echo -e "${BLUE}==>${NC} $1"; }

# Help
if [[ "$1" == "--help" || "$1" == "-h" ]]; then
    echo "Usage: ./setup.sh [--test] [--clean]"
    echo ""
    echo "Options:"
    echo "  --test   Run tests after setup"
    echo "  --clean  Remove all build artifacts and dependencies"
    echo "  --help   Show this help"
    exit 0
fi

# Clean mode
if [[ "$1" == "--clean" ]]; then
    info "Cleaning..."
    
    rm -rf qmdc-ts/node_modules qmdc-ts/dist qmdc-ts/.tsbuildinfo 2>/dev/null && success "TypeScript cleaned" || true
    rm -rf qmdc-rs/target 2>/dev/null && success "Rust cleaned" || true
    rm -rf qmdc-py/build qmdc-py/dist qmdc-py/*.egg-info 2>/dev/null || true
    rm -rf qmdc-py/.pytest_cache qmdc-py/.ruff_cache 2>/dev/null || true
    find . -type d -name "__pycache__" -exec rm -rf {} + 2>/dev/null || true
    find . -type f -name "*.pyc" -delete 2>/dev/null || true
    success "Python cleaned"
    
    echo -e "\n${GREEN}Clean complete!${NC}"
    exit 0
fi

# Setup
info "Installing Python dependencies..."
make py-install
success "Python ready"

info "Installing TypeScript dependencies..."
make ts-install
success "TypeScript ready"

info "Building Rust parser (debug)..."
cd qmdc-rs && cargo build && cd ..
success "Rust ready"

# Test mode
if [[ "$1" == "--test" ]]; then
    info "Running tests..."
    make test
    success "All tests passed"
fi

echo -e "\n${GREEN}Setup complete!${NC}"
echo "Run ./bin/qmdc-py, ./bin/qmdc-ts, or ./bin/qmdc-rs to use parsers"
