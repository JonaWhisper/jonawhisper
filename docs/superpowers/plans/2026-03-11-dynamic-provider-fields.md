# Dynamic Provider Fields — Implementation Plan

## Problem

All cloud providers currently share the same form: name, API key, base URL. This is insufficient:

- **Azure Speech** needs a `region` field and uses a different auth header (`Ocp-Apim-Subscription-Key` instead of `Bearer`)
- **AWS Transcribe** needs `access_key`, `secret_key`, and `region` — no single API key
- **Copilot** and **Gemini ASR** should hide the base URL field (hardcoded endpoints)
- Future providers may need dropdown selects (e.g., Azure API version)

## Current Architecture (Reference)

### Rust — `jona-types`

`ProviderPreset` (`src-tauri/crates/jona-types/src/provider.rs`):
```rust
pub struct ProviderPreset {
    pub id: &'static str,
    pub display_name: &'static str,
    pub base_url: &'static str,
    pub backend_id: &'static str,
    pub supports_asr: bool,
    pub supports_llm: bool,
    pub gradient: &'static str,
    pub default_asr_models: &'static [&'static str],
    pub default_llm_models: &'static [&'static str],
}
```

`Provider` (`src-tauri/crates/jona-types/src/lib.rs:224`):
```rust
pub struct Provider {
    pub id: String,
    pub name: String,
    pub kind: String,       // preset ID or "custom"
    pub url: String,
    pub api_key: String,
    pub allow_insecure: bool,
    pub cached_models: Vec<String>,
    pub supports_asr: bool,
    pub supports_llm: bool,
    pub api_format: Option<String>,
}
```

### Rust — IPC commands (`src-tauri/src/commands/providers.rs`)

- `get_provider_presets()` — returns `Vec<ProviderPresetInfo>` (mirrors `ProviderPreset`)
- `get_providers()` — returns `Vec<Provider>` with masked API keys
- `add_provider(provider)` — stores API key in keychain, pushes to `Preferences.providers`
- `update_provider(provider)` — updates in-place, handles empty api_key = keep existing
- `fetch_provider_models(provider)` — resolves masked key, calls `CloudProvider::list_models()`

### Frontend

- `ProviderForm.vue` — hardcoded fields: kind selector, name, URL (custom only), API key, capabilities, insecure toggle
- `types.ts` — `ProviderPresetInfo` and `Provider` interfaces
- `engines.ts` store — `providerPresets` ref, fetched from backend

### Provider backends

`CloudProvider` trait methods receive `&Provider` — backends access `provider.api_key`, `provider.base_url()`, etc. Extra fields would be accessed via a new `provider.extra` map.

## Design

### Phase 1: Type definitions

#### 1.1 — `PresetField` struct in `jona-types/src/provider.rs`

```rust
/// Type of form field for a preset's extra parameter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldType {
    Text,
    Password,
    Select,
}

/// A custom field defined by a provider preset.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresetField {
    /// Unique field identifier (e.g. "region", "access_key").
    pub id: &'static str,
    /// Display label (English — frontend i18n key derived as `provider.field.{id}`).
    pub label: &'static str,
    /// Field type controls rendering and masking behavior.
    pub field_type: FieldType,
    /// Whether the field must be non-empty to save.
    pub required: bool,
    /// Placeholder text shown in the input.
    pub placeholder: &'static str,
    /// Default value (empty string if none).
    pub default_value: &'static str,
    /// For Select fields: available options as `(value, label)` pairs.
    pub options: &'static [(&'static str, &'static str)],
    /// Whether the value is sensitive (stored in keychain, masked in IPC).
    pub sensitive: bool,
}
```

Key decisions:
- `&'static str` and `&'static [...]` for zero-allocation preset registration (matches existing pattern)
- `sensitive: bool` flag — sensitive extra fields (like AWS secret_key) go to keychain alongside `api_key`
- `options` only used when `field_type == Select`, empty slice otherwise
- Labels are English fallback; frontend uses i18n key `provider.field.{id}` if available

