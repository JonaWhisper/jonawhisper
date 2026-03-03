# Contributing to WhisperDictate

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
