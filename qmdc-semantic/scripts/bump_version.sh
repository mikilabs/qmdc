#!/bin/bash
# Bump version in pyproject.toml and qmdc_semantic/__init__.py
#
# Usage:
#   ./scripts/bump_version.sh patch   # 1.0.0 -> 1.0.1
#   ./scripts/bump_version.sh minor   # 1.0.0 -> 1.1.0
#   ./scripts/bump_version.sh major   # 1.0.0 -> 2.0.0

set -e

PART="${1:-patch}"

if [[ ! "$PART" =~ ^(major|minor|patch)$ ]]; then
    echo "Usage: $0 [major|minor|patch]"
    exit 1
fi

cd "$(dirname "$0")/.."

# Authoritative version comes from pyproject.toml
CURRENT_VERSION=$(grep '^version = ' pyproject.toml | head -1 | sed 's/.*"\(.*\)".*/\1/')
echo "Current version: $CURRENT_VERSION"

IFS='.' read -r -a VERSION_PARTS <<< "$CURRENT_VERSION"
MAJOR="${VERSION_PARTS[0]}"
MINOR="${VERSION_PARTS[1]}"
PATCH="${VERSION_PARTS[2]}"

case "$PART" in
    major) MAJOR=$((MAJOR + 1)); MINOR=0; PATCH=0 ;;
    minor) MINOR=$((MINOR + 1)); PATCH=0 ;;
    patch) PATCH=$((PATCH + 1)) ;;
esac

NEW_VERSION="$MAJOR.$MINOR.$PATCH"
echo "New version: $NEW_VERSION"

# pyproject.toml — replace the exact current version
sed -i.bak "s/^version = \"$CURRENT_VERSION\"/version = \"$NEW_VERSION\"/" pyproject.toml
rm pyproject.toml.bak

# qmdc_semantic/__init__.py — set __version__ to NEW (match any value, so a drifted
# __version__ is corrected rather than left behind)
if [[ -f qmdc_semantic/__init__.py ]]; then
    sed -i.bak "s/^__version__ = \".*\"/__version__ = \"$NEW_VERSION\"/" qmdc_semantic/__init__.py
    rm qmdc_semantic/__init__.py.bak
fi

echo "✓ qmdc-semantic bumped to $NEW_VERSION (pyproject.toml + __init__.py)"