#### 1.2 — Extend `ProviderPreset`

```rust
pub struct ProviderPreset {
    // ... existing fields ...
    /// Additional fields this preset needs beyond name/apiKey/url.
    pub extra_fields: &'static [PresetField],
    /// Default field IDs to hide (e.g. &["base_url"] for providers with fixed endpoints).
    pub hidden_fields: &'static [&'static str],
}
```

Hideable field IDs: `"base_url"`, `"api_key"`, `"name"` (unlikely but possible).

#### 1.3 — Extend `Provider`

```rust
pub struct Provider {
    // ... existing fields ...
    /// Extra field values from preset-specific fields.
    #[serde(default)]
    pub extra: HashMap<String, String>,
}
```

This is a flat `HashMap<String, String>`. All values are strings (even select values). Simple, serializable, no nested structures.

### Phase 2: Storage and security

#### 2.1 — Keychain storage for sensitive extra fields

In `commands/providers.rs::add_provider()` and `update_provider()`:

```rust
// For each extra field marked sensitive in the preset:
if let Some(preset) = jona_provider::preset(&provider.kind) {
    for field in preset.extra_fields {
        if field.sensitive {
            if let Some(value) = provider.extra.get(field.id) {
                if !value.is_empty() {
                    keyring_store_extra(&provider.id, field.id, value);
                }
            }
        }
    }
}
```

New keyring helpers:
- `keyring_store_extra(provider_id, field_id, value)` — service `"JonaWhisper"`, user `"provider:{provider_id}:extra:{field_id}"`
- `keyring_load_extra(provider_id, field_id) -> String`
- `keyring_delete_extra(provider_id, field_id)`

#### 2.2 — Preferences save stripping

In `Preferences::save()`, the existing loop clears `api_key`. Extend it:

```rust
for provider in &mut prefs_for_disk.providers {
    provider.api_key.clear();
    // Strip sensitive extra fields
    if let Some(preset) = jona_provider::preset(&provider.kind) {
        for field in preset.extra_fields {
            if field.sensitive {
                provider.extra.remove(field.id);
            }
        }
    }
}
```

#### 2.3 — Preferences load hydration

In `Preferences::load()` (or wherever providers are loaded from disk), hydrate from keychain:

```rust
for provider in &mut prefs.providers {
    provider.api_key = keyring_load(&provider.id);
    if let Some(preset) = jona_provider::preset(&provider.kind) {
        for field in preset.extra_fields {
            if field.sensitive {
                let val = keyring_load_extra(&provider.id, field.id);
                if !val.is_empty() {
                    provider.extra.insert(field.id.to_string(), val);
                }
            }
        }
    }
}
```

### Phase 3: IPC updates

#### 3.1 — `ProviderPresetInfo` extension

```rust
#[derive(Serialize)]
pub struct PresetFieldInfo {
    pub id: String,
    pub label: String,
    pub field_type: String,  // "text" | "password" | "select"
    pub required: bool,
    pub placeholder: String,
    pub default_value: String,
    pub options: Vec<(String, String)>,
    pub sensitive: bool,
}

pub struct ProviderPresetInfo {
    // ... existing fields ...
    pub extra_fields: Vec<PresetFieldInfo>,
    pub hidden_fields: Vec<String>,
}
```

#### 3.2 — `get_providers()` masking

Extend the existing masking loop to also mask sensitive extra fields:

```rust
providers.into_iter().map(|mut p| {
    p.api_key = p.masked_api_key();
    if let Some(preset) = jona_provider::preset(&p.kind) {
        for field in preset.extra_fields {
            if field.sensitive {
                if let Some(val) = p.extra.get_mut(field.id) {
                    *val = mask_value(val);
                }
            }
        }
    }
    p
}).collect()
```

Extract the masking logic from `Provider::masked_api_key()` into a shared `mask_value(s: &str) -> String` function.

#### 3.3 — `add_provider` / `update_provider`

These already receive a full `Provider` struct. The `extra` HashMap will be deserialized automatically by serde. The only change is adding the keychain store/load logic from Phase 2.

