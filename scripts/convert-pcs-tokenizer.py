#!/usr/bin/env python3
"""
Convert the SentencePiece .model file from the PCS (punct_cap_seg) model
into a tokenizer.json compatible with the Rust `tokenizers` crate.

Usage:
    pip install sentencepiece tokenizers
    python scripts/convert-pcs-tokenizer.py

Downloads the .model from HuggingFace, converts it, and saves tokenizer.json
in the current directory. The resulting file should be hosted (HF/GitHub)
for auto-download by the Rust code.
"""

import json
import tempfile
from pathlib import Path

try:
    import sentencepiece as spm
except ImportError:
    raise SystemExit("pip install sentencepiece")

try:
    from tokenizers import Tokenizer, models, normalizers, pre_tokenizers
except ImportError:
    raise SystemExit("pip install tokenizers")


MODEL_URL = "https://huggingface.co/1-800-BAD-CODE/punct_cap_seg_47_language/resolve/main/spe_unigram_64k_lowercase_47lang.model"
OUTPUT_FILE = "tokenizer.json"


def download_model(url: str) -> Path:
    import urllib.request
    tmp = Path(tempfile.mkdtemp()) / "spe.model"
    print(f"Downloading {url} ...")
    urllib.request.urlretrieve(url, tmp)
    print(f"Downloaded to {tmp} ({tmp.stat().st_size} bytes)")
    return tmp


def convert(model_path: Path) -> Tokenizer:
    sp = spm.SentencePieceProcessor()
    sp.Load(str(model_path))

    vocab_size = sp.GetPieceSize()
    print(f"Vocab size: {vocab_size}")
    print(f"BOS id: {sp.bos_id()}, EOS id: {sp.eos_id()}, PAD id: {sp.pad_id()}, UNK id: {sp.unk_id()}")

    # Extract vocabulary with scores
    vocab = []
    for i in range(vocab_size):
        piece = sp.IdToPiece(i)
        score = sp.GetScore(i)
        vocab.append((piece, score))

    # Build Unigram tokenizer
    tokenizer = Tokenizer(models.Unigram(vocab, unk_id=sp.unk_id()))

    # Normalizer: lowercase + NFKC (matches the model's training)
    tokenizer.normalizer = normalizers.Sequence([
        normalizers.NFKC(),
        normalizers.Lowercase(),
    ])

    # Pre-tokenizer: Metaspace with ▁ replacement (SentencePiece convention)
    tokenizer.pre_tokenizer = pre_tokenizers.Metaspace(replacement="▁", add_prefix_space=True)

    # Verify round-trip
    test_cases = [
        "hello world",
        "bonjour comment ça va",
        "this is a test of the punctuation model",
    ]
    for text in test_cases:
        sp_ids = sp.EncodeAsIds(text.lower())
        tk_encoding = tokenizer.encode(text)
        tk_ids = list(tk_encoding.ids)
        match = "OK" if sp_ids == tk_ids else "MISMATCH"
        print(f"  [{match}] '{text}': SP={sp_ids[:8]}... TK={tk_ids[:8]}...")
        if sp_ids != tk_ids:
            print(f"    SP full: {sp_ids}")
            print(f"    TK full: {tk_ids}")

    return tokenizer


def main():
    model_path = download_model(MODEL_URL)
    tokenizer = convert(model_path)

    tokenizer.save(OUTPUT_FILE)
    size = Path(OUTPUT_FILE).stat().st_size
    print(f"\nSaved {OUTPUT_FILE} ({size:,} bytes)")
    print("Host this file and update TOKENIZER_URL in pcs_punctuation.rs")


if __name__ == "__main__":
    main()
