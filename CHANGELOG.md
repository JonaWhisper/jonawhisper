# Changelog

All notable changes to JonaWhisper will be documented in this file.
## [0.3.0] - 2026-08-28

### Added

- **asr**: add Qwen3-ASR 1.7B, bump qwen-asr to 0.11
- **cleanup**: add Qwen3.5 0.8B, 2B and 4B to the local LLM catalog
- **asr**: add Canary 1B V2 with 25 European languages
- **ui**: tell the user when only cloud transcription is available

### Chore

- **deps**: migrate to Tailwind 4 **BREAKING**
- **deps**: bump typescript to 6.0 and drop deprecated baseUrl
- **deps**: bump vite to 8.2
- **deps**: bump pinia to 4.0
- **deps**: bump vue-router to 5.3
- **deps**: bump lucide-vue-next to 1.0
- **deps**: bump @vue/tsconfig, diff and @types/node
- **deps**: npm update within existing ranges
- **deps**: bump prost, symspell and candle, refresh the lock
- **deps**: bump tokenizers to 0.23, the only bump that earns its place
- **providers**: refresh cloud model lists against current APIs
- **deps**: bump whisper-rs to 0.16
- **deps**: bump cpal to 0.18
- **deps**: bump rust-i18n to 4
- **deps**: bump reqwest to 0.13
- **deps**: bump rusqlite to 0.40
- **deps**: bump whoami to 2
- **deps**: bump core-graphics to 0.25
- **deps**: drop the legacy rustls feature from the AWS SDK crates
- **deps**: update Rust dependencies within semver ranges
- **deps**: update npm dependencies within semver ranges
- **platform**: drop unused objc2-app-kit NSSound feature
- fix clippy errors breaking CI on main

### Documentation

- **deps**: record why typescript and vite are held back
- **architecture**: catch up with the audio thread removal
- **build**: record why arm-ggml-fix.cmake must stay

### Fixed

- **deps**: revert vite to 7, which npm install requires
- **asr**: use first() instead of get(0) to satisfy clippy
- **asr**: correct Qwen3-ASR RTF figures from measurement
- **asr**: repair Canary decoding, broken for every model
- **ui**: keep every cached model for single-capability providers
- **providers**: send AssemblyAI speech_models with universal model IDs
- **deps**: clear the quinn-proto advisories
- **platform**: play system sounds via AudioToolbox instead of NSSound
- **platform**: eliminate zombie processes from sound playback and open commands
- **ui**: never auto-select a cloud provider for transcription
- **ui**: name the model's actual usage in the delete warning
- auto-select first model when selectedModelId is empty
- show single model inline instead of dropdown in transcription
- add amber warning dot on Transcription nav item when no model
- show warning when no transcription model is installed

### Other

- catch peer dependency conflicts npm ci cannot see
- surface a missing updater signing key in the release summary

### Refactored

- **audio**: drop the audio owner thread, fix the reply desync

### Style

- **i18n**: escape non-ASCII in the strings this branch adds

## [0.2.0] - 2026-03-18

### Added