For `update_provider`, handle empty sensitive values (= keep existing) the same way `api_key` is handled:

```rust
if field.sensitive {
    let new_val = provider.extra.get(field.id).map(|s| s.as_str()).unwrap_or("");
    if new_val.is_empty() || new_val.starts_with('\u{2022}') {
        // Keep existing value
        if let Some(existing_val) = existing.extra.get(field.id) {
            provider.extra.insert(field.id.to_string(), existing_val.clone());
        }
    } else {
        keyring_store_extra(&provider.id, field.id, new_val);
    }
}
```

#### 3.4 — `remove_provider`

Delete extra sensitive fields from keychain:

```rust
if let Some(preset) = jona_provider::preset(&id) {
    for field in preset.extra_fields {
        if field.sensitive {
            keyring_delete_extra(&id, field.id);
        }
    }
}
```

### Phase 4: Frontend

#### 4.1 — TypeScript types (`src/stores/types.ts`)

```typescript
export interface PresetFieldInfo {
  id: string
  label: string
  field_type: 'text' | 'password' | 'select'
  required: boolean
  placeholder: string
  default_value: string
  options: [string, string][]  // [value, label] pairs
  sensitive: boolean
}

export interface ProviderPresetInfo {
  // ... existing fields ...
  extra_fields: PresetFieldInfo[]
  hidden_fields: string[]
}

export interface Provider {
  // ... existing fields ...
  extra: Record<string, string>
}
```

#### 4.2 — `ProviderForm.vue` changes

**New reactive state:**

```typescript
const extraValues = ref<Record<string, string>>({})

// Initialize from existing provider or preset defaults
watch(kind, (newKind) => {
  // ... existing logic ...
  const preset = engines.providerPresets.find(p => p.id === newKind)
  if (preset) {
    extraValues.value = {}
    for (const field of preset.extra_fields) {
      extraValues.value[field.id] = field.default_value
    }
  }
}, { immediate: true })

// When editing, populate from provider.extra
if (props.provider) {
  extraValues.value = { ...props.provider.extra }
}
```

**Computed helpers:**

```typescript
const currentPreset = computed(() =>
  engines.providerPresets.find(p => p.id === kind.value)
)

const visibleExtraFields = computed(() =>
  currentPreset.value?.extra_fields ?? []
)

const showUrl = computed(() => {
  if (kind.value === 'custom') return true
  return !(currentPreset.value?.hidden_fields?.includes('base_url'))
})

const showApiKey = computed(() =>
  !(currentPreset.value?.hidden_fields?.includes('api_key'))
)
```

**Dynamic field rendering (in template, after API key section):**

```html
<!-- Dynamic preset fields -->
<div v-for="field in visibleExtraFields" :key="field.id" class="space-y-2">
  <Label class="text-xs text-muted-foreground">
    {{ t(`provider.field.${field.id}`, field.label) }}
    <span v-if="field.required" class="text-destructive">*</span>
  </Label>

  <!-- Text / Password input -->
  <Input
    v-if="field.field_type !== 'select'"
    v-model="extraValues[field.id]"
    :type="field.field_type"
    :placeholder="field.placeholder"
    class="h-9 text-sm"
  />

  <!-- Select dropdown -->
  <Select
    v-else
    :model-value="extraValues[field.id]"
    @update:model-value="v => extraValues[field.id] = String(v)"
  >
    <SelectTrigger class="w-full h-9 text-sm">
      <SelectValue />
    </SelectTrigger>
    <SelectContent class="max-h-[45vh]">
      <SelectItem
        v-for="[value, label] in field.options"
        :key="value"
        :value="value"
      >{{ label }}</SelectItem>
    </SelectContent>
  </Select>

  <p v-if="errors[field.id]" class="text-xs text-destructive">{{ errors[field.id] }}</p>
</div>
```

**Validation update:**

