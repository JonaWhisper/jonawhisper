#!/bin/bash
# Test pill state machine by invoking the simulate_pill_test command.
# Usage: ./scripts/test-pill.sh [rounds]
#
# This opens the pill window and cycles through:
#   recording (with fake spectrum) → transcribing (dots) → complete → repeat
# Then shows error state briefly before closing.
#
# The app must be running. This script uses the dev console to invoke the command.

ROUNDS=${1:-3}

echo "Starting pill simulation with $ROUNDS rounds..."
echo "Make sure WhisperDictate is running."
echo ""

# Use osascript to send the command via the app's JavaScript context
# Unfortunately there's no direct CLI for Tauri commands, so we'll
# use a different approach: add a menu item that triggers the test.

echo "To run the simulation:"
echo "  1. Open the app"
echo "  2. In any webview, open DevTools (Cmd+Option+I)"
echo "  3. Run in console:"
echo ""
echo "     window.__TAURI__.core.invoke('simulate_pill_test', { count: $ROUNDS })"
echo ""
echo "Or trigger it from the tray menu (if added)."
