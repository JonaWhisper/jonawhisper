# Dependencies

Every dependency has a reason. This document explains **what** each one does and **why** it was chosen over alternatives.

---

## Backend (Rust crates)

### Core / Framework

| Crate | Role | Why this one |
|-------|------|--------------|
| [`tauri`](https://github.com/tauri-apps/tauri) v2 | App framework (webview, tray, IPC) | Rust-native, tiny binary (~10 MB vs Electron ~150 MB), native webview, first-class macOS tray support |
| [`tauri-plugin-clipboard-manager`](https://github.com/tauri-apps/plugins-workspace/tree/v2/plugins/clipboard-manager) | Clipboard write before paste simulation | Only remaining Tauri plugin — handles NSPasteboard correctly. All other plugins (fs, shell, global-shortcut) were removed in favor of direct Rust/FFI |
| [`serde`](https://github.com/serde-rs/serde) / [`serde_json`](https://github.com/serde-rs/json) | JSON serialization (settings, IPC, engine catalog) | De facto standard, zero-cost abstractions, derive macros |
| [`tokio`](https://github.com/tokio-rs/tokio) | Async runtime | Required by Tauri 2. Used for transcription tasks, model downloads, API calls |
| [`log`](https://github.com/rust-lang/log) / [`env_logger`](https://github.com/rust-cli/env_logger) | Logging | Lightweight facade. `RUST_LOG=debug` for dev |
| [`thiserror`](https://github.com/dtolnay/thiserror) | Typed error derivation | Clean `AppError` enum with automatic `From` impls. Better than `anyhow` for library-like code |

### Audio

| Crate | Role | Why this one |
|-------|------|--------------|
| [`cpal`](https://github.com/RustAudio/cpal) | Audio capture (microphone) | Cross-platform, pure Rust, direct CoreAudio access. Lighter than PortAudio bindings |
| [`hound`](https://github.com/ruuda/hound) | WAV file I/O | Minimal, correct, no dependencies. Just reads/writes WAV |
| [`rustfft`](https://github.com/ejmahler/RustFFT) | FFT for spectrum visualization | Pure Rust, SIMD-optimized, no C dependencies. Used for mic test and pill spectrum bars |

### ASR Inference

| Crate | Role | Why this one |
|-------|------|--------------|
| [`whisper-rs`](https://github.com/tazz4843/whisper-rs) | Whisper transcription (GGML) | Metal GPU via whisper.cpp. Mature, well-maintained, GGML quantized models (75 MB–3.1 GB) |
| [`ort`](https://github.com/pykeIO/ort) 2.0.0-rc | ONNX Runtime + CoreML EP | Shared runtime for 5 models: Canary ASR, Parakeet ASR, Silero VAD, BERT punctuation, PCS punctuation. CoreML EP = automatic Metal/ANE dispatch |
| [`qwen-asr`](https://github.com/huanglizhuo/QwenASR) | Qwen3-ASR inference | Pure Rust, zero C dependencies, Apple Accelerate/AMX on Silicon. Only option for Qwen3-ASR |
| voxtral.c (vendored C) | Voxtral Realtime 4B | [antirez/voxtral.c](https://github.com/antirez/voxtral.c) — pure C + Metal GPU. No Rust crate exists yet. Compiled via `cc` in build.rs |

### Text Cleanup / ML

| Crate | Role | Why this one |
|-------|------|--------------|
| [`candle-core`](https://github.com/huggingface/candle) / `candle-nn` / `candle-transformers` | ML framework (Metal GPU) | Used for BERT punctuation (safetensors) and T5 correction (encoder-decoder). Metal GPU via candle-metal. Chosen over `tch` (libtorch 2 GB dependency) and `tract` (no Metal) |
| [`llama-cpp-2`](https://github.com/utilityai/llama-cpp-rs) | Local LLM inference (GGUF) | llama.cpp bindings with Metal GPU offload. GGUF Q4 quantization = small models (400 MB–2.5 GB). Alternatives: `mistralrs` (heavier), `llm` crate (abandoned) |
| [`ndarray`](https://github.com/rust-ndarray/ndarray) | Tensors for VAD | LSTM state and ONNX inputs/outputs for Silero VAD. Required by ort's tensor API |
| [`tokenizers`](https://github.com/huggingface/tokenizers) | HuggingFace tokenizer library | Used for PCS punctuation (SentencePiece Unigram). Built programmatically from protobuf model, cached as tokenizer.json |
| [`prost`](https://github.com/tokio-rs/prost) | Protobuf decoding | Parses SentencePiece `.model` files to extract Unigram vocabulary. Lighter than protobuf crate |
| [`encoding_rs`](https://github.com/nickel-org/encoding_rs) | Incremental UTF-8 decoding | LLM token-by-token streaming output. Handles partial multi-byte sequences at token boundaries |

### Network / IO

| Crate | Role | Why this one |
|-------|------|--------------|
| [`reqwest`](https://github.com/seanmonstar/reqwest) | HTTP client | Model downloads (Range headers for resume), cloud ASR/LLM API calls. Async, rustls (no OpenSSL dependency) |
| [`futures-util`](https://github.com/rust-lang/futures-rs) | Async stream utilities | `StreamExt` for download byte streams with progress tracking |
| [`rusqlite`](https://github.com/rusqlite/rusqlite) | Embedded SQLite | Transcription history with WAL mode (crash-safe). Paginated queries via LIMIT/OFFSET + LIKE search. Chosen over sled (less mature), redb (no SQL) |
| [`dirs`](https://github.com/dirs-dev/dirs-rs) | System paths | `~/Library/Application Support/` resolution. Cross-platform ready for Windows port |
| [`shellexpand`](https://github.com/netvl/shellexpand) | Path expansion | Expands `~` and env variables in user-configured paths |

### Concurrency

| Crate | Role | Why this one |
|-------|------|--------------|
| [`crossbeam-channel`](https://github.com/crossbeam-rs/crossbeam) | MPMC channels | Audio thread, hotkey events, spectrum data. Bounded channels with `select!`. Faster than `std::sync::mpsc`, supports multiple consumers |

### Text Processing / i18n

| Crate | Role | Why this one |
|-------|------|--------------|
| [`regex`](https://github.com/rust-lang/regex) | Regex engine | Hallucination filtering (30+ patterns), dictation commands, text finalization. Pre-compiled via `LazyLock` |
| [`rust-i18n`](https://github.com/longbridge/rust-i18n) | Backend i18n | Tray menu labels, system messages in FR/EN. Compile-time YAML loading, `t!()` macro |
| [`sys-locale`](https://github.com/1Password/sys-locale) | System locale detection | Auto-detect FR/EN at startup. Reads macOS `AppleLanguages` preference |

### macOS Platform (behind `cfg(target_os = "macos")`)

| Crate | Role | Why this one |
|-------|------|--------------|
| [`core-graphics`](https://github.com/nickel-org/core-foundation-rs) / [`core-foundation`](https://github.com/nickel-org/core-foundation-rs) | CoreGraphics FFI | CGEvent tap (global hotkey), CGEvent keyboard simulation (paste Cmd+V). Low-level, no Objective-C overhead |
| [`objc2`](https://github.com/madsmtm/objc2) / `objc2-foundation` / `objc2-app-kit` | Objective-C FFI | Mic permissions (AVFoundation), NSPasteboard, NSSound, NSWindow (pill overlay), app activation. Chosen over `objc` crate (unmaintained) — `objc2` is type-safe with proper retain/release |
| [`block2`](https://github.com/madsmtm/objc2) | Objective-C blocks | `requestAccessForMediaType:completionHandler:` callback. Part of the objc2 ecosystem |

### Removed dependencies (and why)

| Crate/Plugin | Was used for | Why removed |
|--------------|--------------|-------------|
| `tauri-plugin-global-shortcut` | Hotkey detection | Can't detect single modifier keys (Right Command alone). Replaced by raw CGEvent tap |
| `tauri-plugin-shell` | Subprocess launch | `std::process::Command` is simpler and sufficient |
| `tauri-plugin-fs` | File system access | `std::fs` is simpler and sufficient |
| `silero-vad-rust` / `voice_activity_detector` | VAD inference | ndarray version conflicts with ort 2.0.0-rc.11. Replaced by direct ort inference |

---

## Frontend (npm)

### App

| Package | Role | Why this one |
|---------|------|--------------|
| [`vue`](https://github.com/vuejs/core) 3 | UI framework | Composition API, excellent TypeScript support, lighter than React. Pairs well with Tauri |
| [`vue-router`](https://github.com/vuejs/router) | Client-side routing | Panel, SetupWizard views. Standard Vue router |
| [`pinia`](https://github.com/vuejs/pinia) | State management | Official Vue store. 5 domain-split stores (app, history, settings, engines, downloads). Simpler than Vuex |
| [`vue-i18n`](https://github.com/intlify/vue-i18n) | Internationalization | FR/EN translations. Composition API integration, message compilation |
| [`@tauri-apps/api`](https://github.com/tauri-apps/tauri) | IPC bridge | `invoke()` for commands, `listen()` for events. Type-safe Tauri API |

### UI Components

| Package | Role | Why this one |
|---------|------|--------------|
| [`lucide-vue-next`](https://github.com/lucide-icons/lucide) | Icons | ~1500 icons, tree-shakeable (only ships used icons). Consistent style, active community |
| [`reka-ui`](https://github.com/unovue/reka-ui) | Headless UI primitives | Foundation for shadcn-vue. Accessible, unstyled, composable. Replaces Radix Vue |
| [`class-variance-authority`](https://github.com/joe-bell/cva) | CSS variants | Component variant definitions for shadcn-vue (size, color, state) |
| [`clsx`](https://github.com/lukeed/clsx) / [`tailwind-merge`](https://github.com/dcastil/tailwind-merge) | CSS class utilities | Conditional classes + Tailwind conflict resolution. Used in shadcn-vue's `cn()` helper |
| [`@vueuse/core`](https://github.com/vueuse/vueuse) | Vue composables | Used by shadcn-vue internally. Also useful for `useEventListener`, `useResizeObserver` |

### Styling

| Package | Role | Why this one |
|---------|------|--------------|
| [`tailwindcss`](https://github.com/tailwindlabs/tailwindcss) | Utility-first CSS | Fast iteration, no CSS files to manage, consistent spacing/colors. Standard for shadcn-vue |
| [`tailwindcss-animate`](https://github.com/jamiebuilds/tailwindcss-animate) | CSS animations | Animate classes for shadcn-vue transitions (fade, slide, scale) |
| [`autoprefixer`](https://github.com/postcss/autoprefixer) / [`postcss`](https://github.com/postcss/postcss) | CSS post-processing | Vendor prefixes for Safari/WebKit compatibility in Tauri webview |

### Tauri Plugins (JS bindings)

| Package | Role | Why this one |
|---------|------|--------------|
| [`@tauri-apps/plugin-clipboard-manager`](https://github.com/tauri-apps/plugins-workspace/tree/v2/plugins/clipboard-manager) | Clipboard write | JS-side clipboard access before Rust-side paste simulation. Only Tauri plugin kept |

### Dev Tooling

| Package | Role | Why this one |
|---------|------|--------------|
| [`@tauri-apps/cli`](https://github.com/tauri-apps/tauri) | Tauri CLI | `tauri dev`, `tauri build`. Manages Rust + frontend builds |
| [`vite`](https://github.com/vitejs/vite) / [`@vitejs/plugin-vue`](https://github.com/vitejs/vite-plugin-vue) | Bundler | Sub-second HMR, native ESM. Standard for Vue 3 + Tauri |
| [`typescript`](https://github.com/microsoft/TypeScript) / [`vue-tsc`](https://github.com/vuejs/language-tools) | Type checking | Full type safety. `vue-tsc` for single-file component checking |

---

*Last updated: March 2026*
