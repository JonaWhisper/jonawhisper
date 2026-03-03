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
│  platform/hotkey ──event──▶ recording ──cmd──▶ audio │
│                                │                     │
│                          cleanup/vad (silence filter) │
│                                │                     │
│                            asr/mod                   │
│                      ┌─────────┴────────┐            │
│                  asr/*            cleanup/*           │
│                  engines/*        (punct/t5/llm)     │
│                                │                     │
│                      platform/paste ──▶ Cmd+V        │
│                                │                     │
│                          state ──▶ SQLite + prefs    │
└────────────┬─────────────────────────────────────────┘
             │  Tauri commands ↑↓ events
┌────────────▼─────────────────────────────────────────┐
│                   Vue 3 Frontend                     │
│                                                      │
│  stores/  app · history · settings · engines · …     │
│  views/   Panel · SetupWizard                        │
│  sections/  Recents · Models · Transcription · …     │
│  components/  ShortcutCapture · SpectrumBars · …     │
└──────────────────────────────────────────────────────┘
```

## Backend (`src-tauri/src/`)

### Core

| File | Role |
|------|------|
| `lib.rs` | App setup: registers commands, spawns threads, manages the `monitor_enabled` flag. 12 modules: asr, audio, cleanup, commands, engines, errors, events, migrations, platform, recording, state, ui. |
| `commands.rs` | All `#[tauri::command]` handlers — thin wrappers that delegate to other modules. Includes `fetch_provider_models` for dynamic model discovery from cloud APIs. |
| `state.rs` | `AppState` with fine-grained mutexes via `context_group!` macro: runtime state, download state, preferences, history DB (SQLite WAL), tray menu state, cached contexts (Whisper, Canary, Parakeet, Qwen, Voxtral, BERT, Candle, PCS, T5, LLM). History queries are paginated (`LIMIT`/`OFFSET`) with optional `LIKE` search. `ProviderKind::base_url()` resolves canonical API URLs at runtime. |
| `recording.rs` | Recording lifecycle (start → stop → enqueue → VAD → transcribe → cleanup → paste) and background thread spawning. Threads `vad_trimmed` to history. Cancel support during recording and transcription. Uses `PILL_CLOSE_GENERATION` guard to prevent stale pill closes. |
| `migrations.rs` | Versioned preference migrations (numbered functions, raw JSON + typed `Preferences`). Runs on startup if `_version` < current. Can also perform filesystem operations (e.g. relocating model files). |
| `audio.rs` | `AudioRecorder` — cpal input stream, WAV output via hound, 12-band FFT spectrum. Owns the cpal stream (not Send), so it lives on a dedicated thread. Also provides `read_wav_f32()` shared WAV reader. |
| `events.rs` | Centralised event name constants to avoid string typos |
| `errors.rs` | App error types (`AppError` enum with `thiserror` derivations) |

### ASR (`asr/`)

Speech recognition inference modules. Each engine handles model loading, context caching, and transcription.

| File | Role |
|------|------|
| `asr/mod.rs` | Transcription dispatcher: routes to cloud API or native engine (Whisper, Canary, Parakeet, Qwen, Voxtral) based on `selected_model_id` prefix. Runs on `spawn_blocking`. |
| `asr/whisper.rs` | WhisperCtx + transcribe_native (whisper-rs, GGML, Metal GPU) |
| `asr/canary.rs` | CanaryContext + transcribe (NVIDIA Canary 182M, ONNX encoder-decoder, CoreML) |
| `asr/parakeet.rs` | ParakeetContext + transcribe (NVIDIA Parakeet-TDT 0.6B, vendored TDT decoder, ONNX + CoreML). Frame-by-frame LSTM transducer with duration head. |
| `asr/qwen.rs` | QwenContext + transcribe (Alibaba Qwen3-ASR 0.6B, qwen-asr crate, Accelerate/AMX) |
| `asr/voxtral.rs` | VoxtralContext + transcribe (Mistral Voxtral 4B, vendored voxtral.c FFI, Metal GPU) |
| `asr/mel.rs` | Mel feature extraction with configurable `MelConfig` (HTK/Slaney scales, pre-emphasis, Bessel correction). Presets for Canary and Parakeet. |

### Cleanup (`cleanup/`)

Text post-processing pipeline: VAD, punctuation, correction, LLM cleanup.

| File | Role |
|------|------|
| `cleanup/mod.rs` | Re-exports (BertContext, CandlePunctContext, PcsContext, T5Context, LlmContext, LlmError) |
| `cleanup/vad.rs` | Voice Activity Detection using Silero VAD v6 ONNX model (~2.3 MB, embedded via `include_bytes!`). Runs inference via `ort` directly. Provides `has_speech()` and `trim_silence()`. Fallback: always proceeds on error. |
| `cleanup/common.rs` | Shared punctuation logic: labels, windowed inference (`restore_punctuation_windowed`), `strip_and_split`, `download_file`. Used by BERT, Candle, and PCS modules. |
| `cleanup/bert.rs` | BERT punctuation restoration via ONNX Runtime. Cached `BertContext` in `AppState`. Delegates windowing to `common`. |
| `cleanup/candle.rs` | BERT punctuation restoration via Candle (safetensors, Metal GPU). `XLMRobertaForTokenClassification` built from base model + Linear head. Delegates windowing to `common`. |
| `cleanup/pcs.rs` | PCS punctuation + capitalization + segmentation (47 languages) via ONNX Runtime. SentencePiece Unigram tokenizer parsed from protobuf via `prost`, cached as `tokenizer.json`. 4-head model, sliding window (128 tokens, 16 overlap). |
| `cleanup/t5.rs` | T5 encoder-decoder text correction via Candle (Metal GPU). Autoregressive decoding with KV cache, repeat penalty (1.1), temperature (0.1). 4 models: GEC T5 Small (60M), T5 Spell FR (220M), FlanEC Large (250M), Flan-T5 Grammar (783M). |
| `cleanup/post_processor.rs` | Regex-based text cleanup: hallucination filtering, dictation commands, finalize (spacing, capitalization) |
| `cleanup/llm_cloud.rs` | Cloud LLM text cleanup via OpenAI or Anthropic API (30s timeout). Falls back to raw transcription on error. |
| `cleanup/llm_local.rs` | Local LLM text cleanup via llama.cpp with Metal GPU offload. Cached `LlmContext` in `AppState`. |
| `cleanup/llm_prompt.rs` | Shared LLM module: `LlmError` enum, `sanitize_output` (think-block stripping), system prompt template |

### UI (`ui/`)

Native UI elements rendered in pure Rust (no WebView).

| File | Role |
|------|------|
| `ui/tray.rs` | System tray icon, context menu (localized via `rust-i18n`), pill window lifecycle. Tray bar icons rendered as 44×44 SDF bitmaps. |
| `ui/pill.rs` | Native pill overlay — pure AppKit NSWindow + NSImageView, no WebView. RGBA bitmap rendered with SDF primitives (spectrum bars, bouncing dots, error X, queue badge). ~30fps animation, first frame pre-rendered (zero flash). |
| `ui/menu_icons.rs` | SDF icon rendering — shared primitives used by both tray icons and pill. Also provides transport type icons (laptop, USB, bluetooth, etc.) composited onto colored bubbles for menu items. |

### Engines (`engines/`)

Engine catalog and registration. No inference logic — just model metadata, download URLs, and trait implementations.

Each speech engine implements the `ASREngine` trait (with `recommended_model_id(language)` for per-language recommendations).

| File | Role |
|------|------|
| `engines/mod.rs` | `ASREngine` trait, `EngineCatalog`, `CleanupKind`, `resolve_model()`. Aggregates all engines and provides model lookup, language listing, availability checks. |
| `engines/whisper.rs` | Whisper model catalog (tiny → large-v3-turbo) |
| `engines/canary.rs` | Canary model catalog |
| `engines/parakeet.rs` | Parakeet-TDT model catalog |
| `engines/qwen.rs` | Qwen3-ASR model catalog |
| `engines/voxtral.rs` | Voxtral model catalog |
| `engines/openai_api.rs` | OpenAI-compatible cloud API engine |
| `engines/bert.rs` | BERT punctuation model catalog |
| `engines/pcs.rs` | PCS punctuation model catalog |
| `engines/correction.rs` | T5 correction model catalog |
| `engines/llama.rs` | Local LLM (llama.cpp) model catalog |
| `engines/ort_session.rs` | Shared `build_session()` helper — adds CoreML EP to all ort sessions (Canary, Parakeet, BERT, PCS). Automatic dispatch to Metal GPU or Apple Neural Engine. |
| `engines/downloader.rs` | Streaming HTTP downloads with resume (Range headers), per-model state, 250ms-throttled progress events, HuggingFace repos, ZIP extraction |

### Platform (`platform/`)

macOS-specific code behind `#[cfg(target_os = "macos")]`, with stubs for future Windows support.

| File | Role |
|------|------|
| `platform/hotkey.rs` | Global shortcut via CGEvent tap on its own CFRunLoop thread. Multi-key support: accumulates up to 4 keys during capture, finalizes on first key release. Three shortcut kinds: **ModifierOnly** (e.g. Right ⌘), **Combo** (e.g. ⌘+A), **Key** (e.g. F13). Lock-free packed atomics (`4×u16` in `AtomicU64`) for capture state. Also implements capture mode for the shortcut picker. |
| `platform/macos.rs` | Permission checks and requests (microphone via objc2/AVFoundation, input monitoring via CGEventTap probe, accessibility via AXIsProcessTrusted) |
| `platform/ffi.rs` | Raw C declarations for CoreGraphics and CoreFoundation |
| `platform/paste.rs` | Writes to clipboard (tauri-plugin-clipboard-manager) then simulates Cmd+V via CGEvent |
| `platform/audio_devices.rs` | CoreAudio device enumeration and transport type detection |
| `platform/audio_ducking.rs` | CoreAudio volume ducking — saves/reduces system volume during recording, restores on stop |

### SDF icon rendering

All icons are rendered at runtime in pure Rust using **Signed Distance Field** (SDF) functions — no image files, no external dependencies. Each icon is described by geometric primitives (rounded rectangles, circles, line segments, triangles). For each pixel, distance to each shape is computed with anti-aliased smoothstep (`sdf_aa`) to produce a smooth alpha value.

| Category | Size | Location |
|----------|------|----------|
| Tray bar icons | 44×44 (22pt @2x) | Menu bar status (idle, recording, transcribing) |
| Transport icons | 16×16 → 36×36 bubble | Device submenu items |

Transport icons are cached in a `LazyLock` and composited onto colored bubbles (blue = selected, gray = other). To add an icon: find the reference on [Lucide](https://lucide.dev/), scale from 24×24 to 16×16, express as SDF primitives in `ui/menu_icons.rs`.

## Frontend (`src/`)

### Views

| View | Route | Description |
|------|-------|-------------|
| `Panel.vue` | `/panel` | Main settings panel with sidebar navigation. Hosts all section components. |
| `SetupWizard.vue` | `/setup` | Two-step wizard: permissions, then initial configuration |

### Sections (`sections/`)

| Section | Description |
|---------|-------------|
| `RecentsSection.vue` | Transcription history grouped by day with backend-driven search (SQLite LIKE) and infinite scroll. Processing badges with tooltips. Copy toast animation. |
| `ModelsSection.vue` | Engine and model management with download progress |
| `TranscriptionSection.vue` | ASR model selector (local + cloud unified), cloud sub-model picker, language, GPU mode with "Recommended" badge |
| `ProcessingSection.vue` | Post-processing: VAD, hallucination filter, text cleanup (BERT/PCS/T5/LLM unified selector), LLM token cap |
| `ShortcutsSection.vue` | Hotkey, recording mode (push-to-talk / toggle), cancel shortcut |
| `MicrophoneSection.vue` | Input device selector with transport type icons, mic test with spectrum + level badge, audio ducking |
| `ProvidersSection.vue` | Cloud provider management (9 presets + custom) |
| `PermissionsSection.vue` | macOS permission status (microphone, accessibility, input monitoring) with grant buttons |
| `GeneralSection.vue` | Appearance (theme), interface language |

### Key components

| Component | Description |
|-----------|-------------|
| `ShortcutCapture.vue` | Press-to-record multi-key shortcut input. Invokes capture mode on the backend, listens for capture events. Displays key caps with localized side labels (Droit/Gauche). |
| `SpectrumBars.vue` | Audio spectrum visualization (dB scale, used in mic test and pill) |
| `SegmentedToggle.vue` | Segmented button group with optional badge support |
| `ModelOption.vue` | Model selector item with label + local/cloud badge |
| `CloudModelPicker.vue` | Cloud model dropdown with custom input and refresh |
| `SetupStep2.vue` | Initial configuration form (hotkey, mode, model, language) — embedded in SetupWizard |
| `ModelCell.vue` | Autonomous model list item — shows progress bar with speed, pause/resume/cancel/delete actions |
| `DownloadActions.vue` | Download action buttons (pause/resume/cancel) with shadcn-vue tooltips |
| `BenchmarkBadges.vue` | WER/RTF benchmark colored badges with quality/speed tiers |
| `TypeBadge.vue` | Colored badge for model type (ASR, punctuation, correction, LLM) |
| `ProviderForm.vue` | Cloud provider configuration (9 presets + custom). Test button validates API key and fetches models. |
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
| `utils/shortcut.ts` | Mirrors the Rust `Shortcut` type: `ShortcutDef` with `key_codes: number[]`, parse (with backward compat for old `key_code` singular format), format key caps with side labels, serialize |
| `utils/format.ts` | Byte formatting (`formatBytes`, `formatSize`, `formatSpeed`) for model sizes and download speeds |

## Threading model

The app uses six long-lived threads:

```
Main thread (Tauri + Tokio runtime)
  │
  ├── CGEvent tap thread (platform/hotkey.rs)
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
  ├── Pill animation thread (ui/pill.rs)
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
7. **VAD check** → if `vad_enabled`, runs Silero VAD (`cleanup/vad.rs`): no speech → plays "Basso" sound, discards file. Speech found → trims leading/trailing silence, rewrites WAV.
8. **Transcription** → `asr::transcribe()` on a blocking thread
9. **Post-processing** → hallucination filter, dictation commands, optional text cleanup (BERT / PCS / T5 correction / local LLM / cloud LLM with autoscale max_tokens), finalize (spacing, capitalization). On any cleanup error, falls back to raw transcription.
10. **Cancel check** → `transcription_cancelled` flag verified before paste
11. **Paste** → text written to clipboard → Cmd+V simulated via CGEvent
12. **History** → entry saved to SQLite (with cleanup_model_id, hallucination_filter, vad_trimmed metadata) → frontend notified via event

**Cancel flow:** Escape during recording → stops audio, discards file, shows error cross. Escape during transcription → sets cancel flag, clears queue. Both error and success pill closes use `PILL_CLOSE_GENERATION` to prevent stale closes from interfering with new recordings.

## Configuration

Preferences are stored as JSON in `~/Library/Application Support/WhisperDictate/preferences.json` with a `_version` field tracking the schema version. History lives in `history.db` (SQLite, WAL mode) in the same directory. All model files are stored under `models/` with subdirectories per engine (`whisper/`, `canary/`, `parakeet/`, `qwen-asr/`, `voxtral/`, `llm/`, `bert/`, `pcs/`, `correction/`).

On startup, `migrations.rs` checks `_version` and runs any pending migrations sequentially. Each migration receives both the raw JSON and the typed `Preferences` struct. To add a migration: append to the `MIGRATIONS` array and bump `CURRENT_VERSION`.

Shortcut values are stored as JSON objects (`{"key_codes":[54],"modifiers":1048576,"kind":"ModifierOnly"}`). Multi-key shortcuts store multiple key codes (up to 4). Legacy formats are automatically parsed for backward compatibility: old JSON (`key_code` singular) and legacy strings (`"right_command"`).
