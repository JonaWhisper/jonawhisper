# Architecture

JonaWhisper is a Tauri v2 app: a Rust backend paired with a Vue 3 frontend rendered in a native webview. It runs as a menu bar icon (no dock presence) and communicates between layers via Tauri commands (frontend → backend) and events (backend → frontend).

## Design philosophy: thin orchestrator + plug-and-play engines

The main Tauri crate (`src-tauri/src/`) is a **thin orchestrator**. It contains no engine logic, no model definitions, and no inference code. Its only job is to wire together independent workspace crates and dispatch work through a trait-based interface.

**Why this matters:**

- **Adding a new engine** = add a crate, implement the `ASREngine` trait, `inventory::submit!` it, add the `cargo` dependency. Zero changes to the orchestrator, zero changes to the frontend.
- **No re-export layers** — the main crate uses `jona_engines::`, `jona_platform::`, `jona_provider::` directly. No wrapper modules that just do `pub use other_crate::*`.
- **Engine isolation** — each engine crate owns its catalog (model list, sizes, URLs) and its inference code. They depend on `jona-types` for the trait and optionally on `jona-engines` for shared utilities (ort session builder, mel features, downloader).
- **Auto-registration** — engine crates register themselves at link time via `inventory::submit!`. At startup, `EngineCatalog::init_auto()` collects all registered engines. The orchestrator never enumerates engines by name. `build.rs` auto-generates `extern crate` declarations by scanning `Cargo.toml` for `jona-engine-*` dependencies, preventing the linker from eliminating unused crates.

```
┌──────────────────────────────────────────────────────────────┐
│                         macOS System                         │
│    CGEvent tap (hotkey)  ·  CoreAudio (mic)  ·  TCC perms    │
└──────────┬──────────────────────┬────────────────────────────┘
           │                      │
┌──────────▼──────────────────────▼────────────────────────────┐
│              Main crate (thin orchestrator)                   │
│                                                              │
│  platform/hotkey ──▶ recording/ ──▶ audio                    │
│                         │                                    │
│                    cleanup/vad                                │
│                         │                                    │
│                   ┌─────▼──────┐                             │
│                   │ ASREngine  │  ← trait from jona-types    │
│                   │  trait     │                              │
│                   └─────┬──────┘                             │
│         ┌───────┬───────┼───────┬───────┐                    │
│     whisper  canary  parakeet  qwen  voxtral  (+ 4 cleanup)  │
│     (independent crates, auto-registered via inventory)      │
│                         │                                    │
│                   platform/paste ──▶ Cmd+V                   │
│                         │                                    │
│                   state ──▶ SQLite + prefs                   │
└──────────┬───────────────────────────────────────────────────┘
           │  Tauri commands ↑↓ events
┌──────────▼───────────────────────────────────────────────────┐
│                      Vue 3 Frontend                          │
│                                                              │
│  stores/  app · history · settings · engines · downloads     │
│  views/   Panel · SetupWizard                                │
│  sections/  Recents · Models · Transcription · …             │
│  components/  ShortcutCapture · SpectrumBars · …             │
└──────────────────────────────────────────────────────────────┘
```

## Backend (`src-tauri/src/`)

### Core

| File | Role |
|------|------|
| `lib.rs` | App setup: registers commands, spawns threads, manages the `monitor_enabled` flag. 10 modules: audio, cleanup, commands, errors, events, migrations, platform, recording, state, ui. |
| `state.rs` | `AppState` with fine-grained mutexes via `context_group!` macro: runtime state, download state, preferences, history DB (SQLite WAL), tray menu state, cached contexts. History queries use cursor-based pagination (`WHERE timestamp < ?cursor … LIMIT`) with optional `LIKE` search. `ProviderKind::base_url()` resolves canonical API URLs at runtime. Keyring helpers (`keyring_store`, `keyring_load`, `keyring_delete`) manage API keys in the OS keychain. `Provider::validate_url()` enforces HTTPS on Custom providers (unless `allow_insecure`). |
| `migrations.rs` | Versioned preference migrations (numbered functions, raw JSON + typed `Preferences`). Runs on startup if `_version` < current. Can also perform filesystem operations (e.g. relocating model files). Current version: 7 (v4 migrates API keys to OS keychain, v6 separates punctuation from cleanup model, v7 cleans up old Candle/safetensors correction models). |
| `audio.rs` | `AudioRecorder` — cpal input stream, WAV output via hound, 12-band FFT spectrum. Owns the cpal stream (not Send), so it lives on a dedicated thread. Also provides `read_wav_f32()` shared WAV reader. |
| `events.rs` | Centralised event name constants to avoid string typos |
| `errors.rs` | App error types (`AppError` enum with `thiserror` derivations) |

