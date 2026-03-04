# Contributing to JonaWhisper

## Prerequisites

- macOS 14.0+ (Apple Silicon)
- [Rust](https://www.rust-lang.org/tools/install) (stable)
- [Node.js](https://nodejs.org/) 24+
- Xcode Command Line Tools: `xcode-select --install`

## Development setup

```bash
# Clone the repo
git clone https://github.com/jplot/dictate-macos.git
cd dictate-macos

# Install frontend dependencies
npm install

# Start dev mode (Vite hot reload + Rust auto-rebuild)
npm run tauri dev
```

For a release build:

```bash
./build.sh
open build/JonaWhisper.app
```

## Architecture

See [ARCHITECTURE.md](ARCHITECTURE.md) for the full module map, threading model, and data flows.

Key directories:

| Directory | Contents |
|-----------|----------|
| `src/` | Vue 3 frontend (views, sections, components, stores) |
| `src-tauri/src/asr/` | ASR inference (whisper, canary, parakeet, qwen, voxtral) |
| `src-tauri/src/cleanup/` | Text cleanup (punctuation, correction, VAD, LLM) |
| `src-tauri/src/engines/` | Engine catalog, model downloads |
| `src-tauri/src/platform/` | macOS-specific code (hotkeys, permissions, paste) |
| `src-tauri/src/ui/` | Native UI (tray, pill overlay, SDF icons) |
| `src/stories/` | Histoire stories + Playwright capture script |
| `docs/` | Pipeline docs, benchmarks, screenshots |

## UI stories

The project uses [Histoire](https://histoire.dev/) to catalog UI patterns visually.

```bash
# Browse stories interactively
npm run story:dev

# Regenerate screenshots for docs/UI_GUIDELINES.md
npm run story:screenshots
```

Stories live in `src/stories/`. When modifying a UI pattern in `docs/UI_GUIDELINES.md`, update the corresponding story variant and regenerate screenshots.

## Pull requests

1. Create a feature branch from `main`
2. Make your changes with commits following the convention below
3. Run checks locally: `npx vue-tsc -b --noEmit` and `cd src-tauri && cargo check --release`
4. Open a PR against `main` with a clear description

## Commit Convention

This project follows [Conventional Commits](https://www.conventionalcommits.org/). Every commit message must follow this format:

```
<type>(<scope>): <description>

[optional body]

[optional footer]
```

### Types

| Type | Usage |
|------|-------|
| `feat` | New feature or capability |
| `fix` | Bug fix |
| `refactor` | Code restructuring (no behavior change) |
| `perf` | Performance improvement |
| `docs` | Documentation only |
| `style` | Formatting, whitespace, no code change |
| `test` | Adding or updating tests |
| `chore` | Build, deps, CI, tooling |
| `revert` | Revert a previous commit |

### Scopes

Use a scope to indicate the area affected:

| Scope | Area |
|-------|------|
| `asr` | ASR engines (whisper, canary, parakeet, qwen, voxtral) |
| `cleanup` | Text cleanup pipeline (punctuation, correction, VAD, LLM) |
| `ui` | Frontend Vue components, CSS, layout |
| `tray` | Tray menu, pill overlay, native UI |
| `engines` | Engine catalog, downloads, model management |
| `platform` | macOS permissions, hotkeys, OS-specific code |
| `audio` | Audio recording, devices, FFT |
| `state` | AppState, preferences, history |
| `i18n` | Translations (FR/EN) |
| `ci` | GitHub Actions, build pipeline |

Scope is optional but encouraged. Omit it for cross-cutting changes.

### Examples

```
feat(asr): add Qwen3-ASR 0.6B engine
fix(tray): pill overlay not closing on error
refactor(cleanup): extract shared punctuation windowing logic
perf(asr): enable CoreML EP for Parakeet encoder
docs: update README with build instructions
chore(ci): add release workflow with git-cliff
chore: bump ort to 2.0.1
feat(ui): add model filter chips with category colors
fix(platform): accessibility permission check on macOS 15
revert: "feat(asr): add Qwen3-ASR 0.6B engine"
```

### Breaking Changes

Add `!` after the type/scope and describe in the footer:

```
feat(state)!: migrate preferences to SQLite

BREAKING CHANGE: preferences.json is no longer read, run migration first.
```

### Rules

- **Imperative mood**: "add feature" not "added feature" or "adds feature"
- **Lowercase** description: "add dark mode" not "Add dark mode"
- **No period** at the end of the description
- **One logical change** per commit — don't mix a feature and a refactor
- Keep the first line under **72 characters**
