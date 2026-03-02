# Architecture

WhisperDictate is a Tauri v2 app: a Rust backend paired with a Vue 3 frontend rendered in a native webview. It runs as a menu bar icon (no dock presence) and communicates between layers via Tauri commands (frontend → backend) and events (backend → frontend).

## High-level overview

```
┌──────────────────────────────────────────────────────┐
│                     macOS System                     │
│  CGEvent tap (hotkey)  ·  CoreAudio (mic)  ·  TCC    │
└────────────┬──────────────────┬──────────────────────┘
             │                  │
┌────────────▼──────────────────▼──────────────────────┐
│                   Rust Backend                       │
│                                                      │
│  hotkey.rs ──event──▶ recording.rs ──cmd──▶ audio.rs │
│                           │                          │
│                        vad.rs (silence filter)       │
│                           │                          │
│                     transcriber.rs                   │
│                      ┌────┴────┐                     │
│                engines/*   post_processor.rs         │
│                             llm_cleanup.rs           │
│                           │                          │
│                      paste.rs ──▶ clipboard + Cmd+V  │
│                           │                          │
│                      state.rs ──▶ SQLite + prefs     │
└────────────┬─────────────────────────────────────────┘
             │  Tauri commands ↑↓ events
┌────────────▼─────────────────────────────────────────┐
│                   Vue 3 Frontend                     │
│                                                      │
│  stores/  app · history · settings · engines · …     │
│  views/   Settings · SetupWizard · History · …       │
│  components/  ShortcutCapture · SpectrumBars · …     │
└──────────────────────────────────────────────────────┘
```

## Backend (`src-tauri/src/`)

### Core

| File | Role |
|------|------|
| `lib.rs` | App setup: registers commands, spawns threads, manages the `monitor_enabled` flag |
| `commands.rs` | All `#[tauri::command]` handlers — thin wrappers that delegate to other modules. Includes `fetch_provider_models` for dynamic model discovery from cloud APIs. |
| `state.rs` | `AppState` with fine-grained mutexes: runtime state, download state, preferences, history DB (SQLite WAL), tray menu state, cached contexts (Whisper, BERT, Candle, PCS, T5, LLM). History queries are paginated (`LIMIT`/`OFFSET`) with optional `LIKE` search. `ProviderKind::base_url()` resolves canonical API URLs at runtime. |
| `migrations.rs` | Versioned preference migrations (numbered functions, raw JSON + typed `Preferences`). Runs on startup if `_version` < current. Can also perform filesystem operations (e.g. relocating model files). |
| `recording.rs` | Recording lifecycle (start → stop → enqueue → VAD → transcribe → paste) and background thread spawning. Threads `vad_trimmed` to history. Cancel support during recording and transcription. Uses `PILL_CLOSE_GENERATION` guard to prevent stale pill closes. |
| `events.rs` | Centralised event name constants to avoid string typos |
| `errors.rs` | App error types (`AppError` enum with `thiserror` derivations) |

### Platform (`platform/`)

macOS-specific code behind `#[cfg(target_os = "macos")]`, with stubs for future Windows support.

| File | Role |
|------|------|
| `hotkey.rs` | Global shortcut via CGEvent tap on its own CFRunLoop thread. Three shortcut kinds: **ModifierOnly** (e.g. Right ⌘), **Combo** (e.g. ⌘R), **Key** (e.g. F13). Also implements capture mode for the shortcut picker. |
| `macos.rs` | Permission checks and requests (microphone via objc2/AVFoundation, input monitoring via CGEventTap probe, accessibility via AXIsProcessTrusted) |
| `ffi.rs` | Raw C declarations for CoreGraphics and CoreFoundation |
| `paste.rs` | Writes to clipboard (tauri-plugin-clipboard-manager) then simulates Cmd+V via CGEvent |
| `audio_devices.rs` | CoreAudio device enumeration and transport type detection |
| `audio_ducking.rs` | CoreAudio volume ducking — saves/reduces system volume during recording, restores on stop |

### Engines (`engines/`)

Each speech engine implements the `ASREngine` trait (with `recommended_model_id(language)` for per-language recommendations).

| File | Role |
|------|------|
| `whisper.rs` | Native Whisper via whisper-rs with Metal GPU acceleration. GGML models, cached context in `AppState`. |
| `openai_api.rs` | Any OpenAI-compatible API (reqwest HTTP) — works with OpenAI, local servers, etc. |
| `downloader.rs` | Streaming HTTP downloads with resume (Range headers), per-model state, 250ms-throttled progress events, HuggingFace repos, ZIP extraction |