- **ui**: add pill render diagnostics + reset smoothed on Recording enter
- **ui**: add auto_release_memory toggle in Diagnostic section
- **ui**: add Diagnostic section in settings panel
- **perf**: memory diagnostics + ORT auto-release after long transcriptions
- **detect**: two-phase detection — probe Keychain without popup
- **detector**: add CLAUDE_CODE_OAUTH_TOKEN to env detection
- **detector**: add jona-detector-env for environment variable detection
- **ui**: add search bar to model and provider Select dropdowns
- **ui**: add update indicator in sidebar + centralize update state
- **ui**: add update button in About card + fix Copilot review
- **ci**: dual update channels (stable/unstable) + conditional signing
- **ui**: add Tauri auto-updater + Windows bundle config
- **providers**: complete auto-detection with OAuth, caching, and persistence
- **ui**: show detected providers with enable/disable toggle
- **state**: integrate auto-detection at startup with IPC commands
- **detector**: add jona-detector-claude-code crate
- **provider**: add detect_all() for credential auto-detection
- **types**: add DetectorRegistration, DetectedCredential, and Provider enabled/source fields
- **ui**: auto-resize provider form window to fit content
- **ui**: move provider form to separate Tauri window
- **providers**: add toggle fields for custom preset capabilities and insecure mode
- **providers**: add AWS Transcribe provider (streaming + batch)
- **providers**: add Azure OpenAI provider
- **ui**: show test button and results outside api_key block
- **providers**: add Azure Speech and Google Cloud Speech-to-Text crates
- **i18n**: add provider extra field labels in English and French
- **ui**: dynamic field rendering in ProviderForm.vue
- **ui**: extend frontend TypeScript types for dynamic provider fields
- **providers**: extend IPC for dynamic provider fields
- **providers**: add keychain helpers for sensitive extra fields
- **providers**: add PresetField, FieldType types and extend ProviderPreset/Provider
- **providers**: add Gladia and Speechmatics ASR crates
- **ui**: replace provider Select with searchable Combobox
- **providers**: add 7 cloud provider crates
- **providers**: add SambaNova and Nebius AI presets
- **settings**: add log retention mode selector (previous/3d/7d/30d/all)
- **logging**: preserve all log files with timestamp rotation
- **general**: write logs to disk with level selector
- **general**: add log level selector in settings
- **cleanup**: track and display protected words in spellcheck pipeline
- **ui**: show pipeline step status (no-change / error) in stepper
- **ui**: add pipeline step diff view and confidence tooltips
- **state**: store raw text and confidence scores in history
- **ui**: replace open_user_dict with structured get/save API
- **cleanup**: confidence-guided spell correction
- **ui**: add dedicated Dictionary section for user dictionary management
- **cleanup**: add phonetic filtering for SymSpell corrections
- **cleanup**: add user dictionary for spell-check protection and ITN mappings
- **cleanup**: add ITN for 7 new languages (DE, ES, PT, IT, NL, PL, RU)
- **cleanup**: expand hallucination filter with multilingual phrases and structural detection
- **ui**: add LanguageModel category to frontend
- **cleanup**: integrate KenLM context-aware reranking into spellcheck
- **engines**: add jona-engine-lm crate with vendored KenLM C++
- **engines**: migrate existing version.json to include ETags
- **engines**: unified ETag-based model update detection
- **engines**: switch T5 correction models from FP32 to INT8
- **engines**: model update detection with SHA256 verification
- **ui**: disable spellcheck toggle when no dict downloaded
- **engines**: dynamic spellcheck models from manifest
- **cleanup**: resolve regional spellcheck dict from system locale
- **engines**: add British English spellcheck model (en-gb)
- **engines**: add regional French spellcheck models (BE, CA, CH)
- **engines**: spellcheck URLs point to GitHub Releases
- **engines**: add jona-engine-spellcheck crate with downloadable per-language dicts
- **cleanup**: add French bigrams for phrase-level correction
- **cleanup**: enrich FR dictionary with DELA (125K → 645K words)
- **cleanup**: replace Hunspell with SymSpell frequency-weighted spell correction
- **ui**: add punctuation, spellcheck, disfluency, ITN badges to history
- **engines**: per-language model recommendations
- **state**: add migration v7 to clean up old Candle correction files
- **cleanup**: add spell-check (spellbook), flanec-base correction model, fix param counts
- **cleanup**: split punctuation and cleanup into independent pipeline steps
- **cleanup**: add ITN (Inverse Text Normalization) for FR/EN
- **cleanup**: add disfluency removal (filler word stripping)
- **providers**: add explicit ASR/LLM capability flags
- **platform**: hide launch-at-login when not signed with Developer ID
- **platform**: switch launch-at-login to SMAppService
- **platform**: add launch at login via SMAppService (macOS 13+)
- **ui**: comprehensive pill test with all states and queue flow
- **ui**: add Setup Wizard item to debug tray menu
- **ci**: hardened runtime, notarization support, macos-26 runners
- **ui**: show app version in General settings
- **ui**: add Histoire stories + screenshots for UI_GUIDELINES

### Chore