### Recording (`recording/`)

Recording lifecycle and transcription pipeline, split into focused sub-modules.

| File | Role |
|------|------|
| `recording/mod.rs` | Public types (`AudioCmd`, `AudioReply`, `RecordingState`, `MicTestSender`), timing constants, shared helpers (`show_error_then_close`, `cleanup_orphan_audio_files`). |
| `recording/lifecycle.rs` | `start_recording`, `stop_recording_and_enqueue`, `handle_short_tap`, `cancel_recording`, `cancel_transcription`. Everything that touches the record button. |
| `recording/pipeline.rs` | `process_next_in_queue` → VAD pre-check → ASR dispatch (cloud or local via `ASREngine` trait) → hallucination filter → dictation commands → disfluency removal → punctuation → spell-check → correction/LLM → finalize → ITN → paste. Each step records its result in `pipeline_steps` (changed / `:nochange` / `:error`). All engine interactions go through `jona_engines::EngineCatalog`. |
| `recording/threads.rs` | Three long-lived threads: audio (cpal), hotkey handler, spectrum emitter (30fps). |

### Commands (`commands/`)

All `#[tauri::command]` handlers (33 total), split by domain. Each sub-module is independent.

| File | Handlers |
|------|----------|
| `commands/mod.rs` | Shared `catalog()` helper |
| `commands/audio.rs` | `get_audio_devices`, `start_mic_test`, `stop_mic_test` |
| `commands/engines.rs` | `get_engines`, `get_models`, `get_downloaded_models`, `download_model_cmd`, `delete_model_cmd`, `pause_download`, `cancel_download`, `get_languages` |
| `commands/history.rs` | `get_history`, `delete_history_entry`, `delete_history_day`, `clear_history` |
| `commands/providers.rs` | `add_provider`, `remove_provider`, `update_provider`, `get_providers`, `fetch_provider_models` |
| `commands/settings.rs` | `get_settings`, `set_setting`, `get_system_locale`, `get_launch_at_login_status`, `set_launch_at_login` |
| `commands/permissions.rs` | `get_permissions`, `request_permission`, `start_monitoring`, `enable_monitoring` |
| `commands/app.rs` | `get_app_state`, `start_shortcut_capture`, `stop_shortcut_capture`, `simulate_pill_test` |

### Cleanup (`cleanup/`)

Text post-processing pipeline: VAD, punctuation, correction, LLM cleanup.

| File | Role |
|------|------|
| `cleanup/mod.rs` | Re-exports (`LlmError` from `jona_engines::llm_prompt`) |
| `cleanup/vad.rs` | Voice Activity Detection using Silero VAD v6.2 ONNX model (~2.3 MB, embedded via `include_bytes!`). Runs inference via `ort` directly. Provides `has_speech()` and `trim_silence()`. Fallback: always proceeds on error. |
| `cleanup/post_processor/` | Regex-based text cleanup, split into sub-modules: `hallucinations.rs` (9-lang pattern matching + repetition detection), `dictation.rs` (FR/EN voice commands → punctuation), `fillers.rs` (9-lang disfluency removal), `mod.rs` (orchestration, finalize spacing/capitalization). |
| `cleanup/symspell_correct.rs` | Spell-check via SymSpell with downloadable frequency dictionaries (6 variants: fr, fr-be, fr-ca, fr-ch, en, en-gb). KenLM trigram reranking for context-aware correction. French guards (plural/apostrophe). Deadlock-free loading (load outside mutex). |
| `cleanup/itn/` | Inverse Text Normalization — 9 languages (FR, EN, DE, ES, PT, IT, NL, PL, RU). Each file: number parser + regex rules (ordinals, %, hours, currencies, units). `mod.rs`: dispatch + shared helpers (`replace_numbers`, `regex_rules!` macro). |
| `cleanup/llm_cloud.rs` | Cloud LLM text cleanup via OpenAI or Anthropic API (30s timeout). Uses `jona_engines::llm_prompt` for prompt templates and output sanitization. |

