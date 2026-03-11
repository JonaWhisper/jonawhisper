use crate::errors::AppError;
use crate::events;
use crate::state::{AppState, Provider, mask_value, keyring_store_extra, keyring_delete_extra};
use std::sync::Arc;
use tauri::{AppHandle, Emitter};

/// Sentinel value: when a sensitive extra field is set to this, the field is
/// explicitly cleared (deleted from keychain and removed from `extra`).
const CLEAR_SENTINEL: &str = "__CLEAR__";

#[tauri::command]
pub fn add_provider(mut provider: Provider, state: tauri::State<'_, Arc<AppState>>, app: AppHandle) {
    // Store API key in OS keychain, not in preferences file
    crate::state::keyring_store(&provider.id, &provider.api_key);
    // Store sensitive extra fields in keychain
    if let Some(preset) = jona_provider::preset(&provider.kind) {
        for field in preset.extra_fields {
            if field.sensitive {
                if let Some(value) = provider.extra.get(field.id) {
                    if value == CLEAR_SENTINEL {
                        // New provider — nothing in keychain yet, just remove from extra map
                        provider.extra.remove(field.id);
                    } else if !value.is_empty() {
                        keyring_store_extra(&provider.id, field.id, value);
                    }
                }
            }
        }
    }
    state.settings.lock().unwrap().providers.push(provider);
    state.save_preferences();
    let _ = app.emit(events::SETTINGS_CHANGED, "providers");
}

#[tauri::command]
pub fn remove_provider(id: String, state: tauri::State<'_, Arc<AppState>>, app: AppHandle) {
    // Delete sensitive extra fields from keychain
    let kind = state.settings.lock().unwrap().providers.iter()
        .find(|p| p.id == id).map(|p| p.kind.clone());
    if let Some(kind) = kind {
        if let Some(preset) = jona_provider::preset(&kind) {
            for field in preset.extra_fields {
                if field.sensitive {
                    keyring_delete_extra(&id, field.id);
                }
            }
        }
    }
    crate::state::keyring_delete(&id);
    state.settings.lock().unwrap().providers.retain(|p| p.id != id);
    state.save_preferences();
    let _ = app.emit(events::SETTINGS_CHANGED, "providers");
}

#[tauri::command]
pub fn update_provider(mut provider: Provider, state: tauri::State<'_, Arc<AppState>>, app: AppHandle) {
    let mut s = state.settings.lock().unwrap();
    if let Some(existing) = s.providers.iter_mut().find(|p| p.id == provider.id) {
        if provider.api_key.is_empty() {
            // Empty api_key from frontend means "keep existing key"
            provider.api_key = existing.api_key.clone();
        } else {
            // New key provided — update keychain
            crate::state::keyring_store(&provider.id, &provider.api_key);
        }
        // Handle sensitive extra fields (same pattern as api_key above):
        // - CLEAR_SENTINEL → delete from keychain and remove from extra
        // - empty or masked (••••) → keep existing value (intentional: both mean "no change")
        // - anything else → store new value in keychain
        if let Some(preset) = jona_provider::preset(&provider.kind) {
            for field in preset.extra_fields {
                if field.sensitive {
                    let new_val = provider.extra.get(field.id).map(|s| s.as_str()).unwrap_or("");
                    if new_val == CLEAR_SENTINEL {
                        keyring_delete_extra(&provider.id, field.id);
                        provider.extra.remove(field.id);
                    } else if new_val.is_empty() || new_val.starts_with('\u{2022}') {
                        // Keep existing value
                        if let Some(existing_val) = existing.extra.get(field.id) {
                            provider.extra.insert(field.id.to_string(), existing_val.clone());
                        }
                    } else {
                        keyring_store_extra(&provider.id, field.id, new_val);
                    }
                }
            }
        }
        *existing = provider;
    }
    drop(s);
    state.save_preferences();
    let _ = app.emit(events::SETTINGS_CHANGED, "providers");
}

