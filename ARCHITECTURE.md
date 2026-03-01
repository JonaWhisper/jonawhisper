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
│                        vad.rs (silence filter)       │
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
│  views/  Settings · SetupWizard · History · …      │
│  components/  ShortcutCapture · SpectrumBars · …    │
└──────────────────────────────────────────────────────┘
```

## Backend (`src-tauri/src/`)

### Core

| File | Role |
|------|------|
| `lib.rs` | App setup: registers commands, spawns threads, manages the `monitor_enabled` flag |
| `commands.rs` | All `#[tauri::command]` handlers — thin wrappers that delegate to other modules. Includes `fetch_provider_models` for dynamic model discovery from cloud APIs. |
| `state.rs` | `AppState` with fine-grained mutexes: runtime state, download state (`HashMap<String, ActiveDownload>` for parallel per-model downloads), preferences, history DB (SQLite WAL with additive migrations for `cleanup_model_id`, `hallucination_filter`, and `vad_trimmed` columns), tray menu state, cached WhisperContext, cached BertContext, cached LlmContext. History queries are paginated (`LIMIT`/`OFFSET`) with optional `LIKE` search — `get_history(query, limit, offset)` and `history_count(query)`. `Provider` struct includes `cached_models: Vec<String>`. `ProviderKind::base_url()` resolves canonical API URLs at runtime for known providers (Custom uses stored URL). |
| `migrations.rs` | Versioned preference migrations. Each migration is a numbered function receiving raw JSON + typed `Preferences`. Runs on startup if `_version` < current. v1: legacy format unification (api_servers, llm_config, cleanup_mode). v2: model file relocation to `~/Library/Application Support/WhisperDictate/models/`. v3: update llm_max_tokens default from 256 to 4096 (autoscale hard cap). |
| `recording.rs` | Recording lifecycle (start → stop → enqueue → VAD check → transcribe → paste) and all background thread spawning. VAD pre-check discards silent recordings and trims silence before transcription; `vad_trimmed` bool threaded through to history and frontend event. Cancel support during both recording (discard audio) and transcription (cancel flag checked before paste). LLM cleanup uses autoscale max_tokens (capped by user hard cap, default 4096) with fallback to raw text on any error. Success pill close uses `PILL_CLOSE_GENERATION` guard (same as error path) to prevent race conditions when re-recording immediately. |
| `events.rs` | Centralised event name constants to avoid string typos |
| `errors.rs` | App error types (`AppError` enum with `thiserror` derivations) |

### Platform (`platform/`)

macOS-specific code behind `#[cfg(target_os = "macos")]`, with stubs for future Windows support.

| File | Role |
|------|------|
| `hotkey.rs` | Global shortcut detection via a CGEvent tap running on its own thread with a CFRunLoop. Supports three shortcut kinds: **ModifierOnly** (e.g. Right ⌘), **Combo** (e.g. ⌘R), **Key** (e.g. F13). Also implements a capture mode for the "press to record" shortcut picker. |
| `macos.rs` | Permission checks and requests (microphone via objc2/AVFoundation, input monitoring via CGEventTap probe, accessibility via AXIsProcessTrusted) |
| `ffi.rs` | Raw C declarations for CoreGraphics and CoreFoundation |
| `paste.rs` | Writes to the clipboard (via tauri-plugin-clipboard-manager) then simulates Cmd+V with CGEvent |
| `audio_devices.rs` | CoreAudio device enumeration and transport type detection |
| `audio_ducking.rs` | CoreAudio volume ducking — saves output volume, reduces it during recording (`duck_volume`), restores on stop (`restore_volume`). Uses `kAudioDevicePropertyVolumeScalar` on the default output device. |

### Engines (`engines/`)

Each speech engine implements the `ASREngine` trait (with a `recommended_model_id(language)` method for per-language recommendations).

