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
    let api_servers = state.settings.lock().unwrap().api_servers.clone();
    EngineCatalog::new(&api_servers)
}

// -- Locale --

#[tauri::command]
pub fn get_system_locale(state: tauri::State<'_, Arc<AppState>>) -> String {
    let locale = state.settings.lock().unwrap().app_locale.clone();
    crate::resolve_locale(&locale)
}

// -- Audio --

#[tauri::command]
pub fn get_audio_devices() -> Vec<crate::platform::audio_devices::AudioDevice> {
    audio::list_usable_devices()
}

// -- Engines & Models --

#[tauri::command]
pub fn get_engines(state: tauri::State<'_, Arc<AppState>>) -> Vec<EngineInfo> {
    catalog(&state).engine_infos()
}

#[tauri::command]
pub fn get_models(state: tauri::State<'_, Arc<AppState>>) -> Vec<serde_json::Value> {
    let cat = catalog(&state);
    let language = state.settings.lock().unwrap().selected_language.clone();
    let recommended_ids = cat.recommended_model_ids(&language);
    cat.all_models().into_iter().map(|m| {
        let downloaded = m.is_downloaded();
        let recommended = recommended_ids.contains(&m.id);
        let mut json = serde_json::to_value(&m).unwrap();
        let obj = json.as_object_mut().unwrap();
        obj.insert("is_downloaded".into(), downloaded.into());
        obj.insert("recommended".into(), recommended.into());
        json
    }).collect()
}

