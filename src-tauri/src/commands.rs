use crate::audio;
use crate::engines::downloader;
use crate::engines::{self, EngineCatalog, EngineInfo, Language};
use crate::errors::AppError;
use crate::platform;
use crate::state::{ApiServerConfig, AppState, HistoryEntry};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};

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
    state.save_preferences();
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
) -> Result<bool, AppError> {
    let api_servers = state.api_servers.lock().unwrap().clone();
    let catalog = EngineCatalog::new(&api_servers);
    let model = catalog
        .model_by_id(&id)
        .ok_or_else(|| AppError::Other(format!("Model not found: {}", id)))?;

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
    state.save_preferences();
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
    state.save_preferences();
}

#[tauri::command]
pub fn get_hotkey(state: tauri::State<'_, Arc<AppState>>) -> String {
    state.hotkey_option.lock().unwrap().clone()
}

#[tauri::command]
pub fn set_hotkey(hotkey: String, state: tauri::State<'_, Arc<AppState>>) {
    *state.hotkey_option.lock().unwrap() = hotkey;
    state.save_preferences();
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
    state.save_preferences();
}

#[tauri::command]
pub fn remove_api_server(id: String, state: tauri::State<'_, Arc<AppState>>) {
    state.api_servers.lock().unwrap().retain(|s| s.id != id);
    state.save_preferences();
}

#[tauri::command]
pub fn get_api_servers(state: tauri::State<'_, Arc<AppState>>) -> Vec<ApiServerConfig> {
    state.api_servers.lock().unwrap().clone()
}

#[tauri::command]
pub fn start_monitoring(
    app: AppHandle,
    enabled: tauri::State<'_, Arc<AtomicBool>>,
    state: tauri::State<'_, Arc<AppState>>,
) {
    if !enabled.load(Ordering::SeqCst) {
        enabled.store(true, Ordering::SeqCst);
        log::info!("Monitoring enabled by start_monitoring command");
    }
    // Close the setup window from the backend (avoids app exit on last window close)
    if let Some(win) = app.get_webview_window("setup") {
        let _ = win.destroy();
    }

    // If the selected model isn't ready, open model manager so user can download one
    let api_servers = state.api_servers.lock().unwrap().clone();
    let catalog = engines::EngineCatalog::new(&api_servers);
    let selected_id = state.selected_model_id.lock().unwrap().clone();
    let model_ready = catalog
        .model_by_id(&selected_id)
        .is_some_and(|m| m.is_downloaded());

    if !model_ready {
        crate::tray::open_window(
            &app,
            "model-manager",
            "Model Manager",
            "/model-manager",
            700.0,
            500.0,
        );
    }
}

/// Simulate the pill state machine for visual testing.
/// Runs: recording (with fake spectrum) → transcribing → complete → repeat.
#[tauri::command]
pub async fn simulate_pill_test(app: AppHandle, count: Option<u32>) {
    use std::time::Duration;
    let rounds = count.unwrap_or(2);

    for round in 0..rounds {
        log::info!("Simulation round {}/{}", round + 1, rounds);

        // Open pill + recording mode
        crate::tray::open_pill_window(&app);
        tokio::time::sleep(Duration::from_millis(200)).await;
        let _ = app.emit("pill-mode", "recording");
        let _ = app.emit("recording-started", ());

        // Fake spectrum data for 2 seconds (30fps)
        for frame in 0..60u32 {
            let spectrum: Vec<f32> = (0..12)
                .map(|i| {
                    let phase = (frame as f32 * 0.15) + (i as f32 * 0.5);
                    (phase.sin() * 0.5 + 0.5) * 0.8
                })
                .collect();
            let _ = app.emit("spectrum-data", &spectrum);
            tokio::time::sleep(Duration::from_millis(33)).await;
        }

        // Stop recording → transcribing
        let _ = app.emit("recording-stopped", serde_json::json!({ "queue_count": rounds - round }));
        let _ = app.emit("pill-mode", "transcribing");
        let _ = app.emit("transcription-started", serde_json::json!({ "queue_count": rounds - round - 1 }));

        // Transcribing dots for 2 seconds
        tokio::time::sleep(Duration::from_millis(2000)).await;

        // Complete
        let _ = app.emit("transcription-complete", serde_json::json!({ "text": format!("Simulation round {}", round + 1) }));

        if round < rounds - 1 {
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    }

    // Show error state briefly
    let _ = app.emit("pill-mode", "error");
    tokio::time::sleep(Duration::from_millis(800)).await;

    // Close pill
    crate::tray::close_pill_window(&app);
    log::info!("Simulation complete");
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