```typescript
function validate(): boolean {
  errors.value = {}
  if (!name.value.trim()) errors.value.name = t('validation.required')
  if (showUrl.value && !url.value.trim()) errors.value.url = t('validation.required')
  // Validate required extra fields
  for (const field of visibleExtraFields.value) {
    if (field.required && !(extraValues.value[field.id]?.trim())) {
      errors.value[field.id] = t('validation.required')
    }
  }
  return Object.keys(errors.value).length === 0
}
```

**Save/test — include extra in Provider object:**

```typescript
const provider: Provider = {
  // ... existing fields ...
  extra: { ...extraValues.value },
}
```

#### 4.3 — i18n keys

Add to both `en.json` and `fr.json`:

```json
{
  "provider": {
    "field": {
      "region": "Region",
      "access_key": "Access Key",
      "secret_key": "Secret Key",
      "api_version": "API Version",
      "deployment_name": "Deployment Name"
    }
  }
}
```

### Phase 5: Backend consumption

#### 5.1 — `Provider` helper methods

Add convenience accessors to `Provider`:

```rust
impl Provider {
    /// Get an extra field value by ID.
    pub fn extra(&self, field_id: &str) -> &str {
        self.extra.get(field_id).map(|s| s.as_str()).unwrap_or("")
    }
}
```

#### 5.2 — Usage in provider crate backends

Provider backends already receive `&Provider`. They access extra fields via `provider.extra("region")`:

```rust
// Example: Azure backend
fn transcribe(&self, provider: &Provider, model: &str, audio_path: &Path, language: &str)
    -> Result<TranscriptionResult, ProviderError>
{
    let region = provider.extra("region");
    if region.is_empty() {
        return Err(ProviderError::NotConfigured("Azure region is required".into()));
    }
    let url = format!("https://{}.stt.speech.microsoft.com/...", region);
    // Use Ocp-Apim-Subscription-Key header instead of Bearer
    // ...
}
```

### Phase 6: Existing preset migration

All existing presets need the two new fields. Since the default is "no extra fields, no hidden fields", this is a trivial update:

```rust
inventory::submit! { ProviderPreset {
    id: "openai",
    // ... existing fields ...
    extra_fields: &[],
    hidden_fields: &[],
}}
```

For Copilot (hide base_url):
```rust
inventory::submit! { ProviderPreset {
    id: "copilot",
    // ...
    extra_fields: &[],
    hidden_fields: &["base_url"],
}}
```

For Gemini ASR (hide base_url):
```rust
inventory::submit! { ProviderPreset {
    id: "gemini-asr",
    // ...
    extra_fields: &[],
    hidden_fields: &["base_url"],
}}
```

### Phase 7: Concrete examples

#### Azure Speech (future crate)

```rust
inventory::submit! { ProviderPreset {
    id: "azure-speech",
    display_name: "Azure Speech",
    base_url: "",
    backend_id: "azure-speech",
    supports_asr: true,
    supports_llm: false,
    gradient: "linear-gradient(135deg, #0078d4, #005a9e)",
    default_asr_models: &["whisper"],
    default_llm_models: &[],
    extra_fields: &[
        PresetField {
            id: "region",
            label: "Region",
            field_type: FieldType::Select,
            required: true,
            placeholder: "",
            default_value: "westeurope",
            options: &[
                ("westeurope", "West Europe"),
                ("eastus", "East US"),
                ("westus2", "West US 2"),
                ("southeastasia", "Southeast Asia"),
                // ... more regions
            ],
            sensitive: false,
        },
    ],
    hidden_fields: &["base_url"],  // URL is constructed from region
}}
```

#### AWS Transcribe (future crate)