#[tauri::command]
pub fn get_downloaded_models(state: tauri::State<'_, Arc<AppState>>) -> Vec<engines::ASRModel> {
    catalog(&state).downloaded_models()
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


// -- Permissions --

#[tauri::command]
pub fn get_permissions() -> platform::PermissionReport {
    platform::check_permissions()
}

#[tauri::command]
pub fn request_permission(kind: String, app: AppHandle) -> bool {
    let result = platform::request_permission(&kind);
    let _ = app.emit(crate::events::PERMISSION_CHANGED, &kind);
    result
}

// -- Settings --

#[tauri::command]
pub fn get_settings(state: tauri::State<'_, Arc<AppState>>) -> serde_json::Value {
    let s = state.settings.lock().unwrap();
    log::info!("get_settings: post_processing={}, hallucination_filter={}, llm_enabled={}",
        s.post_processing_enabled, s.hallucination_filter_enabled, s.llm_config.enabled,
    );
    serde_json::json!({
        "app_locale": s.app_locale,
        "post_processing_enabled": s.post_processing_enabled,
        "hallucination_filter_enabled": s.hallucination_filter_enabled,
        "hotkey": s.hotkey_option,
        "selected_input_device_uid": s.selected_input_device_uid,
        "selected_model_id": s.selected_model_id,
        "selected_language": s.selected_language,
        "cancel_shortcut": s.cancel_shortcut,
        "recording_mode": s.recording_mode,
        "llm_config": {
            "enabled": s.llm_config.enabled,
            "provider": s.llm_config.provider,
            "api_url": s.llm_config.api_url,
            "api_key": s.llm_config.api_key,
            "model": s.llm_config.model,
        },
    })
}

#[tauri::command]
pub fn set_setting(
    key: String,
    value: String,
    state: tauri::State<'_, Arc<AppState>>,
    hotkey_sender: tauri::State<'_, crate::HotkeyUpdateSender>,
    app: AppHandle,
) {
    use crate::platform::hotkey;

    log::info!("set_setting: key={}, value={}", key, value);
    {
        let mut s = state.settings.lock().unwrap();
        match key.as_str() {
            "app_locale" => {
                s.app_locale = value.clone();
                let lang = crate::resolve_locale(&value);
                rust_i18n::set_locale(&lang);
            }
            "post_processing_enabled" => s.post_processing_enabled = value == "true",
            "hallucination_filter_enabled" => s.hallucination_filter_enabled = value == "true",
            "hotkey" => s.hotkey_option = value.clone(),
            "cancel_shortcut" => s.cancel_shortcut = value.clone(),
            "recording_mode" => s.recording_mode = value.clone(),
            "selected_input_device_uid" => {
                s.selected_input_device_uid = if value.is_empty() { None } else { Some(value.clone()) };
            }
            "selected_model_id" => s.selected_model_id = value.clone(),
            "selected_language" => s.selected_language = value.clone(),
            _ => {
                log::warn!("Unknown setting key: {}", key);
                return;
            }
        }
    }
    // Send hotkey updates outside the settings lock
    match key.as_str() {
        "hotkey" => {
            let shortcut = hotkey::Shortcut::parse(&value);
            let _ = hotkey_sender.0.send(hotkey::HotkeyUpdate::SetRecordShortcut(shortcut));
        }
        "cancel_shortcut" => {
            let shortcut = hotkey::Shortcut::parse(&value);
            let _ = hotkey_sender.0.send(hotkey::HotkeyUpdate::SetCancelShortcut(shortcut));
        }
        _ => {}
    }
    state.save_preferences();
    if key == "app_locale" {
        crate::tray::update_tray_labels(&app);
    }
    let _ = app.emit(crate::events::SETTINGS_CHANGED, &key);
}

// -- Mic Test --

#[tauri::command]
pub fn start_mic_test(state: tauri::State<'_, Arc<AppState>>, sender: tauri::State<'_, crate::recording::MicTestSender>) {
    let device_uid = state.settings.lock().unwrap().selected_input_device_uid.clone();
    state.runtime.lock().unwrap().mic_testing = true;
    let _ = sender.0.send(crate::recording::AudioCmd::StartMicTest { device_uid });
}

#[tauri::command]
pub fn stop_mic_test(state: tauri::State<'_, Arc<AppState>>, sender: tauri::State<'_, crate::recording::MicTestSender>) {
    state.runtime.lock().unwrap().mic_testing = false;
    let _ = sender.0.send(crate::recording::AudioCmd::StopMicTest);
}

// -- LLM Config --

#[tauri::command]
pub fn set_llm_config(
    config: crate::state::LlmConfig,
    state: tauri::State<'_, Arc<AppState>>,
    app: AppHandle,
) {
    state.settings.lock().unwrap().llm_config = config;
    state.save_preferences();
    let _ = app.emit(crate::events::SETTINGS_CHANGED, "llm_config");
}

// -- History --

#[tauri::command]
pub fn get_history(state: tauri::State<'_, Arc<AppState>>) -> Vec<HistoryEntry> {
    state.get_history()
}

#[tauri::command]
pub fn search_history(query: String, state: tauri::State<'_, Arc<AppState>>) -> Vec<HistoryEntry> {
    if query.is_empty() {
        return state.get_history();
    }
    state.search_history(&query)
}

#[tauri::command]
pub fn delete_history_entry(timestamp: u64, state: tauri::State<'_, Arc<AppState>>) {
    state.delete_history_entry(timestamp);
}

#[tauri::command]
pub fn delete_history_day(day_timestamp: u64, state: tauri::State<'_, Arc<AppState>>) {
    state.delete_history_day(day_timestamp);
}

#[tauri::command]
pub fn clear_history(state: tauri::State<'_, Arc<AppState>>) {
    state.clear_history();
}

// -- API Servers --

#[tauri::command]
pub fn add_api_server(config: ApiServerConfig, state: tauri::State<'_, Arc<AppState>>) {
    state.settings.lock().unwrap().api_servers.push(config);
    state.save_preferences();
}

#[tauri::command]
pub fn remove_api_server(id: String, state: tauri::State<'_, Arc<AppState>>) {
    state.settings.lock().unwrap().api_servers.retain(|s| s.id != id);
    state.save_preferences();
}

#[tauri::command]
pub fn get_api_servers(state: tauri::State<'_, Arc<AppState>>) -> Vec<ApiServerConfig> {
    state.settings.lock().unwrap().api_servers.clone()
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
    let selected_id = state.settings.lock().unwrap().selected_model_id.clone();
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

// -- Shortcut capture --

#[tauri::command]
pub fn start_shortcut_capture(hotkey_sender: tauri::State<'_, crate::HotkeyUpdateSender>) {
    let _ = hotkey_sender.0.send(crate::platform::hotkey::HotkeyUpdate::EnterCaptureMode);
}

#[tauri::command]
pub fn stop_shortcut_capture(hotkey_sender: tauri::State<'_, crate::HotkeyUpdateSender>) {
    let _ = hotkey_sender.0.send(crate::platform::hotkey::HotkeyUpdate::ExitCaptureMode);
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
        let _ = app.emit(crate::events::PILL_MODE, "recording");
        let _ = app.emit(crate::events::RECORDING_STARTED, ());

        // Fake spectrum data for 2 seconds (30fps)
        for frame in 0..60u32 {
            let spectrum: Vec<f32> = (0..12)
                .map(|i| {
                    let phase = (frame as f32 * 0.15) + (i as f32 * 0.5);
                    (phase.sin() * 0.5 + 0.5) * 0.8
                })
                .collect();
            let _ = app.emit(crate::events::SPECTRUM_DATA, &spectrum);
            tokio::time::sleep(Duration::from_millis(33)).await;
        }

        let _ = app.emit(crate::events::RECORDING_STOPPED, serde_json::json!({ "queue_count": rounds - round }));
        let _ = app.emit(crate::events::PILL_MODE, "transcribing");
        let _ = app.emit(crate::events::TRANSCRIPTION_STARTED, serde_json::json!({ "queue_count": rounds - round - 1 }));

        tokio::time::sleep(Duration::from_millis(2000)).await;

        let _ = app.emit(crate::events::TRANSCRIPTION_COMPLETE, serde_json::json!({ "text": format!("Simulation round {}", round + 1) }));

        if round < rounds - 1 {
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    }

    let _ = app.emit(crate::events::PILL_MODE, "error");
    tokio::time::sleep(Duration::from_millis(800)).await;

    crate::tray::close_pill_window(&app);
    log::info!("Simulation complete");
}