Additionally, `bert.rs` and `pcs.rs` provide punctuation-category engines (BERT and PCS), and `correction.rs` provides correction-category engines (T5 models). Punctuation and correction engines return empty `supported_languages()` to avoid polluting the ASR language selector — their language support is indicated via `lang_codes` on each model. BERT models declare a `runtime` field (`"ort"` for ONNX, `"candle"` for safetensors) so `recording.rs` can route to the right inference backend.

The `EngineCatalog` in `mod.rs` aggregates all engines and provides model lookup, language listing, availability checks, and recommended model selection per language.

### Audio

| File | Role |
|------|------|
| `audio.rs` | `AudioRecorder` — cpal input stream, WAV output via hound, 12-band FFT spectrum. Owns the cpal stream (not Send), so it lives on a dedicated thread. |
| `vad.rs` | Voice Activity Detection using Silero VAD v6 ONNX model (~2.3 MB, embedded via `include_bytes!`). Runs inference via `ort` directly. Provides `has_speech()` and `trim_silence()`. Fallback: always proceeds on error. |

### Transcription & text cleanup

| File | Role |
|------|------|
| `transcriber.rs` | Thin dispatcher: routes to cloud API or native Whisper based on `selected_model_id` prefix. Runs on `spawn_blocking`. |
| `post_processor.rs` | Regex-based text cleanup: hallucination filtering, dictation commands, finalize (spacing, capitalization) |
| `punct_common.rs` | Shared punctuation logic: labels, windowed inference (`restore_punctuation_windowed`), `strip_and_split`, `download_file`. Used by BERT, Candle, and PCS modules. |
| `bert_punctuation.rs` | BERT punctuation restoration via ONNX Runtime. Cached `BertContext` in `AppState`. Delegates windowing to `punct_common`. |
| `candle_punctuation.rs` | BERT punctuation restoration via Candle (safetensors, Metal GPU). `XLMRobertaForTokenClassification` built from base model + Linear head. Cached `CandlePunctContext` in `AppState`. Delegates windowing to `punct_common`. |
| `pcs_punctuation.rs` | PCS punctuation + capitalization + segmentation (47 languages) via ONNX Runtime. SentencePiece Unigram tokenizer parsed from protobuf (`.model`) via `prost`, built into `tokenizers::Tokenizer`, cached as `tokenizer.json`. 4-head model (pre/post punctuation, capitalization, segmentation). Sliding window (128 tokens, 16 overlap). Cached `PcsContext` in `AppState`. |
| `t5_correction.rs` | T5 encoder-decoder text correction via Candle (Metal GPU). Loads safetensors + config.json + tokenizer.json. Single-pass encoding then autoregressive decoding with KV cache, repeat penalty (1.1), temperature (0.1). Output sanitized (reject empty or >3x input). 4 models: GEC T5 Small (60M, multilingual grammar), T5 Spell FR (220M, French), FlanEC Large (250M, post-ASR), Flan-T5 Grammar (783M). Cached `T5Context` in `AppState`. |
| `llm_cleanup.rs` | Cloud LLM text cleanup via OpenAI or Anthropic API (30s timeout). Falls back to raw transcription on error. |
| `llm_local.rs` | Local LLM text cleanup via llama.cpp with Metal GPU offload. Cached `LlmContext` in `AppState`. |
| `llm_prompt.rs` | Shared LLM module: `LlmError` enum, `sanitize_output` (think-block stripping), system prompt template |

### Tray & pill

| File | Role |
|------|------|
| `tray.rs` | System tray icon, context menu (localized via `rust-i18n`), pill window lifecycle. Tray bar icons rendered as 44×44 SDF bitmaps. |
| `pill.rs` | Native pill overlay — pure AppKit NSWindow + NSImageView, no WebView. RGBA bitmap rendered with SDF primitives (spectrum bars, bouncing dots, error X, queue badge). ~30fps animation, first frame pre-rendered (zero flash). |
| `menu_icons.rs` | SDF icon rendering — shared primitives used by both tray icons and pill. Also provides transport type icons (laptop, USB, bluetooth, etc.) composited onto colored bubbles for menu items. Inspired by Lucide, hand-crafted as SDF shapes — zero image dependencies. |

### SDF icon rendering

All icons are rendered at runtime in pure Rust using **Signed Distance Field** (SDF) functions — no image files, no external dependencies. Each icon is described by geometric primitives (rounded rectangles, circles, line segments, triangles). For each pixel, distance to each shape is computed with anti-aliased smoothstep (`sdf_aa`) to produce a smooth alpha value.

| Category | Size | Location |
|----------|------|----------|
| Tray bar icons | 44×44 (22pt @2x) | Menu bar status (idle, recording, transcribing) |
| Transport icons | 16×16 → 36×36 bubble | Device submenu items |

