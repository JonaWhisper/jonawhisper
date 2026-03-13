use crate::errors::AppError;
use crate::events;
use crate::state::{AppState, Provider, mask_value, keyring_store, keyring_delete,
                    keyring_store_extra, keyring_delete_extra};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};

#[tauri::command]
pub fn open_provider_form_window(app: AppHandle, provider_id: Option<String>) {
    let title = if provider_id.is_some() {
        rust_i18n::t!("provider.editTitle")
    } else {
        rust_i18n::t!("settings.providers.add")
    };
    let url = match &provider_id {
        Some(id) => format!("/provider-form?id={}", id),
        None => "/provider-form".to_string(),
    };
    // Close any existing provider-form window so it reopens with the new URL/provider
    if let Some(window) = app.get_webview_window("provider-form") {
        let _ = window.destroy();
    }
    crate::ui::tray::open_fixed_window(&app, "provider-form", &title, &url, 420.0, 550.0);
}

/// Sentinel value: when a sensitive extra field is set to this, the field is
/// explicitly cleared (deleted from keychain and removed from `extra`).
/// Uses a null-byte prefix so it cannot collide with any user-typed value
/// (HTML input fields cannot contain null bytes).
///
/// Field handling pattern (shared by add/update/fetch):
/// - `""` or matches masked value → keep existing
/// - `CLEAR_SENTINEL` → delete from keychain + remove from extra
/// - anything else → new value, store in keychain
const CLEAR_SENTINEL: &str = "\0CLEAR";

/// Store sensitive extra fields in keychain for a new provider.
/// Non-sensitive fields and empty values are left as-is in `provider.extra`.
fn store_sensitive_extras(provider: &mut Provider) {
    let preset = match jona_provider::preset(&provider.kind) {
        Some(p) => p,
        None => return,
    };
    for field in preset.extra_fields.iter().filter(|f| f.sensitive) {
        let val = match provider.extra.get(field.id) {
            Some(v) => v.clone(),
            None => continue,
        };
        if val == CLEAR_SENTINEL {
            provider.extra.remove(field.id);
        } else if !val.is_empty() {
            keyring_store_extra(&provider.id, field.id, &val);
        }
    }
}

/// Update sensitive extra fields on an existing provider.
/// Compares incoming values against stored ones to decide: keep / store new / delete.
fn update_sensitive_extras(provider: &mut Provider, existing: &Provider) {
    let preset = match jona_provider::preset(&provider.kind) {
        Some(p) => p,
        None => return,
    };
    for field in preset.extra_fields.iter().filter(|f| f.sensitive) {
        let new_val = provider.extra.get(field.id).map(|s| s.as_str()).unwrap_or("");
        if new_val == CLEAR_SENTINEL {
            keyring_delete_extra(&provider.id, field.id);
            provider.extra.remove(field.id);
        } else {
            let stored = existing.extra.get(field.id).cloned().unwrap_or_default();
            let masked = mask_value(&stored);
            if new_val.is_empty() || new_val == masked {
                // Keep existing value
                if !stored.is_empty() {
                    provider.extra.insert(field.id.to_string(), stored);
                }
            } else {
                keyring_store_extra(&provider.id, field.id, new_val);
            }
        }
    }
}

/// Hydrate sensitive extra fields from stored values (settings + keychain).
/// Used when the frontend sends masked values that need to be resolved server-side.
fn hydrate_sensitive_extras(provider: &mut Provider, stored_extras: &std::collections::HashMap<String, String>) {
    let preset = match jona_provider::preset(&provider.kind) {
        Some(p) => p,
        None => return,
    };
    for field in preset.extra_fields.iter().filter(|f| f.sensitive) {
        let val = provider.extra.get(field.id).map(|s| s.as_str()).unwrap_or("");
        if val == CLEAR_SENTINEL {
            provider.extra.remove(field.id);
        } else {
            let stored = stored_extras.get(field.id).cloned().unwrap_or_default();
            let masked = mask_value(&stored);
            if (val.is_empty() || val == masked) && !stored.is_empty() {
                provider.extra.insert(field.id.to_string(), stored);
            }
        }
    }
}

/// Delete all sensitive extra fields from keychain for a provider.
fn delete_sensitive_extras(provider_id: &str, kind: &str) {
    if let Some(preset) = jona_provider::preset(kind) {
        for field in preset.extra_fields.iter().filter(|f| f.sensitive) {
            keyring_delete_extra(provider_id, field.id);
        }
    }
}

#[tauri::command]
pub fn add_provider(mut provider: Provider, state: tauri::State<'_, Arc<AppState>>, app: AppHandle) {
    keyring_store(&provider.id, &provider.api_key);
    store_sensitive_extras(&mut provider);
    state.settings.lock().unwrap().providers.push(provider);
    state.save_preferences();
    let _ = app.emit(events::SETTINGS_CHANGED, "providers");
}

