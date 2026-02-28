#!/usr/bin/env python3
"""Audit i18n keys: orphaned, missing, desync, and duplicates."""

import json
import os
import re
import sys
from collections import Counter
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
EN_PATH = ROOT / "src" / "i18n" / "en.json"
FR_PATH = ROOT / "src" / "i18n" / "fr.json"

# Source directories to scan
SRC_DIRS = [ROOT / "src", ROOT / "src-tauri" / "src"]

# File extensions to scan
EXTENSIONS = {".vue", ".ts", ".tsx", ".js", ".rs"}

# Patterns to extract i18n key usage from source files:
#   t('key')           — vue-i18n direct
#   t('key', [...])    — vue-i18n with args
#   t("key")           — vue-i18n double-quoted
#   t!("key")          — rust-i18n
#   t!("key", ...)     — rust-i18n with args
DIRECT_PATTERNS = [
    re.compile(r"""t\(\s*['"]([a-zA-Z]\w+(?:\.\w+)+)['"]\s*[,)]"""),   # t('key.sub') / t('key.sub', args)
    re.compile(r"""t!\(\s*"([a-zA-Z]\w+(?:\.\w+)+)"\s*[,)]"""),         # t!("key.sub") / t!("key.sub", args)
]

# Pattern to find ALL quoted strings that look like i18n keys (word.word.word).
# This catches keys used indirectly: data arrays, object literals, etc.
#   e.g. { label: 'settings.section.general' }
INDIRECT_PATTERN = re.compile(r"""['"]([a-zA-Z]\w+(?:\.\w+)+)['"]""")


def load_json_keys(path: Path) -> list[str]:
    with open(path) as f:
        data = json.load(f)
    return sorted(k for k in data if k != "_version")


def find_duplicate_keys(path: Path) -> list[str]:
    """Find duplicate JSON keys (json module silently deduplicates)."""
    with open(path) as f:
        text = f.read()
    keys = re.findall(r'^\s*"([^"]+)"\s*:', text, re.MULTILINE)
    counts = Counter(keys)
    return sorted(k for k, n in counts.items() if n > 1)


def scan_source_files() -> tuple[set[str], set[str]]:
    """Scan all source files for i18n key references.

    Returns:
        direct_keys:   keys found in t()/t!() calls
        indirect_keys: keys found as quoted strings matching i18n key pattern
    """
    direct_keys: set[str] = set()
    indirect_keys: set[str] = set()

    for src_dir in SRC_DIRS:
        for root, _dirs, files in os.walk(src_dir):
            # Skip i18n directory
            if "i18n" in root:
                continue
            for fname in files:
                ext = os.path.splitext(fname)[1]
                if ext not in EXTENSIONS:
                    continue
                filepath = os.path.join(root, fname)
                try:
                    with open(filepath) as f:
                        content = f.read()
                except (OSError, UnicodeDecodeError):
                    continue

                # Direct t()/t!() calls
                for pattern in DIRECT_PATTERNS:
                    for match in pattern.finditer(content):
                        direct_keys.add(match.group(1))

                # Indirect: any quoted string that looks like an i18n key
                for match in INDIRECT_PATTERN.finditer(content):
                    indirect_keys.add(match.group(1))

    return direct_keys, indirect_keys


def main() -> int:
    errors = 0

    # Load keys
    en_keys = load_json_keys(EN_PATH)
    fr_keys = load_json_keys(FR_PATH)
    en_set = set(en_keys)
    fr_set = set(fr_keys)

    # 1. Check EN/FR sync
    only_en = sorted(en_set - fr_set)
    only_fr = sorted(fr_set - en_set)
    if only_en:
        print("DESYNC — keys in EN but not FR:")
        for k in only_en:
            print(f"  {k}")
        errors = 1
    if only_fr:
        print("DESYNC — keys in FR but not EN:")
        for k in only_fr:
            print(f"  {k}")
        errors = 1

    # 2. Check for duplicate keys
    for path, label in [(EN_PATH, "en.json"), (FR_PATH, "fr.json")]:
        dupes = find_duplicate_keys(path)
        if dupes:
            print(f"DUPLICATES in {label}:")
            for k in dupes:
                print(f"  {k}")
            errors = 1

    # 3. Scan source files
    direct_keys, indirect_keys = scan_source_files()
    all_code_keys = direct_keys | indirect_keys

    # 4. Orphaned keys (in JSON but not found anywhere in code)
    orphaned = sorted(en_set - all_code_keys)
    if orphaned:
        print(f"\nORPHANED ({len(orphaned)} keys in JSON, not found in code):")
        for k in orphaned:
            print(f"  {k}")
        errors = 1

    # 5. Missing keys (in t()/t!() calls but not in JSON)
    # Only flag keys from direct t() calls, not indirect pattern matches
    # (indirect matches are too noisy — they include non-i18n dotted strings)
    missing = sorted(direct_keys - en_set)
    if missing:
        print(f"\nMISSING ({len(missing)} keys used in t()/t!(), not in JSON):")
        for k in missing:
            print(f"  {k}")
        errors = 1

    # Summary
    print(f"\ni18n audit: {len(en_keys)} EN, {len(fr_keys)} FR, "
          f"{len(direct_keys)} direct, {len(indirect_keys)} indirect refs")
    if errors == 0:
        print("All keys OK.")

    return errors


if __name__ == "__main__":
    sys.exit(main())