### UI (`ui/`)

Native UI elements rendered in pure Rust (no WebView).

| File | Role |
|------|------|
| `ui/tray.rs` | System tray icon, context menu (localized via `rust-i18n`), pill window lifecycle. Tray bar icons rendered as 44×44 SDF bitmaps. |
| `ui/pill.rs` | Native pill overlay — pure AppKit NSWindow + NSImageView, no WebView. RGBA bitmap rendered with SDF primitives (spectrum bars, bouncing dots, error X, queue badge). ~30fps animation, first frame pre-rendered (zero flash). |
| `ui/menu_icons.rs` | SDF icon rendering — shared primitives used by both tray icons and pill. Also provides transport type icons (laptop, USB, bluetooth, etc.) composited onto colored bubbles for menu items. |

### Workspace crates (`crates/`)

Engine catalog, shared types, and independent engine crates. The main crate imports these directly (e.g. `jona_engines::EngineCatalog`) — no re-export wrappers.

#### `jona-types` — Shared types

Defines the `ASREngine` trait, `ASRModel`, `EngineError`, `Preferences`, `Provider`, `ContextSlot`, and other types shared across the workspace. No inference logic.

#### `jona-engines` — Infrastructure

| Module | Role |
|--------|------|
| `EngineCatalog` | Collects all `ASREngine` implementations via `inventory`, provides model lookup, language listing, recommended model IDs. |
| `downloader` | Streaming HTTP downloads with resume (Range headers), per-model state, 250ms-throttled progress events, HuggingFace repos. Writes `version.json` after download (URL + ETag + SHA256 per file). Update detection via HTTP HEAD ETag comparison (HuggingFace `x-linked-etag` + standard fallback). |
| `ort_session` | Shared `build_session()` helper — adds CoreML EP to all ort sessions (Canary, Parakeet, BERT, PCS). Automatic dispatch to Metal GPU or Apple Neural Engine. |
| `mel` | Mel feature extraction with configurable `MelConfig` (HTK/Slaney scales, pre-emphasis, Bessel correction). Presets for Canary and Parakeet. |
| `llm_prompt` | LLM prompt templates, `LlmError` enum, `sanitize_output` (think-block stripping). |
| `audio` | Shared `read_wav_f32()` WAV reader. |

#### `jona-platform` — OS-specific code

macOS-specific code behind `#[cfg(target_os = "macos")]`, with stubs for future Windows support. Hotkey (CGEvent tap), permissions, paste, audio devices, audio ducking, sound playback, launch-at-login.

#### `jona-provider` — Cloud backends

`CloudProvider` trait + OpenAI-compatible and Anthropic backends. Handles ASR transcription, LLM chat completion, and model listing via cloud APIs.

#### Engine crates (10)

Each crate implements `ASREngine`, registers itself via `inventory::submit!`, and is fully self-contained (catalog + inference).

