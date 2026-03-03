# WhisperDictate

Local-first voice-to-text dictation for macOS. Runs in the menu bar, records audio via a global hotkey, transcribes with your choice of speech recognition engine, and pastes the result into the active application.

## Features

- **Menu bar app** — lives in the system tray, no dock icon
- **Custom global hotkey** — push-to-talk or toggle mode, multi-key shortcuts (up to 4 keys: modifier key, combo like ⌘+A, or standalone key like F13)
- **Native Whisper** — built-in speech recognition via whisper-rs with Metal GPU acceleration, or any OpenAI-compatible cloud API — unified model selector
- **9 cloud providers** — preconfigured presets (OpenAI, Groq, Cerebras, Gemini, Mistral, Fireworks, Together, DeepSeek, Anthropic) with API key testing and dynamic model discovery
- **Post-processing** — VAD silence detection (Silero VAD v6, discards silent recordings + trims silence), hallucination filtering, dictation commands, text cleanup via punctuation restoration (BERT or PCS 47-language), T5 correction models (grammar, spelling, post-ASR error correction), or LLM (local llama.cpp / cloud) with autoscale max_tokens — resilient fallback to raw text on any error
- **Bilingual UI** — French and English, auto-detected or manual override
- **Floating pill** — visual feedback (recording → transcribing), real-time spectrum bars, cancel support during recording or transcription
- **Audio ducking** — automatically lowers system volume during recording and restores it when done
- **Mic test** — test your microphone with live spectrum visualization in Settings
- **Model manager** — parallel model downloads with per-model progress, pause/resume, speed display, and benchmark badges
- **History badges** — each transcription shows processing details: ASR engine (local/cloud), language, text cleanup method (Punctuation/Correction/LLM/Cloud), hallucination filter status, and VAD silence trimming — with styled tooltips (shadcn-vue)
- **Paginated history** — backend-driven search (SQLite LIKE) and infinite scroll for fast opening even with thousands of entries

## Requirements

