#!/bin/bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
APP_NAME="WhisperDictate"
APP_DIR="$HOME/Applications/${APP_NAME}.app"
BUILD_DIR="$SCRIPT_DIR/.build/release"

echo "=== Building ${APP_NAME} ==="

cd "$SCRIPT_DIR"

# Build with Swift Package Manager
swift build -c release 2>&1

BINARY="$BUILD_DIR/$APP_NAME"

if [ ! -f "$BINARY" ]; then
    echo "ERROR: Binary not found at $BINARY"
    exit 1
fi

echo "=== Creating app bundle ==="

# Create .app bundle structure
mkdir -p "$APP_DIR/Contents/MacOS"
mkdir -p "$APP_DIR/Contents/Resources"

# Copy binary
cp "$BINARY" "$APP_DIR/Contents/MacOS/$APP_NAME"

# Copy Info.plist
cp "$SCRIPT_DIR/Info.plist" "$APP_DIR/Contents/"

# Copy app icon
if [ -f "$SCRIPT_DIR/Resources/AppIcon.icns" ]; then
    cp "$SCRIPT_DIR/Resources/AppIcon.icns" "$APP_DIR/Contents/Resources/"
fi

echo "=== Code signing ==="

IDENTITY=$(security find-identity -v -p codesigning | head -1 | sed 's/.*"\(.*\)"/\1/')
if [ -n "$IDENTITY" ] && [ "$IDENTITY" != "0 valid identities found" ]; then
    echo "Signing with: $IDENTITY"
    codesign --force --deep --sign "$IDENTITY" "$APP_DIR"
else
    echo "No developer certificate found, using ad-hoc signing"
    codesign --force --deep --sign - "$APP_DIR"
fi

echo "=== Done ==="
echo "App installed at: $APP_DIR"
echo ""
echo "To launch: open $APP_DIR"
echo ""
echo "First launch: grant permissions for:"
echo "  - Microphone"
echo "  - Input Monitoring"
echo "  - Accessibility"
echo "  - Notifications"
