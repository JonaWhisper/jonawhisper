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

| Permission | Used for |
|---|---|
| **Microphone** | Audio recording |
| **Accessibility** | Paste simulation (Cmd+V via CGEvent) |
| **Input Monitoring** | Global hotkey detection (CGEvent tap) |

## Engines

WhisperDictate supports multiple speech recognition backends. Install at least one:

| Engine | Install | Models | Languages |
|---|---|---|---|
| **Whisper** (whisper.cpp) | `brew install whisper-cpp` | Tiny to Large V3 (75 MB - 3.1 GB) | Auto, FR, EN, ES, DE |
| **Faster Whisper** | `pip install whisper-ctranslate2` | Tiny to Distil Large V3 | Auto, FR, EN, ES, DE |
| **MLX Whisper** (Apple Silicon) | `pip install mlx-whisper` | Tiny to Large V3 Q4 | Auto, FR, EN, ES, DE |
| **Vosk** | `pip install vosk` | Small/Large per language | EN, FR |
| **Moonshine** | `pip install useful-moonshine` | Tiny, Base | EN |
| **OpenAI API** | - | Custom (via API server) | Auto, FR, EN, ES, DE |

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
| Backend | Rust |
| Frontend | Vue 3, TypeScript, Pinia, Tailwind CSS, [shadcn-vue](https://www.shadcn-vue.com/) |
| Audio | cpal + hound (recording), rustfft (spectrum) |
| Hotkey | Raw CGEvent tap (CoreGraphics FFI) |
| Permissions | objc2 (AVFoundation), CoreGraphics, ApplicationServices |
| i18n | vue-i18n |

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
