use crate::engines::{self, EngineCatalog, EngineInfo, Language};
use crate::engines::downloader;
use crate::platform;
use crate::state::{ApiServerConfig, AppState, HistoryEntry};
use crate::audio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Manager};

#[tauri::command]
pub fn get_audio_devices() -> Vec<audio::AudioDevice> {
    audio::AudioRecorder::list_devices()
}

#[tauri::command]
pub fn get_engines(state: tauri::State<'_, Arc<AppState>>) -> Vec<EngineInfo> {
    let api_servers = state.api_servers.lock().unwrap().clone();
    let catalog = EngineCatalog::new(&api_servers);
    catalog.engine_infos()
}

#[tauri::command]
pub fn get_models(state: tauri::State<'_, Arc<AppState>>) -> Vec<engines::ASRModel> {
    let api_servers = state.api_servers.lock().unwrap().clone();
    let catalog = EngineCatalog::new(&api_servers);
    catalog.all_models()
}

#[tauri::command]
pub fn get_downloaded_models(state: tauri::State<'_, Arc<AppState>>) -> Vec<engines::ASRModel> {
    let api_servers = state.api_servers.lock().unwrap().clone();
    let catalog = EngineCatalog::new(&api_servers);
    catalog.downloaded_models()
}

#[tauri::command]
pub fn select_model(id: String, state: tauri::State<'_, Arc<AppState>>) {
    *state.selected_model_id.lock().unwrap() = id;
}

#[tauri::command]
pub fn get_selected_model_id(state: tauri::State<'_, Arc<AppState>>) -> String {
    state.selected_model_id.lock().unwrap().clone()
}

#[tauri::command]
pub async fn download_model_cmd(
    app: AppHandle,
    id: String,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<bool, String> {
    let api_servers = state.api_servers.lock().unwrap().clone();
    let catalog = EngineCatalog::new(&api_servers);
    let model = catalog
        .model_by_id(&id)
        .ok_or_else(|| format!("Model not found: {}", id))?;

    let state_clone = Arc::clone(&state);
    Ok(downloader::download_model(app, state_clone, model).await)
}

#[tauri::command]
pub fn delete_model_cmd(id: String, state: tauri::State<'_, Arc<AppState>>) -> bool {
    let api_servers = state.api_servers.lock().unwrap().clone();
    let catalog = EngineCatalog::new(&api_servers);
    if let Some(model) = catalog.model_by_id(&id) {
        downloader::delete_model(&model)
    } else {
        false
    }
}

#[tauri::command]
pub fn get_languages(state: tauri::State<'_, Arc<AppState>>) -> Vec<Language> {
    let api_servers = state.api_servers.lock().unwrap().clone();
    let catalog = EngineCatalog::new(&api_servers);
    catalog.supported_languages()
}

#[tauri::command]
pub fn select_language(code: String, state: tauri::State<'_, Arc<AppState>>) {
    *state.selected_language.lock().unwrap() = code;
}

#[tauri::command]
pub fn get_selected_language(state: tauri::State<'_, Arc<AppState>>) -> String {
    state.selected_language.lock().unwrap().clone()
}

#[tauri::command]
pub fn get_permissions() -> platform::PermissionReport {
    platform::check_permissions()
}

#[tauri::command]
pub fn request_permission(kind: String) -> bool {
    platform::request_permission(&kind)
}

#[tauri::command]
pub fn get_post_processing_enabled(state: tauri::State<'_, Arc<AppState>>) -> bool {
    *state.post_processing_enabled.lock().unwrap()
}

#[tauri::command]
pub fn set_post_processing_enabled(enabled: bool, state: tauri::State<'_, Arc<AppState>>) {
    *state.post_processing_enabled.lock().unwrap() = enabled;
}

#[tauri::command]
pub fn get_hotkey(state: tauri::State<'_, Arc<AppState>>) -> String {
    state.hotkey_option.lock().unwrap().clone()
}

#[tauri::command]
pub fn set_hotkey(hotkey: String, state: tauri::State<'_, Arc<AppState>>) {
    *state.hotkey_option.lock().unwrap() = hotkey;
}

#[tauri::command]
pub fn get_history(state: tauri::State<'_, Arc<AppState>>) -> Vec<HistoryEntry> {
    state.transcription_history.lock().unwrap().clone()
}

#[tauri::command]
pub fn clear_history(state: tauri::State<'_, Arc<AppState>>) {
    state.transcription_history.lock().unwrap().clear();
}

#[tauri::command]
pub fn add_api_server(config: ApiServerConfig, state: tauri::State<'_, Arc<AppState>>) {
    state.api_servers.lock().unwrap().push(config);
}

#[tauri::command]
pub fn remove_api_server(id: String, state: tauri::State<'_, Arc<AppState>>) {
    state.api_servers.lock().unwrap().retain(|s| s.id != id);
}

#[tauri::command]
pub fn get_api_servers(state: tauri::State<'_, Arc<AppState>>) -> Vec<ApiServerConfig> {
    state.api_servers.lock().unwrap().clone()
}

#[tauri::command]
pub fn start_monitoring(app: AppHandle, enabled: tauri::State<'_, Arc<AtomicBool>>) {
    if !enabled.load(Ordering::SeqCst) {
        enabled.store(true, Ordering::SeqCst);
        log::info!("Monitoring enabled by start_monitoring command");
    }
    // Close the setup window from the backend (avoids app exit on last window close)
    if let Some(win) = app.get_webview_window("setup") {
        let _ = win.destroy();
    }
}

#[tauri::command]
pub fn get_app_state(state: tauri::State<'_, Arc<AppState>>) -> serde_json::Value {
    serde_json::json!({
        "is_recording": *state.is_recording.lock().unwrap(),
        "is_transcribing": *state.is_transcribing.lock().unwrap(),
        "queue_count": state.queue_count(),
        "downloading_model_id": *state.downloading_model_id.lock().unwrap(),
        "download_progress": *state.download_progress.lock().unwrap(),
        "selected_model_id": *state.selected_model_id.lock().unwrap(),
        "selected_language": *state.selected_language.lock().unwrap(),
        "post_processing_enabled": *state.post_processing_enabled.lock().unwrap(),
        "hotkey": *state.hotkey_option.lock().unwrap(),
    })
}
