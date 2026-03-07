#!/usr/bin/env python3
"""Build SymSpell frequency dictionaries for JonaWhisper.

FR: Lexique383 (140K words with frequencies from books + films corpus)
EN: SymSpell official frequency dictionary (wolfgarbe/SymSpell)

Output: src-tauri/dicts/fr_freq.txt, src-tauri/dicts/en_freq.txt
Format: word<space>frequency (one per line, lowercase)
"""

import csv
import os
import sys
import urllib.request
from pathlib import Path

SCRIPT_DIR = Path(__file__).parent
PROJECT_ROOT = SCRIPT_DIR.parent
DICTS_DIR = PROJECT_ROOT / "src-tauri" / "dicts"

LEXIQUE_URL = "http://www.lexique.org/databases/Lexique383/Lexique383.tsv"
LEXIQUE_CACHE = Path("/tmp/Lexique383.tsv")

# SymSpell official English frequency dict (82K words)
EN_FREQ_URL = "https://raw.githubusercontent.com/wolfgarbe/SymSpell/master/SymSpell/frequency_dictionary_en_82_765.txt"
EN_BIGRAM_URL = "https://raw.githubusercontent.com/wolfgarbe/SymSpell/master/SymSpell/frequency_bigramdictionary_en_243_342.txt"


def download(url: str, dest: Path) -> Path:
    if dest.exists():
        print(f"  cached: {dest}")
        return dest
    print(f"  downloading: {url}")
    urllib.request.urlretrieve(url, dest)
    return dest


def build_fr_dict():
    """Build French frequency dictionary from Lexique383."""
    print("Building FR dictionary from Lexique383...")
    src = download(LEXIQUE_URL, LEXIQUE_CACHE)

    words: dict[str, int] = {}

    with open(src, "r", encoding="utf-8") as f:
        reader = csv.DictReader(f, delimiter="\t")
        for row in reader:
            word = row["ortho"].strip().lower()
            if not word or len(word) <= 1:
                continue

            # Use book frequency (more formal/written) — scale to integer
            # Lexique freq is per million words, multiply by 100 for integer range
            try:
                freq_livres = float(row.get("freqlivres", "0") or "0")
                freq_films = float(row.get("freqfilms2", "0") or "0")
            except ValueError:
                continue

            # Combine both corpora with books weighted higher (more relevant for dictation)
            freq = int((freq_livres * 70 + freq_films * 30) * 100)
            freq = max(freq, 1)  # minimum frequency 1

            # Keep highest frequency for duplicate words
            if word not in words or words[word] < freq:
                words[word] = freq

    # Sort by frequency descending
    sorted_words = sorted(words.items(), key=lambda x: -x[1])

    out = DICTS_DIR / "fr_freq.txt"
    with open(out, "w", encoding="utf-8") as f:
        for word, freq in sorted_words:
            # Use tab separator to avoid ambiguity with multi-word entries
            f.write(f"{word}\t{freq}\n")

    print(f"  wrote {len(sorted_words)} words to {out}")
    return len(sorted_words)


def build_en_dict():
    """Download SymSpell official English frequency dictionary."""
    print("Building EN dictionary from SymSpell official...")
    dest = DICTS_DIR / "en_freq.txt"
    download(EN_FREQ_URL, dest)

    # Count lines
    with open(dest, "r") as f:
        count = sum(1 for _ in f)
    print(f"  {count} words in {dest}")
    return count


def build_en_bigrams():
    """Download SymSpell official English bigram dictionary."""
    print("Downloading EN bigram dictionary...")
    dest = DICTS_DIR / "en_bigram.txt"
    download(EN_BIGRAM_URL, dest)

    with open(dest, "r") as f:
        count = sum(1 for _ in f)
    print(f"  {count} bigrams in {dest}")
    return count


def main():
    DICTS_DIR.mkdir(parents=True, exist_ok=True)

    fr_count = build_fr_dict()
    en_count = build_en_dict()
    en_bi = build_en_bigrams()

    print(f"\nDone! FR: {fr_count} words, EN: {en_count} words, EN bigrams: {en_bi}")
    print(f"Files in: {DICTS_DIR}/")

    # Show sizes
    for f in sorted(DICTS_DIR.glob("*_freq.txt")) :
        size = f.stat().st_size
        print(f"  {f.name}: {size / 1024:.0f} KB")
    for f in sorted(DICTS_DIR.glob("*_bigram.txt")):
        size = f.stat().st_size
        print(f"  {f.name}: {size / 1024:.0f} KB")


if __name__ == "__main__":
    main()