#[tauri::command]
pub fn remove_provider(id: String, state: tauri::State<'_, Arc<AppState>>, app: AppHandle) {
    let kind = state.settings.lock().unwrap().providers.iter()
        .find(|p| p.id == id).map(|p| p.kind.clone());
    if let Some(kind) = kind {
        delete_sensitive_extras(&id, &kind);
    }
    keyring_delete(&id);
    state.settings.lock().unwrap().providers.retain(|p| p.id != id);
    state.save_preferences();
    let _ = app.emit(events::SETTINGS_CHANGED, "providers");
}

#[tauri::command]
pub fn update_provider(mut provider: Provider, state: tauri::State<'_, Arc<AppState>>, app: AppHandle) {
    let mut s = state.settings.lock().unwrap();
    if let Some(existing) = s.providers.iter_mut().find(|p| p.id == provider.id) {
        if provider.api_key.is_empty() {
            provider.api_key = existing.api_key.clone();
        } else {
            keyring_store(&provider.id, &provider.api_key);
        }
        update_sensitive_extras(&mut provider, existing);
        *existing = provider;
    }
    drop(s);
    state.save_preferences();
    let _ = app.emit(events::SETTINGS_CHANGED, "providers");
}

fn mask_provider(mut p: Provider) -> Provider {
    p.api_key = p.masked_api_key();
    if let Some(preset) = jona_provider::preset(&p.kind) {
        let has_toggle = |id: &str| preset.extra_fields.iter().any(|f| f.id == id && f.field_type == jona_types::FieldType::Toggle);
        if !has_toggle("supports_asr") { p.supports_asr = preset.supports_asr; }
        if !has_toggle("supports_llm") { p.supports_llm = preset.supports_llm; }
        if p.url.is_empty() {
            p.url = preset.base_url.to_string();
        }
        for field in preset.extra_fields.iter().filter(|f| f.sensitive) {
            if let Some(val) = p.extra.get_mut(field.id) {
                *val = mask_value(val);
            }
        }
    }
    p
}

#[tauri::command]
pub fn get_providers(state: tauri::State<'_, Arc<AppState>>) -> Vec<Provider> {
    let mut result: Vec<Provider> = state.settings.lock().unwrap().providers.clone()
        .into_iter().map(mask_provider).collect();
    // Append auto-detected providers (masked), skipping any whose ID already
    // exists in the manual list to avoid duplicates.
    let manual_ids: std::collections::HashSet<String> = result.iter().map(|p| p.id.clone()).collect();
    let detected = state.detected_providers.lock().unwrap().clone();
    result.extend(detected.into_iter()
        .filter(|p| !manual_ids.contains(&p.id))
        .map(mask_provider));
    result
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
        jona_types::FieldType::Toggle => "toggle".to_string(),
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

    // Resolve real credentials server-side (frontend sends masked keys).
    // For auto-detected providers, use find_provider() which re-reads fresh
    // credentials from the source. For manual providers, hydrate from settings.
    let resolved = if provider.source.is_some() {
        // Auto-detected provider: resolve via find_provider() for fresh credentials
        let mut p = state.find_provider(&provider.id)
            .ok_or_else(|| AppError::Other(format!("Provider '{}' not found", provider.id)))?;
        // Keep URL from the frontend in case it was customized
        if !provider.url.is_empty() {
            p.url = provider.url.clone();
        }
        p
    } else {
        // Manual provider: hydrate credentials from settings/keychain
        let mut resolved = provider.clone();
        let (stored_key, stored_extras) = {
            let s = state.settings.lock().unwrap();
            let stored = s.providers.iter().find(|p| p.id == provider.id);
            (
                stored.map(|p| p.api_key.clone()).unwrap_or_default(),
                stored.map(|p| p.extra.clone()).unwrap_or_default(),
            )
        };

        if resolved.api_key.is_empty() || resolved.api_key == mask_value(&stored_key) {
            resolved.api_key = stored_key;
        }

        hydrate_sensitive_extras(&mut resolved, &stored_extras);
        resolved
    };

    jona_provider::backend_for_provider(&resolved)
        .list_models(&resolved)
        .await
        .map_err(|e| AppError::Other(e.to_string()))
}

#[tauri::command]
pub fn detect_providers(state: tauri::State<'_, Arc<AppState>>, app: AppHandle) {
    state.run_detection();
    let _ = app.emit(events::SETTINGS_CHANGED, "providers");
}

#[tauri::command]
pub fn toggle_provider_enabled(id: String, enabled: bool, state: tauri::State<'_, Arc<AppState>>, app: AppHandle) {
    // Check manual providers first
    {
        let mut s = state.settings.lock().unwrap();
        if let Some(p) = s.providers.iter_mut().find(|p| p.id == id) {
            p.enabled = enabled;
            drop(s);
            state.save_preferences();
            let _ = app.emit(events::SETTINGS_CHANGED, "providers");
            return;
        }
    }
    // Check detected providers
    let mut detected = state.detected_providers.lock().unwrap();
    if let Some(p) = detected.iter_mut().find(|p| p.id == id) {
        p.enabled = enabled;
        drop(detected);
        // Persist enabled state so it survives restarts
        state.settings.lock().unwrap().detected_enabled.insert(id, enabled);
        state.save_preferences();
    }
    let _ = app.emit(events::SETTINGS_CHANGED, "providers");
}
