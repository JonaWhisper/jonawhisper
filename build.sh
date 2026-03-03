#!/bin/bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
APP_NAME="JonaWhisper"
BUNDLE_ID="com.local.jona-whisper"
DIST_DIR="$SCRIPT_DIR/build"

# Debug or release mode
BUILD_MODE="${1:-release}"
if [ "$BUILD_MODE" = "debug" ]; then
    TARGET_DIR="$SCRIPT_DIR/src-tauri/target/debug"
    TAURI_FLAGS="--debug"
    MODE_LABEL="debug"
else
    TARGET_DIR="$SCRIPT_DIR/src-tauri/target/release"
    TAURI_FLAGS=""
    MODE_LABEL="release"
fi
BUNDLE_DIR="$TARGET_DIR/bundle"
APP_PATH="$BUNDLE_DIR/macos/${APP_NAME}.app"

# ── Signing identity ──────────────────────────────────────
# Auto-detect signing identity for Tauri (if not already set)
if [ -z "${APPLE_SIGNING_IDENTITY:-}" ]; then
    DETECTED=$(security find-identity -v -p codesigning 2>/dev/null \
        | grep -v "^$" | head -1 | sed 's/.*"\(.*\)"/\1/' || true)
    if [ -n "$DETECTED" ] && [[ "$DETECTED" != *"0 valid identities"* ]]; then
        export APPLE_SIGNING_IDENTITY="$DETECTED"
    fi
fi

# ── Build ──────────────────────────────────────────────────
echo ""
echo "=== Building ${APP_NAME} (Tauri ${MODE_LABEL}) ==="

# Ensure deployment target matches Tauri config (needed by whisper-rs-sys cmake)
export MACOSX_DEPLOYMENT_TARGET="14.0"
# Force ARM arch for ggml (Xcode 16+ Clang is strict on i8mm inlining)
export GGML_CPU_ARM_ARCH="armv8.2-a+dotprod"

if [ -n "${APPLE_SIGNING_IDENTITY:-}" ]; then
    echo "  Signing identity: $APPLE_SIGNING_IDENTITY"
else
    echo "  No signing certificate found (ad-hoc signing)"
fi

if [ -n "${APPLE_ID:-}" ] && [ -n "${APPLE_PASSWORD:-}" ] && [ -n "${APPLE_TEAM_ID:-}" ]; then
    echo "  Notarization credentials found"
else
    echo "  Notarization skipped (set APPLE_ID, APPLE_PASSWORD, APPLE_TEAM_ID to enable)"
fi

cd "$SCRIPT_DIR"
npx tauri build --bundles app $TAURI_FLAGS

if [ ! -d "$APP_PATH" ]; then
    echo "ERROR: App bundle not found at $APP_PATH"
    exit 1
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