| Crate | Engine | Inference |
|-------|--------|-----------|
| `jona-engine-whisper` | Whisper (tiny → large-v3-turbo) | whisper-rs (Metal GPU) |
| `jona-engine-canary` | NVIDIA Canary 182M | ort + CoreML |
| `jona-engine-parakeet` | NVIDIA Parakeet-TDT 0.6B | ort + CoreML, vendored TDT decoder |
| `jona-engine-qwen` | Alibaba Qwen3-ASR 0.6B | qwen-asr crate (Accelerate/AMX) |
| `jona-engine-voxtral` | Mistral Voxtral 4B | vendored voxtral.c (Metal GPU) |
| `jona-engine-bert` | BERT punctuation | ort (ONNX + CoreML) or Candle (safetensors + Metal GPU) |
| `jona-engine-pcs` | PCS punctuation (47 lang) | ort (ONNX + CoreML), SentencePiece tokenizer |
| `jona-engine-correction` | T5 grammar correction | ort (ONNX + CoreML), autoregressive decoding, repeat penalty 1.5, n-gram blocking, live loop detection |
| `jona-engine-spellcheck` | SymSpell dictionaries | Data-only (no inference), manifest-driven from GitHub Releases |
| `jona-engine-llama` | Local LLM (llama.cpp) | llama-cpp-2, Metal GPU offload, GGUF Q4 models |

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
| `RecentsSection.vue` | Transcription history grouped by day with backend-driven search (SQLite LIKE), cursor-based pagination, virtual scroll (`@tanstack/vue-virtual`), and infinite scroll. Processing badges with tooltips. Copy toast animation. |
| `ModelsSection.vue` | Engine and model management with download progress, virtual scroll (`@tanstack/vue-virtual`) |
| `TranscriptionSection.vue` | ASR model selector (local + cloud unified), cloud sub-model picker, language, GPU mode with "Recommended" badge |
| `ProcessingSection.vue` | Post-processing: VAD, hallucination filter, text cleanup (BERT/PCS/T5/LLM unified selector), LLM token cap |
| `ShortcutsSection.vue` | Hotkey, recording mode (push-to-talk / toggle), cancel shortcut |
| `MicrophoneSection.vue` | Input device selector with transport type icons, mic test with spectrum + level badge, audio ducking |
| `ProvidersSection.vue` | Cloud provider management (13 presets + custom) |
| `DictionarySection.vue` | User dictionary for protected words and replacement mappings |
| `PermissionsSection.vue` | Permission status (microphone, accessibility, input monitoring) with grant buttons |
| `GeneralSection.vue` | Appearance (theme), interface language, About card (version, GPL-3.0 license) |

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
| `ProviderForm.vue` | Cloud provider configuration (9 presets + custom). Test button validates API key and fetches models. API key field starts empty when editing (backend keeps existing key if empty). `allow_insecure` toggle for Custom providers. |
| `ConfirmDialog.vue` | Reusable confirmation dialog (used for history clear/delete) |

### State management

Pinia stores split by domain:

| Store | Role |
|-------|------|
| `app.ts` | Runtime state, event listeners, initialization |
| `history.ts` | Cursor-based paginated history with backend-driven search and infinite scroll |
| `settings.ts` | User preferences |
| `engines.ts` | Engine catalog, providers, models |
| `downloads.ts` | Active downloads map (model ID → progress/speed/stopping), parallel downloads, optimistic pause transitions |

### Utilities

| File | Role |
|------|------|
| `utils/shortcut.ts` | Mirrors the Rust `Shortcut` type: `ShortcutDef` with `key_codes: number[]`, parse (with backward compat for old `key_code` singular format), format key caps with side labels, serialize |
| `utils/format.ts` | Byte formatting (`formatBytes`, `formatSize`, `formatSpeed`) for model sizes and download speeds |

### Stories (`stories/`)

