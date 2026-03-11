# Copilot Instructions — JonaWhisper

## Project Overview

JonaWhisper is a Tauri v2 dictation app with a Rust backend and Vue 3 frontend.
It runs ASR (speech-to-text) models **locally** on the user's machine — privacy-first, no cloud required.

## Architecture

- **Cargo workspace** with 26 crates under `src-tauri/crates/`
- Main crate: `src-tauri/src/` — thin orchestrator (AppState, recording, commands, UI)
- Frontend: `src/` — Vue 3 + Pinia stores + shadcn-vue + Tailwind
- Engine crates auto-register via `inventory::submit!` + `EngineCatalog::init_auto()`
- Provider backends also use `inventory` for registration

## Key Patterns to Know

### Rust

- `CloudProvider` is a **trait object** (`&dyn CloudProvider`), not an enum — no pattern matching on backend types
- `backend()` and `backend_for_provider()` return `&'static dyn CloudProvider` and **panic** if not found
- Engine crates MUST have `extern crate` in `lib.rs` (linker eliminates dead code otherwise)
- Tests using `require_dicts!()` macro are **local-only** — they skip in CI (no spell-check dictionaries available)
- `context_group!` macro manages engine contexts in AppState
- Platform code uses proper FFI (objc2, CoreGraphics) — never osascript

### Frontend

- Stores use Pinia composition API (`defineStore('name', () => { ... })`)
- Tauri IPC via `invoke()` from `@tauri-apps/api/core`
- Event listeners via `listen()` from `@tauri-apps/api/event`
- i18n JSON files use Unicode escapes (`\u00e9` not `é`)
- Icons: `lucide-vue-next` only
- No `window.__TAURI__` — imports only (`withGlobalTauri: false`)

### Testing

- Frontend: vitest + happy-dom. Mock pattern: `vi.mock('@tauri-apps/api/core')` with `mockInvoke`
- Rust: standard `#[cfg(test)] mod tests` in each file
- No component (`.vue`) tests — ROI too low, complexity is in stores and Rust
- No integration tests for platform/FFI code (macOS-specific, not unit-testable)

## Review Guidelines

When reviewing PRs, focus on:

1. **Security**: API keys must go through keyring, never in preferences.json. HTTPS enforced for providers. No secrets in logs.
2. **Thread safety**: AppState fields behind `Mutex`/`RwLock`. Dict loading outside mutex lock to avoid deadlock.
3. **Error handling**: engines should fail gracefully (fallback, not panic). Cloud API calls need timeouts.
4. **Platform portability**: code in `jona-platform` uses `cfg(target_os)` with stubs for non-macOS.
5. **Dead code**: unused imports, functions, or dependencies should be flagged.

Do NOT flag:

- Missing tests for `.vue` components, platform FFI code, or recording pipeline (intentionally untested)
- `extern crate` statements that look unused — they are required for `inventory` registration
- `unsafe` in platform code (objc2 FFI requires it)
- `#[allow(unused)]` on engine crate imports in `lib.rs` (needed for linker)

## Conventions

- **Commits**: Conventional Commits format (`type(scope): description`)
- **License**: GPL-3.0-or-later
- **Language**: French-speaking maintainer, UI localized FR/EN
- **No over-engineering**: prefer simple, direct solutions
