# Architecture

WhisperDictate is a Tauri v2 app: a Rust backend paired with a Vue 3 frontend rendered in a native webview. It runs as a menu bar icon (no dock presence) and communicates between layers via Tauri commands (frontend → backend) and events (backend → frontend).

## High-level overview

```
┌─────────────────────────────────────────────────────┐
│                    macOS System                      │
│  CGEvent tap (hotkey)  ·  CoreAudio (mic)  ·  TCC   │
└────────────┬──────────────────┬──────────────────────┘
             │                  │
┌────────────▼──────────────────▼──────────────────────┐
│                  Rust Backend                         │
│                                                      │
│  hotkey.rs ──event──▶ recording.rs ──cmd──▶ audio.rs │
│                           │                          │
│                     transcriber.rs                    │
│                      ┌────┴────┐                     │
│                 engines/*   post_processor.rs         │
│                             llm_cleanup.rs           │
│                           │                          │
│                      paste.rs ──▶ clipboard + Cmd+V  │
│                           │                          │
│                      state.rs ──▶ SQLite + prefs     │
└────────────┬─────────────────────────────────────────┘
             │  Tauri commands ↑↓ events
┌────────────▼─────────────────────────────────────────┐
│                  Vue 3 Frontend                       │
│                                                      │
│  stores/app.ts  (Pinia — single source of truth)     │
│  views/  Settings · SetupWizard · FloatingPill · … │
│  components/  ShortcutCapture · SpectrumBars · …    │
└──────────────────────────────────────────────────────┘
```

## Backend (`src-tauri/src/`)

### Core

| File | Role |
|------|------|
| `lib.rs` | App setup: registers commands, spawns threads, manages the `monitor_enabled` flag |
| `commands.rs` | All `#[tauri::command]` handlers — thin wrappers that delegate to other modules |
| `state.rs` | `AppState` with four fine-grained mutexes: runtime state, download state, preferences, and history DB |
| `recording.rs` | Recording lifecycle (start → stop → enqueue → transcribe → paste) and all background thread spawning |
| `events.rs` | Centralised event name constants to avoid string typos |
| `errors.rs` | App error types (`AppError` enum with `thiserror` derivations) |
| `process_runner.rs` | Subprocess execution helper for speech engine CLIs |

### Platform (`platform/`)

macOS-specific code behind `#[cfg(target_os = "macos")]`, with stubs for future Windows support.

| File | Role |
|------|------|
| `hotkey.rs` | Global shortcut detection via a CGEvent tap running on its own thread with a CFRunLoop. Supports three shortcut kinds: **ModifierOnly** (e.g. Right ⌘), **Combo** (e.g. ⌘R), **Key** (e.g. F13). Also implements a capture mode for the "press to record" shortcut picker. |
| `macos.rs` | Permission checks and requests (microphone via objc2/AVFoundation, input monitoring via CGEventTap probe, accessibility via AXIsProcessTrusted) |
| `ffi.rs` | Raw C declarations for CoreGraphics and CoreFoundation |
| `paste.rs` | Writes to the clipboard (via tauri-plugin-clipboard-manager) then simulates Cmd+V with CGEvent |
| `audio_devices.rs` | CoreAudio device enumeration and transport type detection |

### Engines (`engines/`)

Each speech engine implements the `ASREngine` trait (with a `recommended_model_id(language)` method for per-language recommendations). Currently all engines run as subprocesses; native Rust bindings are planned.

| File | Engine |
|------|--------|
| `whisper.rs` | whisper.cpp (CLI) |
| `faster_whisper.rs` | Faster Whisper (Python CLI) |
| `mlx_whisper.rs` | MLX Whisper (Python, macOS only) |
| `vosk.rs` | Vosk (Python CLI) |
| `moonshine.rs` | Moonshine (Python CLI) |
| `openai_api.rs` | Any OpenAI-compatible API (reqwest HTTP) |
| `downloader.rs` | Model downloads: streaming HTTP, HuggingFace repos, ZIP extraction |

The `EngineCatalog` in `mod.rs` aggregates all engines and provides model lookup, language listing, availability checks, and recommended model selection per language.

### Audio & transcription

| File | Role |
|------|------|
| `audio.rs` | `AudioRecorder` — cpal input stream, WAV output via hound, 12-band FFT spectrum. Owns the cpal stream (not Send), so it lives on a dedicated thread. |
| `transcriber.rs` | Picks the right engine from the catalog and calls `engine.transcribe()`. Runs on `spawn_blocking`. |
| `post_processor.rs` | Regex-based text cleanup: hallucination filtering, dictation commands |
| `llm_cleanup.rs` | Optional LLM-based text cleanup via OpenAI or Anthropic API |

### Tray & icons

