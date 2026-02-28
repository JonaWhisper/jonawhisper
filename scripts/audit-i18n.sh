#!/usr/bin/env bash
# Audit i18n keys: find orphaned keys, missing keys, and desync between EN/FR.
# Usage: ./scripts/audit-i18n.sh
set -uo pipefail
cd "$(git rev-parse --show-toplevel)"

EN=src/i18n/en.json
FR=src/i18n/fr.json
SRC_DIRS=(src/ src-tauri/src/)

errors=0

# -- Extract keys from JSON (exclude _version) --
json_keys() {
  python3 << PYEOF
import json, sys
with open("$1") as f:
    d = json.load(f)
for k in sorted(d):
    if k != "_version":
        print(k)
PYEOF
}

en_keys=$(json_keys "$EN")
fr_keys=$(json_keys "$FR")
en_count=$(echo "$en_keys" | wc -l | tr -d ' ')
fr_count=$(echo "$fr_keys" | wc -l | tr -d ' ')

# -- 1. Check EN/FR sync --
only_en=$(comm -23 <(echo "$en_keys") <(echo "$fr_keys"))
only_fr=$(comm -13 <(echo "$en_keys") <(echo "$fr_keys"))

if [ -n "$only_en" ]; then
  echo "DESYNC — keys in EN but not FR:"
  echo "$only_en" | sed 's/^/  /'
  errors=1
fi
if [ -n "$only_fr" ]; then
  echo "DESYNC — keys in FR but not EN:"
  echo "$only_fr" | sed 's/^/  /'
  errors=1
fi

# -- 2. Check for duplicate keys --
for file in "$EN" "$FR"; do
  dupes=$(grep -o '"[^"]*":' "$file" | sort | uniq -d || true)
  if [ -n "$dupes" ]; then
    echo "DUPLICATES in $(basename "$file"):"
    echo "$dupes" | sed 's/^/  /'
    errors=1
  fi
done

# -- 3. Find orphaned keys (in JSON but not referenced in source) --
# For each key, check if it appears as a quoted string in any source file.
orphaned=""
while IFS= read -r key; do
  found=0
  # Escape dots for grep -E
  escaped=$(echo "$key" | sed 's/\./\\./g')
  # Check if the key appears as "key" or 'key' in source files
  if grep -rq --include='*.rs' --include='*.vue' --include='*.ts' --include='*.js' \
       -E "([\"'])${escaped}\\1" "${SRC_DIRS[@]}" 2>/dev/null; then
    found=1
  fi
  if [ $found -eq 0 ]; then
    orphaned+="$key"$'\n'
  fi
done <<< "$en_keys"
orphaned=$(echo "$orphaned" | sed '/^$/d')

# -- 4. Find missing keys (referenced in code but not in JSON) --
# Extract t('key') / t("key") from frontend
fe_keys=$(grep -roh --include='*.vue' --include='*.ts' --include='*.js' \
  "t('[a-zA-Z][^']*')" "${SRC_DIRS[@]}" 2>/dev/null | \
  sed "s/t('//;s/')$//" | sort -u || true)

# Extract t!("key") from Rust
rs_keys=$(grep -roh --include='*.rs' \
  't!("[a-zA-Z][^"]*")' "${SRC_DIRS[@]}" 2>/dev/null | \
  sed 's/t!("//;s/")$//' | sort -u || true)

code_keys=$(printf '%s\n%s\n' "$fe_keys" "$rs_keys" | grep -E '^\w+\.' | sort -u)
missing=$(comm -13 <(echo "$en_keys") <(echo "$code_keys") || true)

# -- Report --
echo ""
echo "i18n audit: $en_count EN keys, $fr_count FR keys"

if [ -n "$orphaned" ]; then
  orphaned_count=$(echo "$orphaned" | wc -l | tr -d ' ')
  echo ""
  echo "ORPHANED ($orphaned_count keys in JSON, not found in code):"
  echo "$orphaned" | sed 's/^/  /'
  errors=1
fi

if [ -n "$missing" ]; then
  missing_count=$(echo "$missing" | wc -l | tr -d ' ')
  echo ""
  echo "MISSING ($missing_count keys used in code, not in JSON):"
  echo "$missing" | sed 's/^/  /'
  errors=1
fi

if [ $errors -eq 0 ]; then
  echo " All keys OK."
fi

exit $errors
