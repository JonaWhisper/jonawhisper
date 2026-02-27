#!/bin/bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
APP_NAME="WhisperDictate"
BUNDLE_ID="com.local.whisper-dictate"
DIST_DIR="$SCRIPT_DIR/build"
RELEASE_DIR="$SCRIPT_DIR/src-tauri/target/release"
BUNDLE_DIR="$RELEASE_DIR/bundle"
APP_PATH="$BUNDLE_DIR/macos/${APP_NAME}.app"

# ── Build ──────────────────────────────────────────────────
echo ""
echo "=== Building ${APP_NAME} (Tauri release) ==="

cd "$SCRIPT_DIR"
npx tauri build --bundles app 2>&1

if [ ! -d "$APP_PATH" ]; then
    echo "ERROR: App bundle not found at $APP_PATH"
    exit 1
fi

# ── Code Signing ───────────────────────────────────────────
echo ""
echo "=== Code signing ==="

IDENTITY=$(security find-identity -v -p codesigning 2>/dev/null | grep -v "^$" | head -1 | sed 's/.*"\(.*\)"/\1/' || true)

if [ -n "$IDENTITY" ] && [[ "$IDENTITY" != *"0 valid identities"* ]]; then
    echo "  Signing with: $IDENTITY"
    codesign --force --deep --sign "$IDENTITY" \
        --entitlements "$SCRIPT_DIR/src-tauri/entitlements.plist" \
        "$APP_PATH" 2>/dev/null || \
    codesign --force --deep --sign "$IDENTITY" "$APP_PATH"
    echo "  Signed with developer certificate (stable identity)"
else
    echo "  No developer certificate found, using ad-hoc signing"
    codesign --force --deep --sign - "$APP_PATH"
    echo "  ⚠ Ad-hoc signed: permissions must be re-granted after each rebuild"
fi

# ── Distribution ───────────────────────────────────────────
echo ""
echo "=== Packaging ==="

mkdir -p "$DIST_DIR"

# Copy .app
rm -rf "$DIST_DIR/${APP_NAME}.app"
cp -R "$APP_PATH" "$DIST_DIR/"
echo "  .app → $DIST_DIR/${APP_NAME}.app"

# Copy DMG if it exists
DMG_FILE=$(find "$BUNDLE_DIR/dmg/" -name "*.dmg" -type f 2>/dev/null | head -1)
if [ -n "$DMG_FILE" ]; then
    cp "$DMG_FILE" "$DIST_DIR/${APP_NAME}.dmg"
    echo "  .dmg → $DIST_DIR/${APP_NAME}.dmg"
fi

# ── Summary ────────────────────────────────────────────────
echo ""
echo "=== Done ==="
echo ""
echo "  App:  $DIST_DIR/${APP_NAME}.app"
[ -n "${DMG_FILE:-}" ] && echo "  DMG:  $DIST_DIR/${APP_NAME}.dmg"
echo ""
echo "  To launch:  open \"$DIST_DIR/${APP_NAME}.app\""
echo "  To install: cp -R \"$DIST_DIR/${APP_NAME}.app\" ~/Applications/"
echo ""
echo "  First launch: grant permissions for:"
echo "    - Microphone (audio recording)"
echo "    - Accessibility (paste simulation via Cmd+V)"
echo "    - Input Monitoring (global hotkey detection)"
echo ""
