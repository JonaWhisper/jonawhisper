#!/usr/bin/env bash
set -euo pipefail

# Usage: bump-version.sh [patch|minor|major]
# Updates version in package.json, tauri.conf.json, and Cargo.toml

BUMP_TYPE="${1:?Usage: bump-version.sh [patch|minor|major]}"

# Read current version from package.json
CURRENT=$(grep -o '"version": "[^"]*"' package.json | head -1 | cut -d'"' -f4)
if [[ -z "$CURRENT" ]]; then
  echo "ERROR: Could not read version from package.json" >&2
  exit 1
fi

IFS='.' read -r MAJOR MINOR PATCH <<< "$CURRENT"

case "$BUMP_TYPE" in
  patch) PATCH=$((PATCH + 1)) ;;
  minor) MINOR=$((MINOR + 1)); PATCH=0 ;;
  major) MAJOR=$((MAJOR + 1)); MINOR=0; PATCH=0 ;;
  *) echo "ERROR: Invalid bump type '$BUMP_TYPE'. Use patch, minor, or major." >&2; exit 1 ;;
esac

NEW_VERSION="${MAJOR}.${MINOR}.${PATCH}"
echo "Bumping version: $CURRENT -> $NEW_VERSION"

# Update package.json
sed -i.bak "s/\"version\": \"$CURRENT\"/\"version\": \"$NEW_VERSION\"/" package.json && rm -f package.json.bak

# Update tauri.conf.json
sed -i.bak "s/\"version\": \"$CURRENT\"/\"version\": \"$NEW_VERSION\"/" src-tauri/tauri.conf.json && rm -f src-tauri/tauri.conf.json.bak

# Update Cargo.toml (only the package version line under [package])
sed -i.bak "s/^version = \"$CURRENT\"/version = \"$NEW_VERSION\"/" src-tauri/Cargo.toml && rm -f src-tauri/Cargo.toml.bak

# Verify all 3 files were updated
for f in package.json src-tauri/tauri.conf.json; do
  if ! grep -q "\"version\": \"$NEW_VERSION\"" "$f"; then
    echo "ERROR: Failed to update $f" >&2
    exit 1
  fi
done
if ! grep -q "^version = \"$NEW_VERSION\"" src-tauri/Cargo.toml; then
  echo "ERROR: Failed to update src-tauri/Cargo.toml" >&2
  exit 1
fi

echo "version=$NEW_VERSION"
echo "tag=v$NEW_VERSION"
