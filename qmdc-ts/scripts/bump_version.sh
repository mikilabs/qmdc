#!/bin/bash
# Bump version in package.json
#
# Usage:
#   ./scripts/bump_version.sh patch   # 0.1.0 -> 0.1.1
#   ./scripts/bump_version.sh minor   # 0.1.0 -> 0.2.0
#   ./scripts/bump_version.sh major   # 0.1.0 -> 1.0.0

set -e

PART="${1:-patch}"

if [[ ! "$PART" =~ ^(major|minor|patch)$ ]]; then
    echo "Usage: $0 [major|minor|patch]"
    exit 1
fi

cd "$(dirname "$0")/.."

# Get current version from package.json
CURRENT_VERSION=$(grep '"version":' package.json | head -1 | sed 's/.*"\(.*\)".*/\1/')

echo "Current version: $CURRENT_VERSION"

# Parse version
IFS='.' read -r -a VERSION_PARTS <<< "$CURRENT_VERSION"
MAJOR="${VERSION_PARTS[0]}"
MINOR="${VERSION_PARTS[1]}"
PATCH="${VERSION_PARTS[2]}"

# Bump version
case "$PART" in
    major)
        MAJOR=$((MAJOR + 1))
        MINOR=0
        PATCH=0
        ;;
    minor)
        MINOR=$((MINOR + 1))
        PATCH=0
        ;;
    patch)
        PATCH=$((PATCH + 1))
        ;;
esac

NEW_VERSION="$MAJOR.$MINOR.$PATCH"

echo "New version: $NEW_VERSION"
echo ""

# Update package.json
echo "Updating package.json..."
sed -i.bak "s/\"version\": \"$CURRENT_VERSION\"/\"version\": \"$NEW_VERSION\"/" package.json
rm package.json.bak

echo ""
echo "✓ Version bumped to $NEW_VERSION"
echo ""
echo "Next steps:"
echo "  1. Review changes: git diff"
echo "  2. Commit: git add -u && git commit -m 'Bump TypeScript parser version to $NEW_VERSION'"
echo "  3. Tag: git tag ts-v$NEW_VERSION"