- add .claude/ to .gitignore
- **i18n**: remove dead provider.allowInsecure key
- update Cargo.lock and docs for azure-openai provider
- **ui**: add app icon and GitHub social preview
- update package-lock.json
- **providers**: add extra_fields/hidden_fields to all preset registrations
- remove unused wireframe.html
- add Copilot review instructions for better PR feedback
- add GitHub Sponsors funding configuration
- **engines**: update spellcheck manifest with new Italian bigram dict
- **todo**: add context persistence and conversational context items
- **ui**: add debug logging to pill state transitions
- **engines**: update KenLM manifest with real file sizes
- update spellcheck manifest and lockfile
- **engines**: update spellcheck manifest to v1.0.4 (19 langs) and add KenLM 9-lang placeholders
- add diagnostic logging for transcription latency and flat spectrum
- reduce tokio features from "full" to ["time", "rt"]
- fix stale docs, remove dead code
- remove convert-pcs-tokenizer.py (Rust builds it natively)
- remove quantize_t5.py (moved to jonawhisper-model-tools)
- remove build_symspell_dicts.py (moved to jonawhisper-spellcheck-dicts repo)
- add model update mechanism to TODO
- **engines**: point spellcheck URLs to GitHub repo
- **scripts**: consolidate all dict generation in build_symspell_dicts.py
- remove unused candle deps from main crate
- remove unused DOWNLOAD_PROGRESS constant from events.rs
- add GPL-3.0 license + document spectrum bug
- **ci**: upgrade runners to macos-15 for Metal 3.2 SDK
- rename WhisperDictate → JonaWhisper

### Documentation