- macOS 13.0+
- [Rust](https://www.rust-lang.org/tools/install) (stable)
- [Node.js](https://nodejs.org/) (LTS)

## Build

```bash
# Install frontend dependencies
npm install

# Build, sign, and package
./build.sh

# Launch
open build/WhisperDictate.app
```

The build script produces `build/WhisperDictate.app` and `build/WhisperDictate.dmg`. If a Developer certificate is available, the app is code-signed with entitlements for stable TCC entries (permissions survive rebuilds).

For a debug build:

```bash
./build.sh debug
```

## Permissions

On first launch, a setup wizard asks for three macOS permissions:

| Permission | Used for | macOS API |
|---|---|---|
| **Microphone** | Audio recording | [AVCaptureDevice](https://developer.apple.com/documentation/avfoundation/avcapturedevice) authorization |
| **Accessibility** | Paste simulation (Cmd+V via [CGEvent](https://developer.apple.com/documentation/coregraphics/cgevent)) | [AXIsProcessTrusted](https://developer.apple.com/documentation/applicationservices/1459186-axisprocesstrusted) |
| **Input Monitoring** | Global hotkey detection ([CGEvent tap](https://developer.apple.com/documentation/coregraphics/1454426-cgeventtapcreate)) | [TCC](https://support.apple.com/guide/security/controlling-app-access-to-files-secddd1d86a6/web) ListenEvent |

## Engines

| Engine | Type | Description | Languages | GPU |
|---|---|---|---|---|
| **Whisper** ([whisper-rs](https://github.com/tazz4843/whisper-rs)) | Native | GGML models with Metal GPU. Default engine. | 99 languages | Metal |
| **Canary** (NVIDIA) | Native | Ultra-light encoder-decoder ASR (182M params, ONNX). | FR, EN, DE, ES | CoreML (Metal/ANE) |
| **Parakeet-TDT** (NVIDIA) | Native | TDT transducer ASR (0.6B params, ONNX int8). Best WER. | 25 European languages | CoreML (Metal/ANE) |
| **Qwen3-ASR** (Alibaba) | Native | Encoder-decoder audio LLM (0.6B params, safetensors). | 30 languages | Accelerate (AMX) |
| **OpenAI API** | Cloud | Any [OpenAI-compatible](https://platform.openai.com/docs/api-reference/audio/createTranscription) server. | Depends on model | N/A |

- **Whisper** is the default and recommended engine — runs locally with Metal GPU acceleration on Apple Silicon (M1/M2/M3/M4). Multiple model sizes available (tiny to large-v3-turbo).
- **Canary** — NVIDIA's ultra-light model, beats Whisper Medium quality at 1/7th the size.
- **Parakeet-TDT** — NVIDIA's TDT transducer with duration head for fast frame-skipping inference. Best overall accuracy.
- **Qwen3-ASR** — Alibaba's audio LLM, hardware-accelerated via Apple's AMX coprocessor.
- **OpenAI API** offloads transcription to the cloud (requires internet and an API key). Also works with any OpenAI-compatible server.

Models are downloaded and managed from within the app (Model Manager). All models are stored in `~/Library/Application Support/WhisperDictate/models/` (subdirectories: `whisper/`, `canary/`, `parakeet/`, `qwen-asr/`, `llm/`, `bert/`, `pcs/`, `correction/`).

## Usage

1. Launch the app - it appears as a menu bar icon
2. Press and hold the hotkey (default: Right Command) to record
3. Release to transcribe - the text is pasted into the active app
4. In toggle mode, press once to start recording, press again to stop

### Cancel

Press Escape at any time to cancel:
- **During recording** — stops recording, discards audio
- **During transcription** — cancels the transcription in progress, discards text

### Settings

Open Settings from the tray menu to configure:

- **Recents** — transcription history with search, copy, delete
- **Models** — download and manage speech recognition and cleanup models
- **Transcription** — ASR model (local + cloud unified selector), language, GPU acceleration
- **Processing** — VAD silence filter, hallucination filter, text cleanup (BERT/PCS/T5/LLM)
- **Shortcuts** — hotkey, recording mode (push-to-talk / toggle), cancel shortcut
- **Microphone** — input device, mic test with spectrum, audio ducking
- **Providers** — cloud provider configuration (9 presets + custom)
- **Permissions** — macOS permission status and grant buttons
- **General** — appearance (theme), interface language

### Text cleanup

Optional post-transcription cleanup via a unified model selector:
- **BERT punctuation** — fast punctuation restoration, 4 languages (ONNX Runtime)
- **PCS punctuation** — punctuation + capitalization + segmentation, 47 languages (ONNX Runtime, SentencePiece tokenizer)
- **T5 correction** — encoder-decoder text correction via Candle (Metal GPU). 4 models: GEC T5 Small (60M, multilingual grammar), T5 Spell FR (220M, French spelling), FlanEC Large (250M, post-ASR errors), Flan-T5 Grammar (783M, grammar synthesis)
- **Local LLM** — full text correction via llama.cpp with Metal GPU (GGUF models)
- **Cloud LLM** — full text correction via OpenAI-compatible or Anthropic API

Corrects punctuation, capitalization, spelling, grammar, and transcription artifacts without changing meaning. T5 models offer a middle ground between fast punctuation (BERT/PCS) and full LLM correction — smarter grammar/spelling fixes at lower latency. LLM max tokens auto-scales based on input length, with a configurable hard cap (default 4096, adjustable via slider 128–8192). On any error (timeout, API failure, etc.), the raw transcription is preserved as fallback — text is never lost. Configure in Settings > Post-processing.

## Tech stack

| Layer | Technologies |
|---|---|
| Backend | Rust |
| Frontend | Vue 3, TypeScript, Pinia, Tailwind CSS, shadcn-vue |
| Audio | cpal + hound (recording), rustfft (spectrum), CoreAudio FFI (ducking) |
| Transcription | whisper-rs (Metal GPU), ort + CoreML (Canary, Parakeet), qwen-asr (Accelerate/AMX) |
| Text cleanup | candle (BERT punctuation, T5 correction, Metal GPU), ort (PCS punctuation), llama-cpp-2 (local LLM) |
| Icons | Lucide (frontend), SDF hand-crafted in Rust (tray/menu bitmaps) |
| Hotkey | Raw CGEvent tap (CoreGraphics FFI), multi-key support |
| Permissions | objc2 (AVFoundation, CoreGraphics, ApplicationServices) |
| i18n | vue-i18n (frontend), rust-i18n (backend) |

## Project structure

See [ARCHITECTURE.md](ARCHITECTURE.md) for a detailed architecture guide with data flows, threading model, and module responsibilities.

```
src/                     Vue frontend
  views/                 Pages (Panel, SetupWizard)
  sections/              Settings panel sections (Recents, Models, Transcription, Processing, Shortcuts, Microphone, Providers, Permissions, General)
  components/            UI components (ShortcutCapture, SpectrumBars, ModelCell, ModelOption, SegmentedToggle, …)
  stores/                Pinia stores (app, history, settings, engines, downloads)
  config/providers.ts    Cloud provider presets and model filter helpers
  utils/                 Shared utilities (shortcut types, formatting, byte/speed formatters)
  i18n/                  Translations (en.json, fr.json)
src-tauri/               Rust backend
  src/
    lib.rs               Tauri setup & app lifecycle
    commands.rs          Tauri IPC commands
    state.rs             App state & persistent preferences
    migrations.rs        Versioned preference migrations & model relocation
    recording.rs         Recording state machine & audio thread
    audio.rs             cpal recording & FFT spectrum
    events.rs            Centralised event name constants
    errors.rs            App error types
    asr/                 ASR inference
      mod.rs             Transcriber dispatch (cloud + local engine routing)
      whisper.rs         WhisperCtx + transcribe_native (whisper-rs, Metal GPU)
      canary.rs          NVIDIA Canary ASR (ONNX Runtime + CoreML)
      parakeet.rs        NVIDIA Parakeet-TDT ASR (vendored TDT decoder, ONNX + CoreML)
      qwen.rs            Alibaba Qwen3-ASR (qwen-asr crate, Accelerate/AMX)
      mel.rs             Mel features (HTK/Slaney scales, pre-emphasis)
    cleanup/             Text cleanup pipeline
      mod.rs             Re-exports
      bert.rs            BERT punctuation (ONNX Runtime)
      candle.rs          BERT punctuation (Candle, safetensors, Metal GPU)
      pcs.rs             PCS punctuation + capitalization + segmentation (ONNX, 47 languages)
      common.rs          Shared punctuation logic (windowing, labels, download)
      t5.rs              T5 encoder-decoder correction (Candle, Metal GPU)
      vad.rs             Silero VAD v6 (bundled ONNX model)
      post_processor.rs  Hallucination filter + dictation commands
      llm_cloud.rs       Cloud LLM cleanup (OpenAI/Anthropic API)
      llm_local.rs       Local LLM cleanup (llama.cpp, Metal GPU)
      llm_prompt.rs      LLM prompt templates + sanitization
    engines/             Engine catalog + registration (no inference logic)
      mod.rs             ASREngine trait, EngineCatalog, model lookup
      downloader.rs      Model download/delete/partial management
      ort_session.rs     Shared ort session builder with CoreML EP
      whisper.rs         Whisper model catalog
      canary.rs, parakeet.rs, qwen.rs, bert.rs, pcs.rs, correction.rs, llama.rs, openai_api.rs
    platform/            OS-specific code (macOS FFI)
      hotkey.rs          Multi-key shortcuts, CGEvent tap, capture mode
      macos.rs           Permission checks & requests
      paste.rs           Clipboard + Cmd+V paste simulation
      audio_devices.rs   CoreAudio device enumeration
      audio_ducking.rs   System volume ducking during recording
      ffi.rs             Raw C declarations
    ui/                  Native UI
      tray.rs            System tray icon, context menu, pill lifecycle
      pill.rs            Native pill overlay (AppKit NSWindow, SDF rendering)
      menu_icons.rs      SDF-rendered bitmap icons (tray bar + device menu)
build.sh                 Build + codesign + package script
```