```rust
inventory::submit! { ProviderPreset {
    id: "aws-transcribe",
    display_name: "AWS Transcribe",
    base_url: "",
    backend_id: "aws-transcribe",
    supports_asr: true,
    supports_llm: false,
    gradient: "linear-gradient(135deg, #ff9900, #e68a00)",
    default_asr_models: &[],
    default_llm_models: &[],
    extra_fields: &[
        PresetField {
            id: "access_key",
            label: "Access Key ID",
            field_type: FieldType::Text,
            required: true,
            placeholder: "AKIAIOSFODNN7EXAMPLE",
            default_value: "",
            options: &[],
            sensitive: false,
        },
        PresetField {
            id: "secret_key",
            label: "Secret Access Key",
            field_type: FieldType::Password,
            required: true,
            placeholder: "",
            default_value: "",
            options: &[],
            sensitive: true,  // stored in keychain
        },
        PresetField {
            id: "region",
            label: "Region",
            field_type: FieldType::Select,
            required: true,
            placeholder: "",
            default_value: "us-east-1",
            options: &[
                ("us-east-1", "US East (N. Virginia)"),
                ("eu-west-1", "EU (Ireland)"),
                ("ap-southeast-1", "Asia Pacific (Singapore)"),
            ],
            sensitive: false,
        },
    ],
    hidden_fields: &["base_url", "api_key"],  // AWS uses access_key + secret_key, not api_key
}}
```

## File changes summary

| File | Change |
|------|--------|
| `src-tauri/crates/jona-types/src/provider.rs` | Add `PresetField`, `FieldType`, extend `ProviderPreset` |
| `src-tauri/crates/jona-types/src/lib.rs` | Add `extra: HashMap<String, String>` to `Provider`, add `Provider::extra()` helper |
| `src-tauri/src/commands/providers.rs` | Extend `ProviderPresetInfo`, handle sensitive extra field keychain ops, mask extra sensitive values |
| `src-tauri/src/state.rs` | Add `keyring_store_extra`, `keyring_load_extra`, `keyring_delete_extra` helpers |
| `src-tauri/crates/jona-types/src/lib.rs` (`Preferences::save`) | Strip sensitive extra fields before writing to disk |
| All 9 provider crate `lib.rs` files | Add `extra_fields: &[], hidden_fields: &[]` to each `ProviderPreset` registration |
| `src/stores/types.ts` | Add `PresetFieldInfo` interface, extend `ProviderPresetInfo` and `Provider` |
| `src/components/ProviderForm.vue` | Dynamic field rendering, validation, extraValues state |
| `src/locales/en.json` | Add `provider.field.*` keys |
| `src/locales/fr.json` | Add `provider.field.*` keys |

## Implementation order

1. **Types first** — `PresetField`, `FieldType` in `jona-types/src/provider.rs`
2. **Extend `ProviderPreset`** — add the two new fields
3. **Extend `Provider`** — add `extra` HashMap + helper
4. **Update all 9 existing preset registrations** — `extra_fields: &[], hidden_fields: &[]`
5. **Keychain helpers** — `keyring_store_extra`, `keyring_load_extra`, `keyring_delete_extra`
6. **IPC updates** — `ProviderPresetInfo`, `get_providers` masking, `add/update/remove_provider` keychain logic, `Preferences::save` stripping
7. **Frontend types** — TypeScript interfaces
8. **ProviderForm.vue** — dynamic rendering + validation
9. **i18n** — English + French field labels
10. **Verify** — `cargo check`, `vue-tsc`, manual test with existing providers (no regressions)

## Backward compatibility

- Existing `preferences.json` files have no `extra` field on providers. `#[serde(default)]` on `HashMap` deserializes as empty map. No migration needed.
- Existing presets compile with `extra_fields: &[], hidden_fields: &[]` — no behavioral change until a preset actually defines fields.
- Frontend gracefully handles empty `extra_fields` array — the `v-for` loop renders nothing.
- `Provider::extra("nonexistent")` returns `""` — safe fallback for backends.

## Testing

- Unit tests for `PresetField` serialization roundtrip
- Unit test: `Provider::extra()` returns empty string for missing keys
- Unit test: sensitive extra fields are stripped in `Preferences::save()`
- Unit test: `mask_value()` works for various lengths
- Frontend: manual test with a mock preset that has extra fields (can add a `#[cfg(debug_assertions)]` test preset)
- Integration: verify existing providers still work unchanged after the refactor
