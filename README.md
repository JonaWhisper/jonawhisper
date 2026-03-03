# WhisperDictate

Local-first voice-to-text dictation for macOS. Runs in the menu bar, records audio via a global hotkey, transcribes with your choice of speech recognition engine, and pastes the result into the active application.

All processing happens on your machine — no data leaves your Mac unless you choose a cloud provider.

![CI](https://github.com/jplot/dictate-macos/actions/workflows/ci.yml/badge.svg)

## Features

- **Menu bar app** — lives in the system tray, no dock icon
- **Global hotkey** — push-to-talk or toggle mode, any key combination (modifier, combo, or standalone key)
- **6 ASR engines** — Whisper, Canary, Parakeet-TDT, Qwen3-ASR, Voxtral (all local, GPU-accelerated), plus any OpenAI-compatible cloud API
- **9 cloud presets** — OpenAI, Groq, Cerebras, Gemini, Mistral, Fireworks, Together, DeepSeek, Anthropic
- **Text cleanup pipeline** — VAD silence trimming, hallucination filter, dictation commands, punctuation (BERT/PCS), grammar correction (T5), or full LLM cleanup (local/cloud)
- **Floating pill** — real-time spectrum visualization, recording/transcribing states, cancel support
- **Model manager** — parallel downloads with progress, pause/resume, benchmarks
- **Paginated history** — backend-driven search, infinite scroll, processing badges
- **Bilingual UI** — French and English

## Engines

### ASR (Speech-to-Text)

| Engine | Params | Size | Languages | GPU | Best WER | RTF |
|--------|--------|------|-----------|-----|----------|-----|
| [Whisper](https://github.com/tazz4843/whisper-rs) | 39M–1.55B | 75 MB–3.1 GB | 99 | Metal | 1.5% | 0.05–0.50 |
| [Canary](https://catalog.ngc.nvidia.com/orgs/nvidia/teams/nemo/models/canary-180m-flash) | 182M | 213 MB | 4 (FR/EN/DE/ES) | CoreML | 1.87% | 0.15 |
| [Parakeet-TDT](https://catalog.ngc.nvidia.com/orgs/nvidia/teams/nemo/models/parakeet-tdt-0.6b-v2) | 600M | 703 MB | 25 European | CoreML | 1.5% | 0.10 |
| [Qwen3-ASR](https://github.com/huanglizhuo/QwenASR) | 600M | 1.88 GB | 30 | Accelerate/AMX | 2.0% | 0.15 |
| [Voxtral](https://github.com/antirez/voxtral.c) | 4.4B | 8.9 GB | 13 | Metal | 8.7% | 0.40 |
| Cloud (OpenAI API) | — | — | depends on provider | — | — | — |

**Recommendations**: Parakeet-TDT for best accuracy (1.5% WER, 25 languages). Whisper V3 Turbo for best balance (2.1% WER, 99 languages, 0.25 RTF). Whisper Tiny for speed (75 MB, 0.05 RTF).

### Punctuation & Capitalization

| Engine | Params | Size | Languages | Capitalization | Speed |
|--------|--------|------|-----------|----------------|-------|
| BERT Fullstop Large (ort) | 560M | 562 MB | 4 (FR/EN/DE/IT) | No | ~100ms |
| BERT Fullstop Base (Candle) | 280M | 1.1 GB | 5 (+ NL) | No | ~80ms |
| **PCS 47 Languages** (ort) | 230M | 233 MB | **47** | **Yes** (4 heads) | ~50ms |

**Recommendation**: PCS — smaller, faster, 47 languages, native capitalization.

### Grammar & Spelling Correction

| Engine | Params | Size | Languages | Speed |
|--------|--------|------|-----------|-------|
| **GEC T5 Small** | 60M | 242 MB | 11 (multilingual) | ~200ms |
| T5 Spell FR | 220M | 892 MB | FR | ~500ms |
| FlanEC Large | 250M | 990 MB | EN | ~800ms |
| Flan-T5 Grammar | 783M | 3.1 GB | EN | ~2s |

All run via Candle with Metal GPU, autoregressive decoding with KV cache.

### Local LLM (Text Cleanup)

| Model | Params | Size | Languages |
|-------|--------|------|-----------|
| Qwen3 0.6B | 0.6B | 484 MB | FR/EN/ES/DE |
| Gemma 3 1B | 1.0B | 806 MB | FR/EN/ES/DE |
| **Qwen3 1.7B** | 1.7B | 1.28 GB | FR/EN/ES/DE |
| Ministral 3B | 3.0B | 2.15 GB | FR/EN/ES/DE |
| Qwen3 4B | 4.0B | 2.50 GB | FR/EN/ES/DE |

All GGUF Q4 quantized, run via llama.cpp with Metal GPU. 11 models available total.

## Processing pipelines

### Audio pipeline

```
Mic (cpal) → WAV 16 kHz → VAD (Silero v6) → Trim silence → ASR
```

| Stage | Status | Description |
|-------|--------|-------------|
| VAD (Silero) | Done | Discards silent recordings, trims leading/trailing silence |
| Denoising | Planned | Hybrid approach: denoised for VAD boundaries, original for ASR |
| Device presets | Planned | Per-mic gain, noise gate, normalization |

See [docs/AUDIO-PIPELINE.md](docs/AUDIO-PIPELINE.md) for the full architecture.

### Text pipeline

```
ASR raw → Hallucination filter → Dictation commands → [Punctuation / Correction / LLM] → Finalize → Paste
```

| Stage | Status | Description |
|-------|--------|-------------|
| Hallucination filter | Done | 30+ regex patterns (FR/EN) |
| Dictation commands | Done | Voice commands → punctuation ("virgule" → ",") |
| Disfluency removal | Planned | Strip fillers (euh, uh, um) |
| Punctuation | Done | BERT or PCS token classification |
| Correction | Done | T5 encoder-decoder (grammar, spelling) |
| LLM cleanup | Done | Local (llama.cpp) or cloud (OpenAI/Anthropic) |
| ITN | Planned | Inverse text normalization ("vingt-trois" → "23") |
| Finalize | Done | Spacing, capitalization |

See [docs/TEXT-PIPELINE.md](docs/TEXT-PIPELINE.md) for the full architecture.

## Requirements

- macOS 13.0+ (Apple Silicon recommended)
- [Rust](https://www.rust-lang.org/tools/install) (stable)
- [Node.js](https://nodejs.org/) 24+
- Xcode Command Line Tools (`xcode-select --install`)

## Build

```bash
npm install
./build.sh
open build/WhisperDictate.app
```

The build script produces `build/WhisperDictate.app` and `build/WhisperDictate.dmg`. If a Developer certificate is available, the app is code-signed with entitlements for stable TCC entries.

For a debug build:

```bash
./build.sh debug
```

## Development

```bash
npm install
npm run tauri dev
```

This starts the Vite dev server with hot reload for the frontend. Rust changes trigger a rebuild automatically.

## Permissions

On first launch, a setup wizard asks for three macOS permissions:

| Permission | Used for | macOS API |
|---|---|---|
| **Microphone** | Audio recording | [AVCaptureDevice](https://developer.apple.com/documentation/avfoundation/avcapturedevice) authorization |
| **Accessibility** | Paste simulation (Cmd+V via [CGEvent](https://developer.apple.com/documentation/coregraphics/cgevent)) | [AXIsProcessTrusted](https://developer.apple.com/documentation/applicationservices/1459186-axisprocesstrusted) |
| **Input Monitoring** | Global hotkey detection ([CGEvent tap](https://developer.apple.com/documentation/coregraphics/1454426-cgeventtapcreate)) | [TCC](https://support.apple.com/guide/security/controlling-app-access-to-files-secddd1d86a6/web) ListenEvent |

## Usage

1. Launch the app — it appears as a menu bar icon
2. Press and hold the hotkey (default: Right Command) to record
3. Release to transcribe — the text is pasted into the active app
4. In toggle mode, press once to start, press again to stop

**Cancel**: Press Escape at any time to cancel recording or transcription.

**Settings**: Open from the tray menu to configure language, ASR model, text cleanup, hotkey, microphone, and cloud providers.

Models are downloaded and managed from within the app. All models are stored in `~/Library/Application Support/WhisperDictate/models/`.

## Tech stack

| Layer | Technologies |
|---|---|
| Framework | [Tauri 2](https://v2.tauri.app/) |
| Backend | [Rust](https://www.rust-lang.org/) |
| Frontend | [Vue 3](https://vuejs.org/), [TypeScript](https://www.typescriptlang.org/), [Pinia](https://pinia.vuejs.org/), [Tailwind CSS](https://tailwindcss.com/), [shadcn-vue](https://www.shadcn-vue.com/) |
| Audio | [cpal](https://github.com/RustAudio/cpal) + [hound](https://github.com/ruuda/hound) (recording), [rustfft](https://github.com/ejmahler/RustFFT) (spectrum), CoreAudio FFI (ducking) |
| ASR | [whisper-rs](https://github.com/tazz4843/whisper-rs) (Metal), [ort](https://github.com/pykeIO/ort) + CoreML (Canary, Parakeet), [qwen-asr](https://github.com/huanglizhuo/QwenASR) (AMX), [voxtral.c](https://github.com/antirez/voxtral.c) (Metal) |
| Text cleanup | [candle](https://github.com/huggingface/candle) (T5, BERT Metal), [ort](https://github.com/pykeIO/ort) (BERT, PCS CoreML), [llama-cpp-2](https://github.com/utilityai/llama-cpp-rs) (local LLM Metal) |
| Icons | SDF (Signed Distance Field) in Rust, RGBA bitmaps — zero image dependencies |
| Hotkey | Raw [CGEvent](https://developer.apple.com/documentation/coregraphics/cgevent) tap (CoreGraphics FFI) |
| Permissions | [objc2](https://github.com/madsmtm/objc2) (AVFoundation, CoreGraphics, ApplicationServices) |
| i18n | [vue-i18n](https://vue-i18n.intlify.dev/) (frontend), [rust-i18n](https://github.com/longbridge/rust-i18n) (backend) |

See [docs/DEPENDENCIES.md](docs/DEPENDENCIES.md) for the full dependency list with rationale for each choice.

## Project structure

```
src/                       Vue frontend
  views/                   Pages (Panel, SetupWizard)
  sections/                Settings sections (9 sections)
  components/              UI components
  stores/                  Pinia stores (app, history, settings, engines, downloads)
  utils/                   Shared utilities (shortcut, formatting)
  i18n/                    Translations (en.json, fr.json)
src-tauri/                 Rust backend
  src/
    lib.rs                 Tauri setup & app lifecycle
    commands.rs            Tauri IPC commands
    state.rs               App state & persistent preferences
    recording.rs           Recording state machine & transcription queue
    audio.rs               cpal recording & FFT
    asr/                   ASR inference (whisper, canary, parakeet, qwen, voxtral, mel)
    cleanup/               Text cleanup (bert, candle, pcs, t5, vad, llm, post_processor)
    engines/               Engine catalog & model downloads
    platform/              macOS-specific (hotkey, permissions, paste, audio devices)
    ui/                    Native UI (tray, pill overlay, SDF icons)
  voxtral-c/               Vendored voxtral.c sources (C + Metal)
docs/                      Technical documentation
  AUDIO-PIPELINE.md        Audio preprocessing architecture & roadmap
  TEXT-PIPELINE.md         Text postprocessing architecture & roadmap
  BENCHMARK.md             Full benchmark data (ASR, LLM, punctuation, correction)
build.sh                   Build + codesign + package script
```

See [ARCHITECTURE.md](ARCHITECTURE.md) for the full architecture guide with data flows, threading model, and module responsibilities.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for commit conventions, development setup, and how to submit PRs.

## License

MIT
