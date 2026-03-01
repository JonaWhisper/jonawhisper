# WhisperDictate

Local-first voice-to-text dictation for macOS. Runs in the menu bar, records audio via a global hotkey, transcribes with your choice of speech recognition engine, and pastes the result into the active application.

## Features

- **Menu bar app** — lives in the system tray, no dock icon
- **Custom global hotkey** — push-to-talk or toggle mode, any key combination (modifier key, combo like ⌘R, or standalone key like F13)
- **Native Whisper** — built-in speech recognition via whisper-rs with Metal GPU acceleration, or any OpenAI-compatible cloud API — unified model selector
- **9 cloud providers** — preconfigured presets (OpenAI, Groq, Cerebras, Gemini, Mistral, Fireworks, Together, DeepSeek, Anthropic) with API key testing and dynamic model discovery
- **Post-processing** — VAD silence detection (Silero VAD v6, discards silent recordings + trims silence), hallucination filtering, dictation commands, text cleanup via BERT punctuation restoration or LLM (local llama.cpp / cloud) with autoscale max_tokens — resilient fallback to raw text on any error
- **Bilingual UI** — French and English, auto-detected or manual override
- **Floating pill** — visual feedback (recording → transcribing), real-time spectrum bars, cancel support during recording or transcription
- **Audio ducking** — automatically lowers system volume during recording and restores it when done
- **Mic test** — test your microphone with live spectrum visualization in Settings
- **Model manager** — parallel model downloads with per-model progress, pause/resume, speed display, and benchmark badges
- **History badges** — each transcription shows processing details: ASR engine (local/cloud), language, text cleanup method (BERT/LLM/Cloud), hallucination filter status, and VAD silence trimming — with styled tooltips (shadcn-vue)
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