Visual catalog of UI patterns using [Histoire](https://histoire.dev/).

| File | Role |
|------|------|
| `stories/setup.ts` | Histoire setup — provides Pinia, vue-i18n, and global CSS |
| `stories/UIPatterns.story.vue` | 10 variants: Card, Form Rows, Section Title, Filter Chips, History Item, Nav Pills, Status Dots, Provider Row, About Icon, Day Group |
| `stories/capture.ts` | Playwright script — launches Histoire, captures light + dark screenshots of each variant into `docs/screenshots/` |

## Threading model

The app uses six long-lived threads:

```
Main thread (Tauri + Tokio runtime)
  │
  ├── CGEvent tap thread (platform/hotkey.rs)
  │     Runs a CFRunLoop, processes keyboard events, sends HotkeyEvent via channel.
  │     Also polls an update channel every 500ms for config changes.
  │
  ├── Hotkey handler thread (recording/threads.rs)
  │     Reads HotkeyEvent from the channel, drives recording start/stop,
  │     forwards capture events to the frontend.
  │
  ├── Audio thread (recording/threads.rs → audio.rs)
  │     Owns the cpal::Stream. Receives AudioCmd, replies with AudioReply.
  │
  ├── Spectrum emitter thread (recording/threads.rs)
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
8. **Transcription** → `recording::pipeline::transcribe()` on a blocking thread (routes to cloud API or local engine via `ASREngine` trait)
9. **Post-processing** → hallucination filter, dictation commands, disfluency removal (filler word stripping), optional text cleanup (BERT / PCS / T5 correction / local LLM / cloud LLM with autoscale max_tokens), finalize (spacing, capitalization). On any cleanup error, falls back to raw transcription.
10. **Cancel check** → `transcription_cancelled` flag verified before paste
11. **Paste** → text written to clipboard → Cmd+V simulated via CGEvent
12. **History** → entry saved to SQLite (with cleanup_model_id, hallucination_filter, vad_trimmed, punctuation_model_id, spellcheck, disfluency_removal, itn metadata) → frontend notified via event

**Cancel flow:** Escape during recording → stops audio, discards file, shows error cross. Escape during transcription → sets cancel flag, clears queue. Both error and success pill closes use `PILL_CLOSE_GENERATION` to prevent stale closes from interfering with new recordings.

## Configuration

Preferences are stored as JSON in `~/Library/Application Support/JonaWhisper/preferences.json` with a `_version` field tracking the schema version. History lives in `history.db` (SQLite, WAL mode) in the same directory. All model files are stored under `models/` with subdirectories per engine (`whisper/`, `canary/`, `parakeet/`, `qwen-asr/`, `voxtral/`, `llm/`, `bert/`, `pcs/`, `correction/`).

On startup, `migrations.rs` checks `_version` and runs any pending migrations sequentially. Each migration receives both the raw JSON and the typed `Preferences` struct. To add a migration: append to the `MIGRATIONS` array and bump `CURRENT_VERSION`.

Shortcut values are stored as JSON objects (`{"key_codes":[54],"modifiers":1048576,"kind":"ModifierOnly"}`). Multi-key shortcuts store multiple key codes (up to 4). Legacy formats are automatically parsed for backward compatibility: old JSON (`key_code` singular) and legacy strings (`"right_command"`).

## Security

**API key storage**: Provider API keys are stored in the macOS Keychain via the `keyring` v3 crate (service `"JonaWhisper"`, username `"provider:<id>"`). Keys are never written to `preferences.json` — the `api_key` field is cleared before saving to disk, and populated from keyring on load. Migration v4 automatically moves any existing plaintext keys to the keychain on first run. The `get_providers` IPC command returns masked keys (`"••••abcd"`) to the frontend — the real key never reaches the webview.

**HTTPS enforcement**: Custom provider URLs are validated via `Provider::validate_url()`. HTTP URLs are rejected unless the per-provider `allow_insecure` flag is set. Known providers (OpenAI, Anthropic, etc.) always use hardcoded HTTPS URLs.

**CSP**: Content Security Policy is enabled in `tauri.conf.json` to restrict the webview to `'self'` for scripts, `'self' 'unsafe-inline'` for styles, and `'self' data: blob:` for images.

**IPC surface**: `withGlobalTauri` is disabled — `window.__TAURI__` is not exposed. All HTTP clients have explicit timeouts (30s for commands, 60s for audio uploads, 30s for LLM).
