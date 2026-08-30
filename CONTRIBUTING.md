# Contributing to JonaWhisper

## Prerequisites

Everywhere:

- [Rust](https://www.rust-lang.org/tools/install) (stable)
- [Node.js](https://nodejs.org/) 24+

### macOS (14.0+, Apple Silicon)

- Xcode Command Line Tools: `xcode-select --install`

### Windows x64

- **Visual Studio Build Tools** with the *Desktop development with C++* workload —
  needed even for `cargo check`, which compiles and runs the build scripts
- **CMake** — `whisper-rs-sys` and `llama-cpp-sys-2` build ggml and llama.cpp with it

```powershell
winget install --id Microsoft.VisualStudio.2022.BuildTools -e --source winget `
  --override "--quiet --wait --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
winget install --id Kitware.CMake -e --source winget
```

### Windows on ARM (Copilot+ PC, Surface, a VM on Apple Silicon)

Everything above, plus two tools that x64 does not need. They are not optional:
the errors they raise name them.

- **LLVM/clang-cl** — `ring` and `aws-lc-sys` refuse to build on
  `aarch64-pc-windows-msvc` without it (*"Windows ARM64 requires clang-cl"*)
- **Ninja** — ggml rejects MSVC on ARM (*"MSVC is not supported for ARM, use
  clang"*), and the Visual Studio generator ignores `CC`/`CXX`, so the build must
  go through a generator that honours them

```powershell
winget install --id LLVM.LLVM -e --source winget
winget install --id Ninja-build.Ninja -e --source winget
```

Build from a shell where the MSVC environment is loaded — `clang-cl` compiles
but does not know where the Windows SDK headers and libraries are, and CMake
fails on its `project()` line without them:

```bat
call "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\Common7\Tools\VsDevCmd.bat" -arch=arm64 -host_arch=arm64
set CMAKE_GENERATOR=Ninja
set CC=clang-cl
set CXX=clang-cl
set CXXFLAGS=/EHsc
cargo check -p jona-whisper
```

`CXXFLAGS=/EHsc` is the last of them: with `CXX` forced to clang-cl, cc-rs no
longer recognises the compiler as MSVC and stops passing the flag itself, so
`llama-cpp-sys-2` fails on `cannot use 'try' with exceptions disabled`.

With all five in place the whole binary builds on Windows ARM — verified, 4m26
from a cold cache. Releases are still x64: this path is for checking a change
before pushing it, not for shipping.

## Development setup

```bash
# Clone the repo
git clone https://github.com/JonaWhisper/jonawhisper.git
cd jonawhisper

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
| `crates/jona-engine-*/` | ASR & cleanup engines (whisper, canary, parakeet, qwen, voxtral, bert, pcs, correction, llama, spellcheck, lm) |
| `crates/jona-engines/` | Engine catalog, model downloads, shared inference utilities |
| `crates/jona-types/` | Shared types (ASREngine trait, ASRModel, Preferences, etc.) |
| `crates/jona-platform/` | macOS-specific code (hotkeys, permissions, paste) |
| `crates/jona-provider*/` | Cloud provider backends (11 crates: OpenAI-compatible, Anthropic, Deepgram, Copilot, Gemini ASR, Rev.ai, AssemblyAI, ElevenLabs, Cohere, Gladia, Speechmatics) |
| `src-tauri/src/cleanup/` | Text cleanup pipeline (VAD, post-processing, spellcheck, ITN, LLM) |
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