| Engine | Type | Description | Languages |
|---|---|---|---|
| **Whisper** ([whisper-rs](https://github.com/tazz4843/whisper-rs)) | Native (built-in) | GGML models running natively in Rust with Metal GPU acceleration on Apple Silicon. No external install needed. | Auto, FR, EN, ES, DE |
| **OpenAI API** | Cloud | Any [OpenAI-compatible](https://platform.openai.com/docs/api-reference/audio/createTranscription) server (OpenAI, local, or remote). Requires an API key. | Auto, FR, EN, ES, DE |

- **Whisper** is the default and recommended engine — runs locally with Metal GPU acceleration on Apple Silicon (M1/M2/M3/M4). Multiple model sizes available (tiny to large-v3-turbo).
- **OpenAI API** offloads transcription to the cloud (requires internet and an API key). Also works with any OpenAI-compatible server.

Models are downloaded and managed from within the app (Model Manager). All models are stored in `~/Library/Application Support/WhisperDictate/models/` (subdirectories: `whisper/`, `llm/`, `bert/`).

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

- Interface language (Auto / Français / English)
- Transcription model (local Whisper models + cloud providers in a single selector, GPU mode for local)
- Post-processing (VAD silence filter, hallucination filter, text cleanup with unified BERT/LLM model selector)
- Recording mode (push-to-talk / toggle)
- Hotkey and cancel shortcut
- Input microphone and audio ducking (reduce system volume while recording)

### Text cleanup

Optional post-transcription cleanup via a unified model selector:
- **BERT punctuation** — fast, lightweight punctuation restoration (ONNX Runtime)
- **Local LLM** — full text correction via llama.cpp with Metal GPU (GGUF models)
- **Cloud LLM** — full text correction via OpenAI-compatible or Anthropic API

Corrects punctuation, capitalization, and transcription artifacts without changing meaning. LLM max tokens auto-scales based on input length, with a configurable hard cap (default 4096, adjustable via slider 128–8192). On any LLM error (timeout, API failure, etc.), the raw transcription is preserved as fallback — text is never lost. Configure in Settings > Post-processing.

## Tech stack

| Layer | Technologies |
|---|---|
| Framework | [Tauri 2](https://v2.tauri.app/) |
| Backend | [Rust](https://www.rust-lang.org/) |
| Frontend | [Vue 3](https://vuejs.org/), [TypeScript](https://www.typescriptlang.org/), [Pinia](https://pinia.vuejs.org/), [Tailwind CSS](https://tailwindcss.com/), [shadcn-vue](https://www.shadcn-vue.com/) |
| Audio | [cpal](https://github.com/RustAudio/cpal) + [hound](https://github.com/ruuda/hound) (recording), [rustfft](https://github.com/ejmahler/RustFFT) (spectrum), CoreAudio FFI (audio ducking) |
| Transcription | [whisper-rs](https://github.com/tazz4843/whisper-rs) (native, Metal GPU) |
| Icons (tray/menu) | SDF (Signed Distance Field) hand-crafted en Rust, rendues en bitmap RGBA — zéro dépendance image, inspirées [Lucide](https://lucide.dev/) |
| Hotkey | Raw [CGEvent](https://developer.apple.com/documentation/coregraphics/cgevent) tap ([CoreGraphics](https://developer.apple.com/documentation/coregraphics) FFI) |
| Permissions | [objc2](https://github.com/madsmtm/objc2) ([AVFoundation](https://developer.apple.com/documentation/avfoundation), [CoreGraphics](https://developer.apple.com/documentation/coregraphics), [ApplicationServices](https://developer.apple.com/documentation/applicationservices)) |
| i18n | [vue-i18n](https://vue-i18n.intlify.dev/) (frontend), [rust-i18n](https://github.com/longbridge/rust-i18n) (backend/tray menu) |

## Dependencies

### Backend (Rust crates)

**Core / Framework**

| Crate | Rôle |
|-------|------|
| [`tauri`](https://github.com/tauri-apps/tauri) | Framework app (webview, tray, IPC, windows) |
| [`tauri-plugin-clipboard-manager`](https://github.com/tauri-apps/plugins-workspace/tree/v2/plugins/clipboard-manager) | Accès clipboard (paste simulation) |
| [`serde`](https://github.com/serde-rs/serde) / [`serde_json`](https://github.com/serde-rs/json) | Sérialisation JSON (settings, IPC) |
| [`tokio`](https://github.com/tokio-rs/tokio) | Runtime async (transcription, téléchargements) |
| [`log`](https://github.com/rust-lang/log) / [`env_logger`](https://github.com/rust-cli/env_logger) | Logging |
| [`thiserror`](https://github.com/dtolnay/thiserror) | Dérivation d'erreurs typées |

**Audio / Transcription**

| Crate | Rôle |
|-------|------|
| [`cpal`](https://github.com/RustAudio/cpal) | Capture audio cross-platform (enregistrement micro) |
| [`hound`](https://github.com/ruuda/hound) | Écriture fichiers WAV |
| [`rustfft`](https://github.com/ejmahler/RustFFT) | FFT pour spectre audio (visualisation) |
| [`whisper-rs`](https://github.com/tazz4843/whisper-rs) | Transcription native Whisper (GGML, Metal GPU sur macOS) |
| [`llama-cpp-2`](https://github.com/utilityai/llama-cpp-rs) | Inférence LLM locale (GGUF, Metal GPU, text cleanup) |
| [`ort`](https://github.com/pykeIO/ort) | ONNX Runtime (Silero VAD, BERT ponctuation) |
| [`ndarray`](https://github.com/rust-ndarray/ndarray) | Tensors pour VAD (état LSTM, entrées/sorties ONNX) |
| [`encoding_rs`](https://github.com/nickel-org/encoding_rs) | Décodage UTF-8 incrémental (sortie LLM token par token) |

**Réseau / IO**

| Crate | Rôle |
|-------|------|
| [`reqwest`](https://github.com/seanmonstar/reqwest) | HTTP client (téléchargement modèles, API ASR/LLM) |
| [`futures-util`](https://github.com/rust-lang/futures-rs) | Utilitaires async (streams de téléchargement) |
| [`dirs`](https://github.com/dirs-dev/dirs-rs) | Chemins système (`~/Library/Application Support/`, etc.) |
| [`shellexpand`](https://github.com/netvl/shellexpand) | Expansion de chemins (`~`, variables d'env) |
| [`rusqlite`](https://github.com/rusqlite/rusqlite) | SQLite embarqué (historique des transcriptions, WAL) |

**Concurrence**

| Crate | Rôle |
|-------|------|
| [`crossbeam-channel`](https://github.com/crossbeam-rs/crossbeam) | Canaux multi-producteur (thread audio, hotkey) |

**Traitement texte / i18n**

| Crate | Rôle |
|-------|------|
| [`regex`](https://github.com/rust-lang/regex) | Post-traitement transcriptions (filtrage hallucinations) |
| [`rust-i18n`](https://github.com/longbridge/rust-i18n) | Internationalisation backend (menus tray, messages) |
| [`sys-locale`](https://github.com/1Password/sys-locale) | Détection locale système |

**macOS uniquement**

| Crate | Rôle |
|-------|------|
| [`core-graphics`](https://github.com/nickel-org/core-foundation-rs) / [`core-foundation`](https://github.com/nickel-org/core-foundation-rs) | CGEvent tap (hotkey), CGEvent paste (Cmd+V) |
| [`objc2`](https://github.com/madsmtm/objc2) / [`objc2-foundation`](https://github.com/madsmtm/objc2) / [`objc2-app-kit`](https://github.com/madsmtm/objc2) | FFI Objective-C (permissions micro, NSPasteboard, NSSound, activation app) |
| [`block2`](https://github.com/madsmtm/objc2) | Blocks Objective-C (callback `requestAccessForMediaType`) |

### Frontend (npm)

**App**

| Package | Rôle |
|---------|------|
| [`vue`](https://github.com/vuejs/core) | Framework UI |
| [`vue-router`](https://github.com/vuejs/router) | Routing (pill, settings, model-manager, history, setup) |
| [`pinia`](https://github.com/vuejs/pinia) | State management |
| [`vue-i18n`](https://github.com/intlify/vue-i18n) | Internationalisation (FR/EN) |
| [`@tauri-apps/api`](https://github.com/tauri-apps/tauri) | Bridge IPC vers le backend Rust |

**UI**

| Package | Rôle |
|---------|------|
| [`lucide-vue-next`](https://github.com/lucide-icons/lucide) | Icones (tree-shakeable, ~1500 icones) |
| [`reka-ui`](https://github.com/unovue/reka-ui) | Primitives UI headless (base de shadcn-vue) |
| [`class-variance-authority`](https://github.com/joe-bell/cva) | Variants CSS pour composants (shadcn-vue) |
| [`clsx`](https://github.com/lukeed/clsx) / [`tailwind-merge`](https://github.com/dcastil/tailwind-merge) | Utilitaires classes CSS |
| [`@vueuse/core`](https://github.com/vueuse/vueuse) | Composables Vue (utilisé par shadcn-vue) |

**Styling**

| Package | Rôle |
|---------|------|
| [`tailwindcss`](https://github.com/tailwindlabs/tailwindcss) / [`tailwindcss-animate`](https://github.com/jamiebuilds/tailwindcss-animate) | CSS utility-first + animations |
| [`autoprefixer`](https://github.com/postcss/autoprefixer) / [`postcss`](https://github.com/postcss/postcss) | Post-traitement CSS |

**Plugins Tauri (JS bindings)**

| Package | Rôle |
|---------|------|
| [`@tauri-apps/plugin-clipboard-manager`](https://github.com/tauri-apps/plugins-workspace/tree/v2/plugins/clipboard-manager) | Écriture clipboard avant paste |

**Dev**

| Package | Rôle |
|---------|------|
| [`@tauri-apps/cli`](https://github.com/tauri-apps/tauri) | CLI Tauri (build, dev) |
| [`vite`](https://github.com/vitejs/vite) / [`@vitejs/plugin-vue`](https://github.com/vitejs/vite-plugin-vue) | Bundler + plugin Vue SFC |
| [`typescript`](https://github.com/microsoft/TypeScript) / [`vue-tsc`](https://github.com/vuejs/language-tools) | Type checking |
| [`@vue/tsconfig`](https://github.com/vuejs/tsconfig) / [`@types/node`](https://github.com/DefinitelyTyped/DefinitelyTyped) | Config TypeScript |

## Project structure

See [ARCHITECTURE.md](ARCHITECTURE.md) for a detailed architecture guide with data flows, threading model, and module responsibilities.

```
src/                     Vue frontend
  views/                 Pages (Settings, ModelManager, History, FloatingPill, SetupWizard)
  components/            UI components (ShortcutCapture, SpectrumBars, ModelCell, BenchmarkBadges, …)
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
    vad.rs               Voice Activity Detection (Silero VAD v6 via ONNX Runtime)
    audio.rs             cpal recording & FFT
    transcriber.rs       Transcription dispatcher (native whisper / cloud API)
    post_processor.rs    Text post-processing
    llm_cleanup.rs       Cloud LLM text cleanup (OpenAI/Anthropic)
    llm_local.rs         Local LLM text cleanup (llama.cpp)
    llm_prompt.rs        Shared LLM module (error types, output sanitization, prompt)
    bert_punctuation.rs  BERT punctuation restoration (ONNX Runtime)
    tray.rs              Menu bar menu & tray icon states
    menu_icons.rs        SDF-rendered bitmap icons (tray bar + device menu)
    events.rs            Centralised event name constants
    errors.rs            App error types
    engines/             Speech recognition engine adapters
    platform/            OS-specific code (permissions, hotkey, paste, audio devices, audio ducking)
build.sh                 Build + codesign + package script
```

## License

Private project.
