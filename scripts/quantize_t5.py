#!/usr/bin/env python3
"""Quantize T5 correction ONNX models from FP32 to INT8 (dynamic quantization).

Reads encoder_model.onnx + decoder_model.onnx from each model directory,
produces encoder_model_int8.onnx + decoder_model_int8.onnx alongside them.

Usage:
    python scripts/quantize_t5.py [model_dir ...]

If no model_dir given, processes all models in ~/Library/Application Support/JonaWhisper/models/correction/
"""

import sys
from pathlib import Path

try:
    from onnxruntime.quantization import quantize_dynamic, QuantType
except ImportError:
    print("ERROR: onnxruntime not installed. Run: pip install onnxruntime")
    sys.exit(1)

DEFAULT_DIR = Path.home() / "Library" / "Application Support" / "JonaWhisper" / "models" / "correction"


def quantize_model(model_path: Path, output_path: Path):
    """Quantize a single ONNX model to INT8."""
    print(f"  Quantizing {model_path.name} → {output_path.name}")
    print(f"    Input:  {model_path.stat().st_size / 1024 / 1024:.1f} MB")

    quantize_dynamic(
        str(model_path),
        str(output_path),
        weight_type=QuantType.QInt8,
    )

    print(f"    Output: {output_path.stat().st_size / 1024 / 1024:.1f} MB")
    ratio = output_path.stat().st_size / model_path.stat().st_size
    print(f"    Ratio:  {ratio:.1%}")


def process_model_dir(model_dir: Path):
    """Quantize encoder and decoder in a model directory."""
    print(f"\nProcessing: {model_dir.name}")

    for name in ["encoder_model", "decoder_model"]:
        fp32 = model_dir / f"{name}.onnx"
        int8 = model_dir / f"{name}_int8.onnx"

        if not fp32.exists():
            print(f"  SKIP: {fp32} not found")
            continue

        if int8.exists():
            print(f"  SKIP: {int8.name} already exists")
            continue

        quantize_model(fp32, int8)


def main():
    if len(sys.argv) > 1:
        dirs = [Path(d) for d in sys.argv[1:]]
    else:
        if not DEFAULT_DIR.exists():
            print(f"No models found at {DEFAULT_DIR}")
            sys.exit(1)
        dirs = sorted(d for d in DEFAULT_DIR.iterdir() if d.is_dir())

    if not dirs:
        print("No model directories to process")
        sys.exit(1)

    for d in dirs:
        process_model_dir(d)

    print("\nDone!")


if __name__ == "__main__":
    main()