#[tauri::command]
pub fn get_providers(state: tauri::State<'_, Arc<AppState>>) -> Vec<Provider> {
    let providers = state.settings.lock().unwrap().providers.clone();
    providers.into_iter().map(|mut p| {
        p.api_key = p.masked_api_key();
        // Resolve capabilities and URL from preset for known providers
        if let Some(preset) = jona_provider::preset(&p.kind) {
            p.supports_asr = preset.supports_asr;
            p.supports_llm = preset.supports_llm;
            if p.url.is_empty() {
                p.url = preset.base_url.to_string();
            }
            // Mask sensitive extra fields
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
}

#[derive(serde::Serialize)]
pub struct PresetFieldInfo {
    pub id: String,
    pub label: String,
    pub field_type: String,
    pub required: bool,
    pub placeholder: String,
    pub default_value: String,
    pub options: Vec<(String, String)>,
    pub sensitive: bool,
}

#[derive(serde::Serialize)]
pub struct ProviderPresetInfo {
    pub id: String,
    pub display_name: String,
    pub base_url: String,
    pub supports_asr: bool,
    pub supports_llm: bool,
    pub gradient: String,
    pub default_asr_models: Vec<String>,
    pub default_llm_models: Vec<String>,
    pub extra_fields: Vec<PresetFieldInfo>,
    pub hidden_fields: Vec<String>,
}

fn field_type_str(ft: jona_types::FieldType) -> String {
    match ft {
        jona_types::FieldType::Text => "text".to_string(),
        jona_types::FieldType::Password => "password".to_string(),
        jona_types::FieldType::Select => "select".to_string(),
    }
}

#[tauri::command]
pub fn get_provider_presets() -> Vec<ProviderPresetInfo> {
    jona_provider::presets().iter().map(|p| ProviderPresetInfo {
        id: p.id.to_string(),
        display_name: p.display_name.to_string(),
        base_url: p.base_url.to_string(),
        supports_asr: p.supports_asr,
        supports_llm: p.supports_llm,
        gradient: p.gradient.to_string(),
        default_asr_models: p.default_asr_models.iter().map(|s| s.to_string()).collect(),
        default_llm_models: p.default_llm_models.iter().map(|s| s.to_string()).collect(),
        extra_fields: p.extra_fields.iter().map(|f| PresetFieldInfo {
            id: f.id.to_string(),
            label: f.label.to_string(),
            field_type: field_type_str(f.field_type),
            required: f.required,
            placeholder: f.placeholder.to_string(),
            default_value: f.default_value.to_string(),
            options: f.options.iter().map(|(v, l)| (v.to_string(), l.to_string())).collect(),
            sensitive: f.sensitive,
        }).collect(),
        hidden_fields: p.hidden_fields.iter().map(|s| s.to_string()).collect(),
    }).collect()
}

#[tauri::command]
pub async fn fetch_provider_models(provider: Provider, state: tauri::State<'_, Arc<AppState>>) -> Result<Vec<String>, AppError> {
    provider.validate_url().map_err(|e| AppError::Other(e.to_string()))?;

    // If api_key is masked (editing mode), use the stored key
    let mut resolved = provider.clone();
    if resolved.api_key.is_empty() || resolved.api_key.starts_with('\u{2022}') {
        resolved.api_key = state.settings.lock().unwrap().providers.iter()
            .find(|p| p.id == provider.id)
            .map(|p| p.api_key.clone())
            .unwrap_or_default();
    }

    // Hydrate sensitive extra fields:
    // - CLEAR_SENTINEL → leave empty (don't hydrate)
    // - masked/empty → use stored values
    if let Some(preset) = jona_provider::preset(&resolved.kind) {
        let stored_extras = state.settings.lock().unwrap().providers.iter()
            .find(|p| p.id == provider.id)
            .map(|p| p.extra.clone())
            .unwrap_or_default();
        for field in preset.extra_fields {
            if field.sensitive {
                let val = resolved.extra.get(field.id).map(|s| s.as_str()).unwrap_or("");
                if val == CLEAR_SENTINEL {
                    resolved.extra.remove(field.id);
                } else if val.is_empty() || val.starts_with('\u{2022}') {
                    if let Some(stored_val) = stored_extras.get(field.id) {
                        resolved.extra.insert(field.id.to_string(), stored_val.clone());
                    }
                }
            }
        }
    }

    jona_provider::backend_for_provider(&resolved)
        .list_models(&resolved)
        .await
        .map_err(|e| AppError::Other(e.to_string()))
}