| File | Engine |
|------|--------|
| `whisper.rs` | Native Whisper via [whisper-rs](https://github.com/tazz4843/whisper-rs) with Metal GPU acceleration on macOS. Models are GGML format, downloaded and cached locally. Context is kept in `AppState.whisper_context` to avoid reloading. |
| `openai_api.rs` | Any OpenAI-compatible API (reqwest HTTP) — works with OpenAI, local servers, etc. |
| `downloader.rs` | Model downloads: streaming HTTP with resume (Range headers), per-model state tracking, 250ms-throttled progress events with speed, HuggingFace repos, ZIP extraction |

The `EngineCatalog` in `mod.rs` aggregates all engines and provides model lookup, language listing, availability checks, and recommended model selection per language.

### Audio & transcription

| File | Role |
|------|------|
| `vad.rs` | Voice Activity Detection using Silero VAD v6 ONNX model (~2.3 MB, embedded via `include_bytes!`). Runs inference via `ort` directly (no VAD crate — ndarray version conflicts). Provides `has_speech()` (chunk-by-chunk probability check) and `trim_silence()` (find first/last speech segments with margin). Fallback: always proceeds on error (never loses dictation). |
| `audio.rs` | `AudioRecorder` — cpal input stream, WAV output via hound, 12-band FFT spectrum. Owns the cpal stream (not Send), so it lives on a dedicated thread. |
| `transcriber.rs` | Thin dispatcher: routes to cloud ASR API (`openai_api::transcribe`) if `selected_model_id` starts with `cloud:`, otherwise calls native `whisper::transcribe_native`. Runs on `spawn_blocking`. |
| `post_processor.rs` | Regex-based text cleanup: hallucination filtering, dictation commands, finalize (punctuation spacing, capitalization) |
| `bert_punctuation.rs` | BERT punctuation restoration via ONNX Runtime (`ort`). Cached `BertContext` in `AppState`. Sentence-level batching with tokenizer. |
| `llm_cleanup.rs` | Cloud LLM text cleanup via OpenAI or Anthropic API with 30s HTTP timeout. Uses `Provider::base_url()` for URL resolution. On any error, recording.rs falls back to raw transcription. Uses shared `LlmError` and `sanitize_output` from `llm_prompt`. |
| `llm_local.rs` | Local LLM text cleanup via llama.cpp (`llama-cpp-2`). Cached `LlmContext` (backend + model), Metal GPU offload, configurable `max_tokens`. Uses shared `LlmError` and `sanitize_output` from `llm_prompt`. |
| `llm_prompt.rs` | Shared LLM module: `LlmError` enum (unified error type for local and cloud), `sanitize_output` (think-block stripping + sanity check), and system prompt template. |

### Tray & icons

| File | Role |
|------|------|
| `tray.rs` | System tray icon, context menu (localized with `rust-i18n`), pill window lifecycle (delegates to `pill.rs`). Tray bar icons (idle/recording/transcribing) are rendered as 44×44 SDF bitmaps using primitives from `menu_icons`. |
| `pill.rs` | **Native pill overlay** — pure AppKit NSWindow with NSImageView, no WebView. Renders the pill as an RGBA bitmap using SDF primitives (spectrum bars, bouncing dots, error X, queue badge with 3×5 bitmap font). Animation at ~30fps via background thread + main thread rendering. First frame rendered before window is shown — zero flash. |
| `menu_icons.rs` | SDF (Signed Distance Field) icon rendering — shared primitives (`sdf_aa`, `sdf_rrect`, `sdf_circle`, `sdf_segment`, `point_in_triangle`) used by both tray icons and the native pill. Also provides 8 transport type icons (laptop, USB, bluetooth, waves, hard drive, zap, monitor, mic). Icons are rendered at 16×16, cached in a `LazyLock`, and composited onto 36×36 colored bubbles (blue=selected, gray=other) for menu items. Inspired by [Lucide](https://lucide.dev/) icon paths, hand-crafted as SDF shapes — zero image dependencies. |

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
| `Settings.vue` | `/settings` | Settings panel with sidebar navigation (general, providers, transcription, post-processing, shortcuts, microphone). Unified model selectors for ASR (local + cloud) and text cleanup (BERT + LLM). Refresh buttons to re-fetch models from cloud providers. Token hard cap slider (128–8192). |
| `ModelManager.vue` | `/model-manager` | Engine and model management with download progress |
| `History.vue` | `/history` | Transcription history timeline with backend-driven search (SQLite LIKE) and infinite scroll pagination. Processing badges with shadcn-vue tooltips: ASR local/cloud, language, cleanup method, hallucination filter, VAD trimmed. |

### Key components

| Component | Description |
|-----------|-------------|
| `ShortcutCapture.vue` | Press-to-record shortcut input. Invokes `start_shortcut_capture` on the backend, listens for `shortcut-capture-update` and `shortcut-capture-complete` events. |
| `SpectrumBars.vue` | Reusable audio spectrum visualization (used in mic test) |
| `SetupStep2.vue` | Initial configuration form (hotkey, recording mode, model, language) — embedded in SetupWizard |
| `ModelCell.vue` | Autonomous model list item — reads download/delete state directly from store, shows progress bar with speed, pause/resume/cancel actions, and delete indicator (greyed trash with indeterminate bar) |
| `BenchmarkBadges.vue` | WER/RTF benchmark colored badges (shadcn Badge) with quality/speed tiers |
| `ProviderForm.vue` | Reusable form for configuring cloud providers (9 presets + custom). Includes a Test button that validates the API key and fetches available models (`fetch_provider_models`). Fetched models are cached on the Provider and used in Settings dropdowns. Error feedback in a styled alert box (border + icon). |

### State management

Pinia stores are split by domain: `app.ts` (runtime state + event listeners), `history.ts` (paginated history with backend-driven search and infinite scroll), `settings.ts`, `engines.ts`, `downloads.ts`. Download state uses an `activeDownloads` map (model ID → progress/speed/stopping) enabling parallel downloads. Pause transitions use optimistic updates (immediate state swap, no async gap) to avoid visual flash.

### Utilities

`utils/shortcut.ts` mirrors the Rust `Shortcut` type: parse, format, serialize, and a key code → label table matching the backend.

`utils/format.ts` provides unified byte formatting (`formatBytes`, `formatSize`, `formatSpeed`) used for model sizes and download speeds.

## Threading model

The app uses six long-lived threads:

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
  │     feeds pill directly. Also detects audio stream errors.
  │
  ├── Pill animation thread (pill.rs)
  │     ~30fps loop while pill is open. Smooths spectrum, advances dot phase,
  │     dispatches rendering to main thread via run_on_main_thread.
  │
  └── Tokio async tasks
        Transcription (spawn_blocking), model downloads, LLM cleanup.
```

## Data flow: recording to paste

1. **Hotkey press** → CGEvent callback detects the configured shortcut → sends `HotkeyEvent::KeyDown`
2. **Hotkey handler** receives the event → calls `start_recording()`
3. **Recording starts** → if audio ducking enabled, saves and reduces system volume → sends `AudioCmd::StartRecording` to the audio thread → cpal stream begins capturing
4. **Pill opens** — native NSWindow with first frame pre-rendered (no flash), shows "recording" mode (spectrum bars)
5. **Hotkey release** → `HotkeyEvent::KeyUp` → `stop_recording_and_enqueue()`
6. **Audio stops** → system volume restored → WAV file path returned → enqueued in `RuntimeState.queue`
7. **VAD check** → if `vad_enabled`, reads the WAV and runs Silero VAD: if no speech detected → plays "Basso" sound, discards file, skips to next queue item. If speech found → trims leading/trailing silence and rewrites the WAV.
8. **Transcription** → `transcriber::transcribe()` on a blocking thread
9. **Post-processing** → preprocess (hallucination filter, dictation commands), then optional text cleanup (BERT punctuation / local LLM / cloud LLM with autoscale max_tokens), then finalize (spacing, capitalization). On any LLM error, falls back to raw transcription.
10. **Cancel check** → `transcription_cancelled` flag verified before paste — if cancel arrived during cleanup, text is discarded
11. **Paste** → text written to clipboard → Cmd+V simulated via CGEvent
12. **History** → entry saved to SQLite (with cleanup_model_id, hallucination_filter, vad_trimmed metadata) → frontend notified via event

**Cancel flow:** Escape during recording → `cancel_recording()` stops audio, discards file, shows error cross. Escape during transcription → `cancel_transcription()` sets cancel flag, clears queue. Both error and success delayed pill closes use a generation counter (`PILL_CLOSE_GENERATION`) to prevent stale closes from interfering with new recordings.

## Configuration

Preferences are stored as JSON in `~/Library/Application Support/WhisperDictate/preferences.json` with a `_version` field tracking the schema version. History lives in `history.db` (SQLite, WAL mode) in the same directory. All model files are stored under `~/Library/Application Support/WhisperDictate/models/` with subdirectories per engine (`whisper/`, `llm/`, `bert/`).

On startup, `migrations.rs` checks `_version` and runs any pending migrations sequentially. Each migration receives both the raw JSON (to access removed fields) and the typed `Preferences` struct. Migrations can also perform filesystem operations (e.g. relocating model files). To add a migration: append to the `MIGRATIONS` array and bump `CURRENT_VERSION`.

Shortcut values are stored as JSON objects (`{"key_code":54,"modifiers":1048576,"kind":"ModifierOnly"}`). Legacy string values (`"right_command"`, `"escape"`) are automatically parsed for backward compatibility.
