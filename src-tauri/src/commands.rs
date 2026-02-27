use crate::audio;
use crate::engines::downloader;
use crate::engines::{self, EngineCatalog, EngineInfo, Language};
use crate::errors::AppError;
use crate::platform;
use crate::state::{ApiServerConfig, AppState, HistoryEntry};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};

/// Build an engine catalog from current state.
fn catalog(state: &Arc<AppState>) -> EngineCatalog {
    let api_servers = state.api_servers.lock().unwrap().clone();
    EngineCatalog::new(&api_servers)
}

// -- Audio --

#[tauri::command]
pub fn get_audio_devices() -> Vec<audio::AudioDevice> {
    audio::AudioRecorder::list_devices()
}

// -- Engines & Models --

#[tauri::command]
pub fn get_engines(state: tauri::State<'_, Arc<AppState>>) -> Vec<EngineInfo> {
    catalog(&state).engine_infos()
}

#[tauri::command]
pub fn get_models(state: tauri::State<'_, Arc<AppState>>) -> Vec<engines::ASRModel> {
    catalog(&state).all_models()
}

#[tauri::command]
pub fn get_downloaded_models(state: tauri::State<'_, Arc<AppState>>) -> Vec<engines::ASRModel> {
    catalog(&state).downloaded_models()
}

#[tauri::command]
pub fn select_model(id: String, state: tauri::State<'_, Arc<AppState>>) {
    *state.selected_model_id.lock().unwrap() = id;
    state.save_preferences();
}

#[tauri::command]
pub async fn download_model_cmd(
    app: AppHandle,
    id: String,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<bool, AppError> {
    let model = catalog(&state)
        .model_by_id(&id)
        .ok_or_else(|| AppError::Other(format!("Model not found: {}", id)))?;

    Ok(downloader::download_model(app, Arc::clone(&state), model).await)
}

#[tauri::command]
pub fn delete_model_cmd(id: String, state: tauri::State<'_, Arc<AppState>>) -> bool {
    catalog(&state)
        .model_by_id(&id)
        .is_some_and(|m| downloader::delete_model(&m))
}

// -- Language --

#[tauri::command]
pub fn get_languages(state: tauri::State<'_, Arc<AppState>>) -> Vec<Language> {
    catalog(&state).supported_languages()
}

#[tauri::command]
pub fn select_language(code: String, state: tauri::State<'_, Arc<AppState>>) {
    *state.selected_language.lock().unwrap() = code;
    state.save_preferences();
}

// -- Permissions --

#[tauri::command]
pub fn get_permissions() -> platform::PermissionReport {
    platform::check_permissions()
}

#[tauri::command]
pub fn request_permission(kind: String) -> bool {
    platform::request_permission(&kind)
}

// -- Settings --

#[tauri::command]
pub fn get_settings(state: tauri::State<'_, Arc<AppState>>) -> serde_json::Value {
    serde_json::json!({
        "app_locale": *state.app_locale.lock().unwrap(),
        "post_processing_enabled": *state.post_processing_enabled.lock().unwrap(),
        "hallucination_filter_enabled": *state.hallucination_filter_enabled.lock().unwrap(),
        "hotkey": *state.hotkey_option.lock().unwrap(),
        "selected_input_device_uid": *state.selected_input_device_uid.lock().unwrap(),
        "selected_model_id": *state.selected_model_id.lock().unwrap(),
        "selected_language": *state.selected_language.lock().unwrap(),
        "cancel_shortcut": *state.cancel_shortcut.lock().unwrap(),
        "recording_mode": *state.recording_mode.lock().unwrap(),
    })
}

#[tauri::command]
pub fn set_setting(
    key: String,
    value: String,
    state: tauri::State<'_, Arc<AppState>>,
    app: AppHandle,
) {
    match key.as_str() {
        "app_locale" => *state.app_locale.lock().unwrap() = value,
        "post_processing_enabled" => {
            *state.post_processing_enabled.lock().unwrap() = value == "true";
        }
        "hallucination_filter_enabled" => {
            *state.hallucination_filter_enabled.lock().unwrap() = value == "true";
        }
        "hotkey" => *state.hotkey_option.lock().unwrap() = value,
        "cancel_shortcut" => *state.cancel_shortcut.lock().unwrap() = value,
        "recording_mode" => *state.recording_mode.lock().unwrap() = value,
        "selected_input_device_uid" => {
            *state.selected_input_device_uid.lock().unwrap() = if value.is_empty() {
                None
            } else {
                Some(value)
            };
        }
        _ => {
            log::warn!("Unknown setting key: {}", key);
            return;
        }
    }
    state.save_preferences();
    let _ = app.emit("settings-changed", &key);
}

// -- History --

#[tauri::command]
pub fn get_history(state: tauri::State<'_, Arc<AppState>>) -> Vec<HistoryEntry> {
    state.transcription_history.lock().unwrap().clone()
}

#[tauri::command]
pub fn clear_history(state: tauri::State<'_, Arc<AppState>>) {
    state.transcription_history.lock().unwrap().clear();
}

// -- API Servers --

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

// -- App lifecycle --

#[tauri::command]
pub fn start_monitoring(
    app: AppHandle,
    enabled: tauri::State<'_, Arc<AtomicBool>>,
    state: tauri::State<'_, Arc<AppState>>,
) {
    if !enabled.load(Ordering::SeqCst) {
        enabled.store(true, Ordering::SeqCst);
        log::info!("Monitoring enabled");
    }
    if let Some(win) = app.get_webview_window("setup") {
        let _ = win.destroy();
    }

    // Open model manager if no model is ready
    let selected_id = state.selected_model_id.lock().unwrap().clone();
    let model_ready = catalog(&state)
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

#[tauri::command]
pub fn get_app_state(state: tauri::State<'_, Arc<AppState>>) -> serde_json::Value {
    state.to_frontend_json()
}

// -- Debug --

#[tauri::command]
pub async fn simulate_pill_test(app: AppHandle, count: Option<u32>) {
    use std::time::Duration;
    let rounds = count.unwrap_or(2);

    for round in 0..rounds {
        log::info!("Simulation round {}/{}", round + 1, rounds);

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

        let _ = app.emit("recording-stopped", serde_json::json!({ "queue_count": rounds - round }));
        let _ = app.emit("pill-mode", "transcribing");
        let _ = app.emit("transcription-started", serde_json::json!({ "queue_count": rounds - round - 1 }));

        tokio::time::sleep(Duration::from_millis(2000)).await;

        let _ = app.emit("transcription-complete", serde_json::json!({ "text": format!("Simulation round {}", round + 1) }));

        if round < rounds - 1 {
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    }

    let _ = app.emit("pill-mode", "error");
    tokio::time::sleep(Duration::from_millis(800)).await;

    crate::tray::close_pill_window(&app);
    log::info!("Simulation complete");
}