Transport icons are cached in a `LazyLock` and composited onto colored bubbles (blue = selected, gray = other). To add an icon: find the reference on [Lucide](https://lucide.dev/), scale from 24×24 to 16×16, express as SDF primitives in `menu_icons.rs`.

## Frontend (`src/`)

### Views

| View | Route | Description |
|------|-------|-------------|
| `SetupWizard.vue` | `/setup` | Two-step wizard: permissions, then initial configuration |
| `Settings.vue` | `/settings` | Settings panel with sidebar navigation. Unified model selectors for ASR and text cleanup. Refresh buttons with tooltips to re-fetch cloud models. Token hard cap slider (128–8192). |
| `ModelManager.vue` | `/model-manager` | Engine and model management with download progress |
| `History.vue` | `/history` | Transcription history grouped by day with backend-driven search (SQLite LIKE) and infinite scroll. Processing badges with shadcn-vue tooltips (ASR, language, cleanup, hallucination filter, VAD). |

### Key components

| Component | Description |
|-----------|-------------|
| `ShortcutCapture.vue` | Press-to-record shortcut input. Invokes capture mode on the backend, listens for capture events. |
| `SpectrumBars.vue` | Audio spectrum visualization (used in mic test) |
| `SetupStep2.vue` | Initial configuration form (hotkey, mode, model, language) — embedded in SetupWizard |
| `ModelCell.vue` | Autonomous model list item — shows progress bar with speed, pause/resume/cancel/delete actions |
| `DownloadActions.vue` | Download action buttons (pause/resume/cancel) with shadcn-vue tooltips |
| `BenchmarkBadges.vue` | WER/RTF benchmark colored badges with quality/speed tiers |
| `ProviderForm.vue` | Cloud provider configuration (9 presets + custom). Test button validates API key and fetches models. Error feedback in a styled alert box. |
| `ConfirmDialog.vue` | Reusable confirmation dialog (used for history clear/delete) |

### State management

Pinia stores split by domain:

| Store | Role |
|-------|------|
| `app.ts` | Runtime state, event listeners, initialization |
| `history.ts` | Paginated history with backend-driven search and infinite scroll |
| `settings.ts` | User preferences |
| `engines.ts` | Engine catalog, providers, models |
| `downloads.ts` | Active downloads map (model ID → progress/speed/stopping), parallel downloads, optimistic pause transitions |

### Utilities

| File | Role |
|------|------|
| `utils/shortcut.ts` | Mirrors the Rust `Shortcut` type: parse, format, serialize, key code → label table |
| `utils/format.ts` | Byte formatting (`formatBytes`, `formatSize`, `formatSpeed`) for model sizes and download speeds |

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
4. **Pill opens** — native NSWindow with first frame pre-rendered (no flash), shows recording mode (spectrum bars)
5. **Hotkey release** → `HotkeyEvent::KeyUp` → `stop_recording_and_enqueue()`
6. **Audio stops** → system volume restored → WAV file path returned → enqueued in `RuntimeState.queue`
7. **VAD check** → if `vad_enabled`, runs Silero VAD: no speech → plays "Basso" sound, discards file. Speech found → trims leading/trailing silence, rewrites WAV.
8. **Transcription** → `transcriber::transcribe()` on a blocking thread
9. **Post-processing** → hallucination filter, dictation commands, optional text cleanup (BERT / PCS / T5 correction / local LLM / cloud LLM with autoscale max_tokens), finalize (spacing, capitalization). On any cleanup error, falls back to raw transcription.
10. **Cancel check** → `transcription_cancelled` flag verified before paste
11. **Paste** → text written to clipboard → Cmd+V simulated via CGEvent
12. **History** → entry saved to SQLite (with cleanup_model_id, hallucination_filter, vad_trimmed metadata) → frontend notified via event

**Cancel flow:** Escape during recording → stops audio, discards file, shows error cross. Escape during transcription → sets cancel flag, clears queue. Both error and success pill closes use `PILL_CLOSE_GENERATION` to prevent stale closes from interfering with new recordings.

## Configuration

Preferences are stored as JSON in `~/Library/Application Support/WhisperDictate/preferences.json` with a `_version` field tracking the schema version. History lives in `history.db` (SQLite, WAL mode) in the same directory. All model files are stored under `models/` with subdirectories per engine (`whisper/`, `canary/`, `parakeet/`, `qwen-asr/`, `llm/`, `bert/`, `pcs/`, `correction/`).

On startup, `migrations.rs` checks `_version` and runs any pending migrations sequentially. Each migration receives both the raw JSON and the typed `Preferences` struct. To add a migration: append to the `MIGRATIONS` array and bump `CURRENT_VERSION`.

Shortcut values are stored as JSON objects (`{"key_code":54,"modifiers":1048576,"kind":"ModifierOnly"}`). Legacy string values are automatically parsed for backward compatibility.
