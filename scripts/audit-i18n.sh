#!/usr/bin/env bash
# Audit i18n keys: find orphaned keys, missing keys, and desync between EN/FR.
# Usage: ./scripts/audit-i18n.sh
cd "$(git rev-parse --show-toplevel)"
exec python3 scripts/audit-i18n.py