| File | Role |
|------|------|
| `tray.rs` | System tray icon, context menu (localized with `rust-i18n`), floating pill window lifecycle. Tray bar icons (idle/recording/transcribing) are rendered as 44×44 SDF bitmaps using primitives from `menu_icons`. |
| `menu_icons.rs` | SDF (Signed Distance Field) icon rendering — shared primitives (`sdf_aa`, `sdf_rrect`, `sdf_circle`, `sdf_segment`, `point_in_triangle`) and 8 transport type icons (laptop, USB, bluetooth, waves, hard drive, zap, monitor, mic). Icons are rendered at 16×16, cached in a `LazyLock`, and composited onto 36×36 colored bubbles (blue=selected, gray=other) for menu items. Inspired by [Lucide](https://lucide.dev/) icon paths, hand-crafted as SDF shapes — zero image dependencies. |

### SDF icon rendering — how it works

All tray bar and menu icons are rendered at runtime in pure Rust using **Signed Distance Field** (SDF) functions — no image files, no PNG assets, no external dependencies.

**How it works:** each icon is described by geometric primitives (rounded rectangles, circles, line segments, triangles). For each pixel, we compute the distance to each shape and use anti-aliasing smoothstep (`sdf_aa`) to produce a smooth alpha value. The result is a raw RGBA bitmap.

**Two categories of icons:**

| Category | Size | Where | Examples |
|----------|------|-------|---------|
| Tray bar icons | 44×44 (22pt @2x) | Menu bar status | `make_idle_icon()` (mic), `make_recording_icon()` (audio bars), `make_transcribing_icon()` (speech bubble) |
| Transport icons | 16×16 shape → 36×36 bubble | Device submenu items | laptop, USB, bluetooth, waves, hard drive, zap, monitor, mic |

**Transport icons** are composited onto colored bubbles: blue `(0,122,255)` = selected device, gray `(99,99,102)` = other devices. The 16×16 shapes are cached in a `LazyLock` and upsampled to 36×36 with bilinear interpolation.

**To add or modify an icon:**
1. Find the reference icon on [Lucide](https://lucide.dev/) (all current icons are inspired by Lucide v0.575)
2. Scale the SVG path coordinates from 24×24 to 16×16 (multiply by `16.0/24.0`)
3. Express the path as SDF primitives in a `render_*()` function in `menu_icons.rs`
4. Add it to the `ICON_SHAPES` array and update the `transport_icon()` match

**Tray bar icons** are in `tray.rs`, use the same SDF primitives from `menu_icons.rs`, and render at 44×44 (set as template images for automatic light/dark mode adaptation).

## Frontend (`src/`)

### Views

| View | Route | Description |
|------|-------|-------------|
| `SetupWizard.vue` | `/setup` | Two-step wizard: permissions, then initial configuration |
| `FloatingPill.vue` | `/pill` | Overlay showing recording/transcribing state with spectrum animation |
| `Settings.vue` | `/settings` | Settings panel with sidebar navigation (general, post-processing, shortcuts, microphone) |
| `ModelManager.vue` | `/model-manager` | Engine and model management with download progress |
| `History.vue` | `/history` | Transcription history timeline with search and deletion |

### Key components

| Component | Description |
|-----------|-------------|
| `ShortcutCapture.vue` | Press-to-record shortcut input. Invokes `start_shortcut_capture` on the backend, listens for `shortcut-capture-update` and `shortcut-capture-complete` events. |
| `SpectrumBars.vue` | Reusable audio spectrum visualization (used in pill and mic test) |
| `SetupStep2.vue` | Initial configuration form (locale, hotkey, model, language) — embedded in SetupWizard |
| `ModelCell.vue` | Model list item with download progress, actions, and benchmark display |
| `BenchmarkBadges.vue` | WER/RTF benchmark badges with visual indicators for model quality/speed |
| `ApiServerForm.vue` | Reusable form for configuring OpenAI-compatible API server endpoints |

### State management

`stores/app.ts` is the single Pinia store. It holds all reactive state, wraps every `invoke()` call with optimistic updates and rollback, and sets up Tauri event listeners on init.

### Utilities

`utils/shortcut.ts` mirrors the Rust `Shortcut` type: parse, format, serialize, and a key code → label table matching the backend.

## Threading model

The app uses five long-lived threads:

```
Main thread (Tauri + Tokio runtime)
  │
  ├── CGEvent tap thread (hotkey.rs)
  │     Runs a CFRunLoop, processes keyboard events, sends HotkeyEvent via channel.
  │     Also polls an update channel every 500ms for config changes.
  │
  ├── Hotkey handler thread (recording.rs)
  │     Reads HotkeyEvent from the channel, drives recording start/stop,
  │     forwards capture events to the frontend.
  │
  ├── Audio thread (recording.rs → audio.rs)
  │     Owns the cpal::Stream. Receives AudioCmd, replies with AudioReply.
  │
  ├── Spectrum emitter thread (recording.rs)
  │     30fps loop. Polls spectrum data from the audio thread,
  │     emits to frontend. Also detects audio stream errors.
  │
  └── Tokio async tasks
        Transcription (spawn_blocking), model downloads, LLM cleanup.
```

## Data flow: recording to paste

1. **Hotkey press** → CGEvent callback detects the configured shortcut → sends `HotkeyEvent::KeyDown`
2. **Hotkey handler** receives the event → calls `start_recording()`
3. **Recording starts** → sends `AudioCmd::StartRecording` to the audio thread → cpal stream begins capturing → pill window opens
4. **Hotkey release** → `HotkeyEvent::KeyUp` → `stop_recording_and_enqueue()`
5. **Audio stops** → WAV file path returned → enqueued in `RuntimeState.queue`
6. **Transcription** → `process_next_in_queue()` picks the file → `transcriber::transcribe()` on a blocking thread
7. **Post-processing** → regex cleanup, then optional LLM cleanup
8. **Paste** → text written to clipboard → Cmd+V simulated via CGEvent
9. **History** → entry saved to SQLite → frontend notified via event

## Configuration

Preferences are stored as JSON in `~/Library/Application Support/WhisperDictate/preferences.json`. History lives in `history.db` (SQLite, WAL mode) in the same directory.

Shortcut values are stored as JSON objects (`{"key_code":54,"modifiers":1048576,"kind":"ModifierOnly"}`). Legacy string values (`"right_command"`, `"escape"`) are automatically parsed for backward compatibility.
