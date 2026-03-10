use crate::errors::AppError;
use crate::events;
use crate::state::{AppState, Provider};
use std::sync::Arc;
use tauri::{AppHandle, Emitter};

#[tauri::command]
pub fn add_provider(provider: Provider, state: tauri::State<'_, Arc<AppState>>, app: AppHandle) {
    // Store API key in OS keychain, not in preferences file
    crate::state::keyring_store(&provider.id, &provider.api_key);
    state.settings.lock().unwrap().providers.push(provider);
    state.save_preferences();
    let _ = app.emit(events::SETTINGS_CHANGED, "providers");
}

#[tauri::command]
pub fn remove_provider(id: String, state: tauri::State<'_, Arc<AppState>>, app: AppHandle) {
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
        }
        p
    }).collect()
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

    jona_provider::backend_for_provider(&resolved)
        .list_models(&resolved)
        .await
        .map_err(|e| AppError::Other(e.to_string()))
}