- **todo**: clean up resolved items, keep only open tasks
- **todo**: mark logging refonte as resolved
- **todo**: mark memory/performance items as resolved (PR #35)
- update ARCHITECTURE.md and copilot-instructions for recent changes
- **todo**: mark setup wizard + input monitoring check as resolved
- **todo**: add setup wizard persistence bug + input monitoring check fix
- **todo**: add memory management tasks, update spectrum flat bug status
- update BUILD-SECRETS for Certum Cloud HSM signing
- update provider counts and add Azure/Google Speech
- update provider counts and implementation status
- update proprietary ASR table with implementation status
- update CLOUD-PROVIDERS.md to reflect all implemented crates
- update preset count to 20 with new provider crates
- update cloud preset count, add DictionarySection, fix CI badge URL
- fix crate paths and Nebius model ID in CLOUD-PROVIDERS.md
- reorganize cloud provider docs and add auto-detection guide
- mark confidence-based correction as implemented in TODO
- mark user dictionary as implemented in TODO and TEXT-PIPELINE
- update project docs with pipeline research, stepper UI, and cleanup paths
- update TODO with research findings and TEXT-PIPELINE with module split
- **todo**: add logging refactor plan
- **todo**: update SymSpell status with KenLM integration progress
- standardize all Silero VAD references to v6.2
- fix repo URLs (dictate-macos → jona-whisper) and VAD version (v6 → v5)
- **todo**: add latency diagnostic, SymSpell quality, and update spectrum bug entries
- remove obsolete GEC research, fix stale spellcheck/T5 references
- fix T5 model sizes (FP32 → INT8), spellcheck references
- fix unicode escapes in markdown, remove completed tasks
- update ARCHITECTURE, README, TODO for ETag system
- mark model update detection as done in TODO
- update all documentation to reflect current codebase state
- **cleanup**: update GEC research with conversion results
- **cleanup**: fix step ordering in TEXT-PIPELINE.md
- **cleanup**: complete GEC research with SymSpell, DELA, phonetics, broader search results
- **cleanup**: comprehensive update to GEC research with spellbook, T5 analysis, conversion strategy
- **cleanup**: update TEXT-PIPELINE.md for pipeline chaining
- **cleanup**: add GEC research survey (LanguageTool, models, spell-check)
- **cleanup**: update roadmap after Harper/GECToR evaluation
- update architecture and pipeline docs
- remove redundant npm install from build instructions
- document thin orchestrator architecture, update module maps
- update architecture for cursor pagination, virtual scroll, and license
- add TODO to track whisper-rs-sys ggml i8mm workaround removal
- update requirements and build docs for hardened runtime changes
- clean up TODO.md — remove completed items
- update ARCHITECTURE, README, CLOUD-INTEGRATION for security changes
- update DEPENDENCIES and TODO for security hardening
- update DEPENDENCIES, CONTRIBUTING, TODO, ARCHITECTURE for Histoire
- **ui**: update UI_GUIDELINES to reflect Tailwind-only styling
- **ui**: rewrite UI_GUIDELINES to match current glassmorphism design
- add DEPENDENCIES.md with rationale for each dependency
- rewrite README, ARCHITECTURE, and CONTRIBUTING for open source
- add commit convention guidelines and update cliff config

### Fixed

- **ci**: force-push release branch + skip PR creation if already exists
- **ci**: revert squash merge match — we always use merge commits
- **ci**: match squash merge commit message + remove Node dependency in tag job
- **build**: remove old .app before build to detect real failures
- **build**: ignore tauri signing key error, check .app existence instead
- use app_state directly instead of app.state() + add startup log
- **ui**: persist setup_completed flag, show wizard until step 2 done
- **setup**: load languages at app init so setup wizard has data
- **ui**: resolve compiler warnings and setup language dropdown
- **i18n**: localize N/A in diagnostic RSS display
- Preferences default via serde, rename ORT constant, Windows link attr
- **perf**: capture model_id before spawn, rate-limit pill flat warn
- **platform**: check Input Monitoring with full event mask, not just flagsChanged
- **perf**: address second review — settings wiring, typed struct, page size, mutex
- remove needless borrows in maybe_auto_release calls (clippy)
- **perf**: address PR review — race condition, cross-platform RSS, stable context list
- **detect**: persist detector sources for reliable skip on restart
- **audio**: rate-limit fft_buffer contention warning in realtime callback
- **detect**: use persisted detected_enabled to skip detectors on restart
- **audio**: improve flat spectrum detection with visual threshold + diagnostics
- **state**: BusyGuard panic safety + run_detection lock ordering
- **detect,ui**: default detected providers to enabled, complete ARIA tabs
- **state**: rewrite ContextMap with busy-set concurrency model
- **ci**: clippy vec!→array in detector, remove $attrs.class binding
- ContextMap deadlock + MaxTokensSlider inheritAttrs
- **ui**: wrap null_mut in MainThreadPtr in pill test to fix CI build
- **ui**: remove unsafe string cast on $attrs.class in MaxTokensSlider
- address Copilot review — ContextMap race, provider error propagation, LIKE escaping
- **audio**: replace spectrum Mutex with lock-free AtomicSpectrum
- **detect**: skip detectors for disabled providers to avoid Keychain popups
- gate test_pill tray handler in release, fix build errors
- audit cleanup round 2 — debug gate, token TTL, dead export
- **ui**: audit cleanup — computed, i18n, dead CSS, dead code
- address audit findings — LIKE escape, ContextMap TOCTOU, dead code
- **ci**: reset working tree before base branch checkout in coverage
- **providers**: use UTF-8 safe truncation for error body log
- **audio**: fix callback count off-by-one and downgrade contention logs to debug
- **ui**: reset updateAvailable before check to avoid stale update info on failure
- **providers**: re-add debug log for API error response body (truncated to 200 chars)
- **docs**: update timestamp server URL to match tauri.conf.json (Sectigo)
- **ci**: complete macOS signing secret check and disable Windows signing
- address review comments on logging and safety
- address CI clippy error and review comments
- **docs**: revert timestampUrl to HTTP — Certum timestamp server is HTTP-only
- address PR review feedback and add safety tests
- **state**: remove URL from provider API error log
- **state**: guard keyring against storing masked API keys
- **cleanup**: add diagnostic log for cloud LLM api_key presence
- use is_multiple_of() for Clippy 1.94 compat
- **providers**: reset selection on disable and resolve auto-detected credentials
- **detector**: correct module docs to reflect macOS-only compilation
- **ui**: reset model selection when cloud provider is disabled
- **state**: default supports_llm to false for unknown presets
- **providers**: deduplicate detected providers in get_providers()
- **build**: replace invalid strip_suffix closure with simple split parse
- **providers**: add missing enabled/source fields in azure-openai test
- **ui**: use parseCloudId() in validateSelections to avoid false matches
- **state**: drop mutex before Keychain I/O and disambiguate provider IDs
- **detector**: make jona-detector-claude-code macOS-only
- **providers**: downgrade detection logs to DEBUG, remove token from log
- **providers**: add missing enabled/source fields in aws-transcribe test
- **ui**: use destroy() for sync window close + guard missing provider
- **ui**: close stale provider form window + add save error handling
- **ui**: fix provider form window sizing and disable minimize/maximize
- **ui**: correct WebviewWindow import for pseudo-modal focus
- **i18n**: remove generic provider.field.api_key to preserve preset labels
- **providers**: don't override user-configurable capabilities from preset
- **providers**: address PR #24 review comments
- **ui**: match original capabilities layout (flex gap-4)
- **ui**: match original toggle styles and add missing i18n keys
- **ui**: add API key field for custom providers
- **audio**: read spectrum directly from recorder, eliminating flat spectrum bug
- **providers**: comprehensive audit fixes across all 13 provider crates
- **providers**: cap batch poll to 100s to fit within pipeline timeout
- **providers**: address seventh round of PR #21 review
- **providers**: address sixth round of PR #21 review
- **providers**: address fifth round of PR #21 review
- **providers**: address fourth round of PR #21 review
- **providers**: address third round of PR #21 review
- **providers**: address second round of PR #21 review
- **providers**: address PR #21 review comments for AWS Transcribe
- **providers**: reject dot-only and leading/trailing dot in URL segments
- **providers**: validate deployment_name in list_models (PR #22 review)
- **providers**: address PR #22 review — model param, URL validation
- **tests**: exclude azure-openai from default models assertions
- **ui**: hide URL field for preset providers
- **ui**: use app icon in General About card
- **providers**: harden Copilot JWT cache — full token key, hold lock across fetch
- **providers**: address PR review — copilot guards, allow keyless OpenAI-compat
- **providers**: use trimmed API key in all request headers
- **providers**: address PR review — trim API keys, strict polling, scoped JWT cache
- **providers**: address audit findings across all provider crates
- **ui**: open provider combobox on input focus
- **providers**: enforce HTTPS for presets and fix async preset race
- **providers**: resolve clippy collapsible_if and consolidate settings lock
- remove accidentally committed agent worktree refs
- **providers**: harden masked value detection and clear sentinel
- **providers**: address PR review — sentinel docs, extraValues defaults, comments
- **providers**: support clearing sensitive extra fields via sentinel
- **providers**: address PR review — UTF-8 safe masking, hydrate sensitive extras, filter hidden fields
- **providers**: add missing extra field to Provider test constructors
- **providers**: URL-encode Google Speech API key in request URL
- **providers**: address PR review — sanitize API key in errors, validate Azure region, document encoding
- **providers**: address Copilot review — correct Speechmatics doc comment
- **ui**: move Custom option to top of provider combobox with separator
- **audio**: add grace period for flat spectrum warnings at recording start
- **providers**: address Copilot review — guard empty API keys, fix polling timeout, use header auth for Gemini
- **audio**: add grace period for flat spectrum warnings at recording start
- **audio**: address PR review — non-blocking transition, named handles, no GetSpectrum spam
- **audio**: suppress spectrum flat warning during startup grace period
- **pill**: use AtomicBool samples_received instead of spectrum polling
- **pill**: wait for first audio samples before transitioning to Recording
- **state**: initialize provider catalog before AppState
- resolve 6 clippy warnings from Rust 1.94
- **cleanup**: strip <unk> tokens, add tooltip to logs button
- **cleanup**: disable dictation commands when punctuation model active
- **ui**: show main text when spellcheck has protected words but no diff
- **itn**: strip commas too, prevent raw atom combining
- **cleanup**: ITN trailing punct, cross-lang accent guard, error bubble
- **ui**: improve protected words display in history
- **cleanup**: guard DoubleMetaphone from non-ASCII panic
- **cleanup**: resolve SS_CACHE mutex deadlock in spellcheck
- **pipeline**: run spellcheck in spawn_blocking to avoid blocking tokio
- **pipeline**: add 120s timeout to transcription task
- **pipeline**: add RAII guard to reset is_transcribing on panic
- **ui**: display final corrected text in history, not raw ASR output
- **cleanup**: protect anglicisms from spell-check via cross-language guard
- **engines**: satisfy clippy needless_range_loop in mel.rs
- **cleanup**: strip cross-language hesitation fillers (uh/um in FR, euh in EN)
- **ui**: make error X overlay smaller and more transparent
- **ui**: refine pipeline error/nochange overlays
- **ui**: use Lucide X/Minus icons for pipeline error/nochange overlays
- **itn**: match "pourcent(s)" as single word for FR percentage
- **cleanup**: prevent spellcheck deadlock and isolate test dicts
- **itn**: require digit before heure→h conversion and show ITN diffs
- **test**: add non-null assertions in download store tests
- **engines**: restore download speed calculation and fix pause state
- **ui**: split mapping input into two fields (pattern → replacement)
- **ui**: add tooltips on dictionary entry type icons
- **ui**: replace tooltip-wrapped icon with visible Edit button for user dict
- correct VAD version comment (v5 → v6) and upgrade spectrum log to warn
- **engines**: recover missing .complete marker for fully downloaded multi-file models
- **test**: resolve pre-existing vue-tsc errors in test files
- **commands**: propagate Result from history, engines, and provider commands
- reduce provider cloning, downgrade user-text log, fix store mutation
- **ui**: localize byte units and add aria-labels to icon buttons
- **platform**: use map_err for Result types in simulate_paste
- **engines**: add download timeout and deduplicate download logic
- **state**: propagate SQLite errors from history queries
- **engines**: release ContextMap lock during inference
- **platform**: return Result from simulate_paste and guard FFI callback
- **test**: use real spellcheck dicts instead of fake test fixtures
- **engines**: update HF URLs from realjPlot to JonaWhisper org
- **ui**: add 'punctuation' to CleanupModel group type
- **docs**: use native UTF-8 chars instead of unicode escapes in markdown
- **cleanup**: catch T5 whole-phrase duplication
- **cleanup**: harden T5 correction against repetition loops
- **ui**: shortcut capture too wide in settings
- **engines**: force-link engine crates so inventory registrations survive
- **ui**: add fallback for panel window recreation + startup warning
- **ui**: disable launch-at-login switch while command is in flight
- **platform**: use launchctl bootstrap/bootout for launch at login
- **platform**: switch launch-at-login to LaunchAgent plist
- **build**: auto-install npm dependencies when needed
- **ui**: match ShortcutCapture size with Select component
- **platform**: shortcut capture not working in setup wizard
- **platform**: mic staying active after releasing hotkey
- **ci**: disable GGML_NATIVE in toolchain file
- **ci**: use CMAKE_TOOLCHAIN_FILE for ggml ARM arch fix
- **ci**: guard MTLMathModeFast with SDK version check
- **state**: migrate API keys to OS keychain and harden security

### Other

- merge Rust Check and Rust Tests into single job
- move Frontend job to ubuntu-latest
- bump all GitHub Actions to latest major versions
- remove redundant cargo check --release from rust-check job
- parallelize CI into 3 independent jobs
- **coverage**: add unified Rust + Frontend coverage report on PRs
- **coverage**: add PR coverage workflow with before/after comparison
- add frontend Vitest tests to CI pipeline
- **general**: show GPL-3.0 license in About card
- add Voxtral (Mistral) ASR integration options (cloud, voxtral.c, vLLM, Transformers)
- move side label to right of symbol
- group inference contexts into InferenceContexts with ContextSlot<T>
- subtler selected state, remove recording mode desc, add silence detection TODO
- move model recommendations from frontend to backend
- add local model catalogue, LLM hallucination filter, inference tools
- add UX note for WER/RTF visual display
- replace static atomics with TapState struct via user_info; centralize event names in events.rs
- deduplicate engines, thread safety, file organization

### Performance

- raise ORT release threshold from 5s to 30s transcription time
- downgrade SeqCst to Relaxed on all independent atomics
- **audio**: use try_lock in realtime callback and compute FFT outside lock
- **engines**: skip config/tokenizer ETags in update check
- **cleanup**: add T5 INT8 quantization support (75% smaller, ~2-3x faster)
- **ui**: instant panel open via pre-created hidden WebView
- **build**: set codegen-units = 1 for smaller, faster release binary
- **ui**: lazy-load audio devices, languages, and history
- **ui**: virtual scroll for models list
- **ui**: virtual scroll for history list
- **ui**: replace OFFSET pagination with cursor-based pagination

### Refactored

- **ci**: split back into two files (prepare + build)
- **ci**: consolidate release into single workflow (prepare PR + auto tag + build)
- **ci**: split release into prepare PR + auto build on merge
- **ui**: MainThreadPtr newtype and ARIA accessibility improvements
- explicit jona_types imports and capture listener unlisten fns
- **ui**: extract useProviderModels composable and MaxTokensSlider component
- **imports**: use jona_types:: directly instead of crate::state:: re-export
- **providers**: extract sensitive field helpers and reduce duplication
- **i18n**: use provider.field.* pattern for all extra fields
- **providers**: remove unnecessary hidden_fields base_url
- **ui**: make custom a preset with URL as extra field
- **ui**: move api_key to preset-defined extra_fields
- **provider**: replace ApiFormat enum with string-based backend_id
- **provider**: replace ProviderKind enum with data-driven ProviderPreset system
- **provider**: split jona-provider into multi-crate architecture with inventory
- **itn**: move unit words to per-language files and fix un/une disambiguation
- **ui**: replace history badges with pipeline stepper icons
- **cleanup**: split post_processor.rs into concern-based modules
- **cleanup**: split monolithic itn.rs into per-language modules
- **ui**: extract HistoryEntryCard from RecentsSection
- **ui**: extract SettingToggle component from ProcessingSection
- **engines**: deduplicate storage_dir() and read_wav_f32()
- **engines**: remove migration code
- **engines**: extract migrations into dedicated module
- **engines**: reuse blocking clients for ETag fetching via LazyLock
- **cleanup**: dynamic language resolution for spellcheck dictionaries
- **engines**: migrate T5 correction from Candle to ONNX Runtime
- **engines**: auto-generate extern crate links from Cargo.toml
- clean up main crate — remove re-exports, split recording & commands
- **engines**: plug-and-play engine system with inventory auto-registration
- **engines**: merge inference into cleanup engine crates
- **engines**: extract Llama, BERT, PCS, Correction into crates
- **engines**: extract Voxtral into jona-engine-voxtral crate
- **engines**: extract Qwen, Canary, Parakeet into independent crates
- **engines**: extract Whisper into standalone jona-engine-whisper crate
- **providers**: extract cloud API logic into jona-provider crate
- centralize shared patterns and add type safety
- simplify codebase — lazy iteration, cached icons, single-pass VAD
- **engines**: extract jona-engines crate
- **platform**: extract jona-platform crate
- **state**: create Cargo workspace + jona-types crate
- **ui**: migrate custom CSS classes to Tailwind utilities

### Reverted

- **detect**: keep detected providers disabled by default

### Style

- **ui**: make low-confidence word underline more visible
- **i18n**: harmonize terminology — use "Transcription" everywhere

### Testing

- update formatRam tests to match i18n units (Go/Mo)
- **types**: add keyring guard and mask round-trip tests
- **ui**: add checkForUpdate state transition tests
- **ui**: add validateSelections tests for disabled vs removed providers
- **providers**: add non-ASCII test for mask_value
- address Copilot review on provider catalog tests
- **providers**: add 14 provider catalog integrity tests
- address Copilot review — proper mock payloads in app/engines tests
- fix tautological assertions and add store tests
- add 76 unit tests for providers, migrations, and post-processor
- add ITN + spellcheck guard tests, fix zero conversion, remove line-clamp
- add confidence scoring and spell correction tests
- **stores**: add 16 behavioral tests for downloads store
- add pipeline e2e, history SQLite, and clippy CI
- **pill**: add 23 behavioral tests for pill rendering
- add behavioral test suite (237 Rust + 78 frontend tests)

