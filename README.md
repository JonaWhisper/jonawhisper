# WhisperDictate

Local-first voice-to-text dictation for macOS. Runs in the menu bar, records audio via a global hotkey, transcribes with your choice of speech recognition engine, and pastes the result into the active application.

## Features

- **Menu bar app** — lives in the system tray, no dock icon
- **Global hotkey** — push-to-talk or toggle mode (Right Command, Right Option, Right Control, or Right Shift)
- **Multiple speech engines** — Whisper (C++), Faster Whisper, MLX Whisper, Vosk, Moonshine, or any OpenAI-compatible API
- **Post-processing** — hallucination filtering, dictation commands (new line, new paragraph), optional LLM text cleanup
- **Bilingual UI** — French and English, auto-detected or manual override
- **Floating pill** — visual recording indicator with real-time spectrum bars
- **Mic test** — test your microphone with live spectrum visualization in Settings
- **Model manager** — download, select, and manage models per engine

## Requirements

- macOS 13.0+
- [Rust](https://www.rust-lang.org/tools/install) (stable)
- [Node.js](https://nodejs.org/) (LTS)
- At least one speech engine installed (see [Engines](#engines) below)

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

WhisperDictate supports multiple speech recognition backends. Each has different strengths:

| Engine | Best for | Install | Languages |
|---|---|---|---|
| [**whisper.cpp**](https://github.com/ggerganov/whisper.cpp) | CPU, lightweight, general use | `brew install whisper-cpp` | Auto, FR, EN, ES, DE |
| [**Faster Whisper**](https://github.com/SYSTRAN/faster-whisper) (CTranslate2) | GPU acceleration (~4x faster) | `pip install whisper-ctranslate2` | Auto, FR, EN, ES, DE |
| [**MLX Whisper**](https://github.com/ml-explore/mlx-examples/tree/main/whisper) | Apple Silicon (M1/M2/M3/M4) | `pip install mlx-whisper` | Auto, FR, EN, ES, DE |
| [**Vosk**](https://alphacephei.com/vosk/) | Low resources, small models | `pip install vosk` | EN, FR |
| [**Moonshine**](https://github.com/usefulsensors/moonshine) | Ultra-fast, English only | `pip install useful-moonshine` | EN |
| **OpenAI API** | Cloud, no local compute | API key only | Auto, FR, EN, ES, DE |

- **whisper.cpp** is the default engine — C++ implementation, runs well on any Mac.
- **MLX Whisper** is the recommended engine on Apple Silicon — uses Apple's [MLX framework](https://github.com/ml-explore/mlx) for best performance on the Neural Engine and unified GPU.
- **Faster Whisper** shines with NVIDIA GPUs via [CTranslate2](https://github.com/OpenNMT/CTranslate2) optimization.
- **Vosk** and **Moonshine** are lightweight alternatives for quick dictation with smaller models.
- **OpenAI API** offloads transcription to the cloud (requires internet and an API key). Also works with any [OpenAI-compatible](https://platform.openai.com/docs/api-reference/audio/createTranscription) server (local or remote).

Models are downloaded and managed from within the app (Model Manager).

## Usage

1. Launch the app - it appears as a menu bar icon
2. Press and hold the hotkey (default: Right Command) to record
3. Release to transcribe - the text is pasted into the active app
4. In toggle mode, press once to start recording, press again to stop

### Cancel a recording

Press Escape while recording to cancel without transcribing.

### Settings

Open Settings from the tray menu to configure:

- Interface language (Auto / Francais / English)
- Post-processing (hallucination filter, LLM cleanup)
- Recording mode (push-to-talk / toggle)
- Hotkey and cancel shortcut
- Input microphone

### LLM text cleanup

Optional post-transcription cleanup via an LLM API (OpenAI-compatible or Anthropic). Corrects punctuation, capitalization, and transcription artifacts without changing meaning. Configure in Settings > Post-processing.

## Tech stack

| Layer | Technologies |
|---|---|
| Framework | [Tauri 2](https://v2.tauri.app/) |
| Backend | [Rust](https://www.rust-lang.org/) |
| Frontend | [Vue 3](https://vuejs.org/), [TypeScript](https://www.typescriptlang.org/), [Pinia](https://pinia.vuejs.org/), [Tailwind CSS](https://tailwindcss.com/), [shadcn-vue](https://www.shadcn-vue.com/) |
| Audio | [cpal](https://github.com/RustAudio/cpal) + [hound](https://github.com/ruuda/hound) (recording), [rustfft](https://github.com/ejmahler/RustFFT) (spectrum) |
| Hotkey | Raw [CGEvent](https://developer.apple.com/documentation/coregraphics/cgevent) tap ([CoreGraphics](https://developer.apple.com/documentation/coregraphics) FFI) |
| Permissions | [objc2](https://github.com/madsmtm/objc2) ([AVFoundation](https://developer.apple.com/documentation/avfoundation), [CoreGraphics](https://developer.apple.com/documentation/coregraphics), [ApplicationServices](https://developer.apple.com/documentation/applicationservices)) |
| i18n | [vue-i18n](https://vue-i18n.intlify.dev/) |

## Project structure

```
src/                     Vue frontend
  views/                 Pages (Settings, ModelManager, FloatingPill, SetupWizard)
  components/            UI components
  stores/app.ts          Pinia store
  i18n/                  Translations (en.json, fr.json)
src-tauri/               Rust backend
  src/
    lib.rs               Tauri setup & app lifecycle
    recording.rs         Recording state machine & audio thread
    transcriber.rs       Transcription orchestration
    post_processor.rs    Text post-processing
    llm_cleanup.rs       LLM text cleanup client
    commands.rs          Tauri IPC commands
    state.rs             App state & persistent preferences
    tray.rs              Menu bar menu
    audio.rs             cpal recording & FFT
    engines/             Speech recognition engine adapters
    platform/            OS-specific code (permissions, hotkey, paste)
build.sh                 Build + codesign + package script
```

## License

Private project.
