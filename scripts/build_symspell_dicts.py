#!/usr/bin/env python3
"""Build SymSpell frequency dictionaries for JonaWhisper.

FR: Lexique383 (125K words with frequencies) + DELA (641K inflected forms) + Google Books bigrams
EN: SymSpell official frequency dictionary (wolfgarbe/SymSpell) + bigrams

Output in src-tauri/dicts/:
  fr_freq.txt     — 645K+ French words (tab-separated: word<TAB>freq)
  fr_bigram.txt   — French bigrams (space-separated: word1 word2 freq)
  en_freq.txt     — 82K English words (space-separated: word freq)
  en_bigram.txt   — 242K English bigrams (space-separated: word1 word2 freq)

Usage:
  python scripts/build_symspell_dicts.py          # build all (uses cache)
  python scripts/build_symspell_dicts.py --fresh   # force re-download everything
"""

import csv
import subprocess
import sys
import urllib.request
from pathlib import Path

SCRIPT_DIR = Path(__file__).parent
PROJECT_ROOT = SCRIPT_DIR.parent
DICTS_DIR = PROJECT_ROOT / "src-tauri" / "dicts"

# --- Sources ---

LEXIQUE_URL = "http://www.lexique.org/databases/Lexique383/Lexique383.tsv"
LEXIQUE_CACHE = Path("/tmp/Lexique383.tsv")

# DELA French dictionary — 641K inflected forms from LADL
DELA_DICT = Path("/tmp/dela/share/dict/dict-fr-DELA-common-words.unicode")

# French bigrams from Google Books Ngram Corpus v3 (top 5K, 2010-2019 books)
FR_BIGRAM_URL = "https://raw.githubusercontent.com/orgtre/google-books-ngram-frequency/main/ngrams/2grams_french.csv"
FR_BIGRAM_CACHE = Path("/tmp/fr_bigrams_google.csv")

# SymSpell official English frequency dict (82K words) + bigrams (242K)
EN_FREQ_URL = "https://raw.githubusercontent.com/wolfgarbe/SymSpell/master/SymSpell/frequency_dictionary_en_82_765.txt"
EN_BIGRAM_URL = "https://raw.githubusercontent.com/wolfgarbe/SymSpell/master/SymSpell/frequency_bigramdictionary_en_243_342.txt"


def download(url: str, dest: Path, fresh: bool = False) -> Path:
    if dest.exists() and not fresh:
        print(f"  cached: {dest}")
        return dest
    print(f"  downloading: {url}")
    urllib.request.urlretrieve(url, dest)
    return dest


def ensure_dela():
    """Ensure DELA dictionary is available (pip install if needed)."""
    if DELA_DICT.exists():
        print(f"  DELA cached: {DELA_DICT}")
        return
    print("  Installing dict-fr-DELA from PyPI...")
    subprocess.check_call(
        [sys.executable, "-m", "pip", "install", "--target", "/tmp/dela", "dict-fr-DELA"],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    if not DELA_DICT.exists():
        print("  WARNING: DELA install failed, skipping DELA enrichment")


def build_fr_dict(fresh: bool = False):
    """Build French frequency dictionary from Lexique383 + DELA."""
    print("Building FR dictionary from Lexique383 + DELA...")
    src = download(LEXIQUE_URL, LEXIQUE_CACHE, fresh)

    words: dict[str, int] = {}

    # Step 1: Load Lexique383 (with frequencies)
    with open(src, "r", encoding="utf-8") as f:
        reader = csv.DictReader(f, delimiter="\t")
        for row in reader:
            word = row["ortho"].strip().lower()
            if not word or len(word) <= 1:
                continue

            try:
                freq_livres = float(row.get("freqlivres", "0") or "0")
                freq_films = float(row.get("freqfilms2", "0") or "0")
            except ValueError:
                continue

            # Combine both corpora with books weighted higher (more relevant for dictation)
            freq = int((freq_livres * 70 + freq_films * 30) * 100)
            freq = max(freq, 1)

            if word not in words or words[word] < freq:
                words[word] = freq

    lexique_count = len(words)
    print(f"  Lexique383: {lexique_count} words")

    # Step 2: Enrich with DELA (641K inflected forms, no frequency data)
    ensure_dela()
    dela_added = 0
    if DELA_DICT.exists():
        with open(DELA_DICT, "r", encoding="utf-8") as f:
            for line in f:
                word = line.strip().lower()
                if not word or len(word) <= 1:
                    continue
                if " " in word:
                    continue
                if word not in words:
                    words[word] = 1
                    dela_added += 1
        print(f"  DELA: +{dela_added} new words (total: {len(words)})")
    else:
        print("  DELA: skipped (not available)")

    # Sort by frequency descending
    sorted_words = sorted(words.items(), key=lambda x: -x[1])

    out = DICTS_DIR / "fr_freq.txt"
    with open(out, "w", encoding="utf-8") as f:
        for word, freq in sorted_words:
            f.write(f"{word}\t{freq}\n")

    print(f"  wrote {len(sorted_words)} words to {out}")
    return len(sorted_words)


def build_fr_bigrams(fresh: bool = False):
    """Build French bigram dictionary from Google Books Ngram Corpus."""
    print("Building FR bigrams from Google Books Ngram...")
    src = download(FR_BIGRAM_URL, FR_BIGRAM_CACHE, fresh)

    out = DICTS_DIR / "fr_bigram.txt"
    count = 0
    with open(src, "r", encoding="utf-8") as f, open(out, "w", encoding="utf-8") as fout:
        reader = csv.DictReader(f)
        for row in reader:
            ngram = row["ngram"].strip()
            freq = int(row["freq"])
            parts = ngram.split()
            # Only keep clean 2-word bigrams (skip tokenization artifacts like "d' un")
            if len(parts) == 2:
                fout.write(f"{parts[0]} {parts[1]} {freq}\n")
                count += 1

    print(f"  wrote {count} bigrams to {out}")
    return count


def build_en_dict(fresh: bool = False):
    """Download SymSpell official English frequency dictionary."""
    print("Building EN dictionary from SymSpell official...")
    dest = DICTS_DIR / "en_freq.txt"
    download(EN_FREQ_URL, dest, fresh)

    with open(dest, "r") as f:
        count = sum(1 for _ in f)
    print(f"  {count} words in {dest}")
    return count


def build_en_bigrams(fresh: bool = False):
    """Download SymSpell official English bigram dictionary."""
    print("Downloading EN bigram dictionary...")
    dest = DICTS_DIR / "en_bigram.txt"
    download(EN_BIGRAM_URL, dest, fresh)

    with open(dest, "r") as f:
        count = sum(1 for _ in f)
    print(f"  {count} bigrams in {dest}")
    return count


def main():
    fresh = "--fresh" in sys.argv

    DICTS_DIR.mkdir(parents=True, exist_ok=True)

    fr_count = build_fr_dict(fresh)
    fr_bi = build_fr_bigrams(fresh)
    en_count = build_en_dict(fresh)
    en_bi = build_en_bigrams(fresh)

    print(f"\nDone!")
    print(f"  FR: {fr_count} words, {fr_bi} bigrams")
    print(f"  EN: {en_count} words, {en_bi} bigrams")
    print(f"\nFiles in: {DICTS_DIR}/")

    for f in sorted(DICTS_DIR.glob("*.txt")):
        size = f.stat().st_size
        print(f"  {f.name}: {size / 1024:.0f} KB")


if __name__ == "__main__":
    main()
