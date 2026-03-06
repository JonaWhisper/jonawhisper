use crate::audio;
use crate::engines::downloader;
use crate::engines::{self, EngineCatalog, EngineInfo, Language};
use crate::errors::AppError;
use crate::events;
use crate::platform;
use crate::state::{AppState, HistoryEntry, Provider};
use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, LazyLock};
use tauri::{AppHandle, Emitter, Manager};

static HTTP_CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
});

/// Build the local-only engine catalog.
fn catalog() -> EngineCatalog {
    EngineCatalog::new()
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
pub fn get_engines() -> Vec<EngineInfo> {
    catalog().engine_infos()
}

#[tauri::command]
pub fn get_models(state: tauri::State<'_, Arc<AppState>>) -> Vec<serde_json::Value> {
    let cat = catalog();
    let language = state.settings.lock().unwrap().selected_language.clone();
    let recommended_ids = cat.recommended_model_ids(&language);
    cat.all_models().into_iter().map(|m| {
        let downloaded = m.is_downloaded();
        let recommended = recommended_ids.contains(&m.id);
        let partial = if downloaded { None } else { downloader::partial_progress(&m) };
        let mut json = serde_json::to_value(&m).unwrap();
        let obj = json.as_object_mut().unwrap();
        obj.insert("is_downloaded".into(), downloaded.into());
        obj.insert("recommended".into(), recommended.into());
        obj.insert("partial_progress".into(), serde_json::json!(partial));
        json
    }).collect()
}

#[tauri::command]
pub fn get_downloaded_models() -> Vec<engines::ASRModel> {
    catalog().downloaded_models()
}


#[tauri::command]
pub async fn download_model_cmd(
    app: AppHandle,
    id: String,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<bool, AppError> {
    let model = catalog()
        .model_by_id(&id)
        .ok_or_else(|| AppError::Other(format!("Model not found: {}", id)))?;

    let result = downloader::download_model(app.clone(), Arc::clone(&state.download), model).await;
    let _ = app.emit(events::MODELS_CHANGED, ());
    Ok(result)
}

#[tauri::command]
pub async fn delete_model_cmd(app: AppHandle, id: String) -> bool {
    let result = tokio::task::spawn_blocking(move || {
        catalog()
            .model_by_id(&id)
            .is_some_and(|m| downloader::delete_model(&m))
    }).await.unwrap_or(false);
    let _ = app.emit(events::MODELS_CHANGED, ());
    result
}

#[tauri::command]
pub fn pause_download(id: String, state: tauri::State<'_, Arc<AppState>>) {
    let dl = state.download.lock().unwrap();
    if let Some(entry) = dl.active.get(&id) {
        entry.cancel_requested.store(true, Ordering::SeqCst);
    }
}

#[tauri::command]
pub fn cancel_download(app: AppHandle, id: String, state: tauri::State<'_, Arc<AppState>>) {
    let dl = state.download.lock().unwrap();
    let is_active = dl.active.contains_key(&id);
    if let Some(entry) = dl.active.get(&id) {
        entry.cancel_requested.store(true, Ordering::SeqCst);
        entry.delete_partial.store(true, Ordering::SeqCst);
    }
    drop(dl);

    // Also delete partial directly (handles paused/no-active-download case)
    if !is_active {
        if let Some(model) = catalog().model_by_id(&id) {
            downloader::delete_partial(&model);
        }
        let _ = app.emit(events::MODELS_CHANGED, ());
    }
}

// -- Language --

#[tauri::command]
pub fn get_languages() -> Vec<Language> {
    catalog().supported_languages()
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
    log::info!("get_settings: hallucination_filter={}, text_cleanup_enabled={}, cleanup_model_id={}",
        s.hallucination_filter_enabled, s.text_cleanup_enabled, s.cleanup_model_id,
    );
    serde_json::json!({
        "app_locale": s.app_locale,
        "hallucination_filter_enabled": s.hallucination_filter_enabled,
        "hotkey": s.hotkey_option,
        "selected_input_device_uid": s.selected_input_device_uid,
        "selected_model_id": s.selected_model_id,
        "selected_language": s.selected_language,
        "cancel_shortcut": s.cancel_shortcut,
        "recording_mode": s.recording_mode,
        "text_cleanup_enabled": s.text_cleanup_enabled,
        "cleanup_model_id": s.cleanup_model_id,
        "llm_provider_id": s.llm_provider_id,
        "llm_model": s.llm_model,
        "asr_cloud_model": s.asr_cloud_model,
        "gpu_mode": s.gpu_mode,
        "llm_max_tokens": s.llm_max_tokens,
        "audio_ducking_enabled": s.audio_ducking_enabled,
        "audio_ducking_level": s.audio_ducking_level,
        "vad_enabled": s.vad_enabled,
        "theme": s.theme,
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

    log::info!("set_setting: key={}", key);
    {
        let mut s = state.settings.lock().unwrap();
        match key.as_str() {
            "app_locale" => {
                s.app_locale = value.clone();
                let lang = crate::resolve_locale(&value);
                rust_i18n::set_locale(&lang);
            }
            "hallucination_filter_enabled" => s.hallucination_filter_enabled = value == "true",
            "hotkey" => s.hotkey_option = value.clone(),
            "cancel_shortcut" => s.cancel_shortcut = value.clone(),
            "recording_mode" => s.recording_mode = value.clone(),
            "selected_input_device_uid" => {
                s.selected_input_device_uid = if value.is_empty() { None } else { Some(value.clone()) };
            }
            "selected_model_id" => s.selected_model_id = value.clone(),
            "selected_language" => s.selected_language = value.clone(),
            "text_cleanup_enabled" => s.text_cleanup_enabled = value == "true",
            "cleanup_model_id" => s.cleanup_model_id = value.clone(),
            "llm_provider_id" => s.llm_provider_id = value.clone(),
            "llm_model" => s.llm_model = value.clone(),
            "asr_cloud_model" => s.asr_cloud_model = value.clone(),
            "gpu_mode" => s.gpu_mode = value.clone(),
            "llm_max_tokens" => s.llm_max_tokens = value.parse::<u32>().unwrap_or(256),
            "audio_ducking_enabled" => s.audio_ducking_enabled = value == "true",
            "audio_ducking_level" => s.audio_ducking_level = value.parse().unwrap_or(0.8),
            "vad_enabled" => s.vad_enabled = value == "true",
            "theme" => s.theme = value.clone(),
            _ => {
                log::warn!("Unknown setting key: {}", key);
                return;
            }
        }
    }
    // Invalidate cached ASR contexts when model or GPU mode changes
    if key == "selected_model_id" || key == "gpu_mode" {
        state.inference.asr.invalidate_all();
    }
    // Invalidate all cleanup contexts when cleanup model changes
    if key == "cleanup_model_id" {
        state.inference.cleanup.invalidate_all();
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
        crate::ui::tray::update_tray_labels(&app);
    }
    let _ = app.emit(crate::events::SETTINGS_CHANGED, &key);
}

// -- Launch at Login --

#[tauri::command]
pub fn get_launch_at_login_status() -> String {
    platform::get_launch_at_login_status().to_string()
}

#[tauri::command]
pub fn set_launch_at_login(enabled: bool) -> Result<String, String> {
    platform::set_launch_at_login(enabled).map(|s| s.to_string())
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


// -- History --

#[derive(Serialize)]
pub struct HistoryPage {
    entries: Vec<HistoryEntry>,
    total: u32,
}

#[tauri::command]
pub fn get_history(query: String, limit: u32, cursor: Option<u64>, state: tauri::State<'_, Arc<AppState>>) -> HistoryPage {
    let entries = state.get_history(&query, limit, cursor);
    let total = state.history_count(&query);
    HistoryPage { entries, total }
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

// -- Providers --

#[tauri::command]
pub fn add_provider(provider: Provider, state: tauri::State<'_, Arc<AppState>>, app: AppHandle) {
    // Store API key in OS keychain, not in preferences file
    crate::state::keyring_store(&provider.id, &provider.api_key);
    state.settings.lock().unwrap().providers.push(provider);
    state.save_preferences();
    let _ = app.emit(crate::events::SETTINGS_CHANGED, "providers");
}

#[tauri::command]
pub fn remove_provider(id: String, state: tauri::State<'_, Arc<AppState>>, app: AppHandle) {
    crate::state::keyring_delete(&id);
    state.settings.lock().unwrap().providers.retain(|p| p.id != id);
    state.save_preferences();
    let _ = app.emit(crate::events::SETTINGS_CHANGED, "providers");
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
    let _ = app.emit(crate::events::SETTINGS_CHANGED, "providers");
}

#[tauri::command]
pub fn get_providers(state: tauri::State<'_, Arc<AppState>>) -> Vec<Provider> {
    let providers = state.settings.lock().unwrap().providers.clone();
    providers.into_iter().map(|mut p| {
        p.api_key = p.masked_api_key();
        p
    }).collect()
}

#[tauri::command]
pub async fn fetch_provider_models(provider: Provider, state: tauri::State<'_, Arc<AppState>>) -> Result<Vec<String>, String> {
    provider.validate_url().map_err(|e| e.to_string())?;

    // If api_key is empty (editing mode), use the stored key
    let api_key = if provider.api_key.is_empty() || provider.api_key.starts_with('\u{2022}') {
        state.settings.lock().unwrap().providers.iter()
            .find(|p| p.id == provider.id)
            .map(|p| p.api_key.clone())
            .unwrap_or_default()
    } else {
        provider.api_key.clone()
    };

    let url = format!("{}/models", provider.base_url());

    let mut req = HTTP_CLIENT.get(&url);
    if !api_key.is_empty() {
        if provider.kind.is_anthropic_format() {
            req = req
                .header("x-api-key", &api_key)
                .header("anthropic-version", "2023-06-01");
        } else {
            req = req.header("Authorization", format!("Bearer {}", api_key));
        }
    }

    let response = req.send().await.map_err(|e| e.to_string())?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("HTTP {}: {}", status, body));
    }

    let json: serde_json::Value = response.json().await.map_err(|e| e.to_string())?;

    // Parse model IDs: handle {data:[...]} (OpenAI-compatible) and bare [...] (some providers)
    let models_array = json.get("data").and_then(|d| d.as_array())
        .or_else(|| json.as_array());

    let ids: Vec<String> = models_array
        .map(|arr| {
            arr.iter()
                .filter_map(|m| m.get("id").and_then(|id| id.as_str()).map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();

    if ids.is_empty() {
        return Err("No models found in response".to_string());
    }

    Ok(ids)
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

    // Open model manager if no model is ready (skip if using cloud ASR)
    let selected_id = state.settings.lock().unwrap().selected_model_id.clone();
    let model_ready = selected_id.starts_with("cloud:")
        || catalog()
            .model_by_id(&selected_id)
            .is_some_and(|m| m.is_downloaded());

    if !model_ready {
        crate::ui::tray::open_window_with_min(
            &app,
            "panel",
            "JonaWhisper",
            "/panel",
            750.0,
            550.0,
            Some((680.0, 450.0)),
        );
    }
}

#[tauri::command]
pub fn get_app_state(state: tauri::State<'_, Arc<AppState>>) -> serde_json::Value {
    state.to_frontend_json()
}

/// Enable the hotkey event tap without closing the setup window.
/// Called when permissions are granted so that shortcut capture works in setup step 2.
#[tauri::command]
pub fn enable_monitoring(enabled: tauri::State<'_, Arc<AtomicBool>>) {
    if !enabled.load(Ordering::SeqCst) {
        enabled.store(true, Ordering::SeqCst);
        log::info!("Monitoring enabled (pre-start)");
    }
}

// -- Shortcut capture --

#[tauri::command]
pub fn start_shortcut_capture(capture: tauri::State<'_, Arc<crate::platform::hotkey::CaptureControl>>) {
    capture.enter();
}

#[tauri::command]
pub fn stop_shortcut_capture(capture: tauri::State<'_, Arc<crate::platform::hotkey::CaptureControl>>) {
    capture.exit();
}

// -- Debug --

#[tauri::command]
pub async fn simulate_pill_test(app: AppHandle, _count: Option<u32>) {
    use crate::ui::pill::{self, PillMode};
    use std::time::Duration;

    fn fake_spectrum(frame: u32) -> Vec<f32> {
        (0..12)
            .map(|i| {
                let phase = (frame as f32 * 0.15) + (i as f32 * 0.5);
                (phase.sin() * 0.5 + 0.5) * 0.8
            })
            .collect()
    }

    async fn recording_phase(app: &AppHandle, secs: f32) {
        pill::set_mode(PillMode::Recording);
        let _ = app.emit(crate::events::RECORDING_STARTED, ());
        let frames = (secs * 30.0) as u32;
        for frame in 0..frames {
            pill::set_spectrum(&fake_spectrum(frame));
            tokio::time::sleep(Duration::from_millis(33)).await;
        }
    }

    log::info!("=== Pill test: full flow ===");

    // ── 1. Simple recording → transcribing → success ──
    log::info!("[1/5] Single recording → success");
    crate::ui::tray::open_pill_window(&app);
    tokio::time::sleep(Duration::from_millis(100)).await;

    recording_phase(&app, 2.0).await;

    pill::set_pending(1);
    pill::set_mode(PillMode::Transcribing);
    let _ = app.emit(crate::events::RECORDING_STOPPED, serde_json::json!({ "queue_count": 1 }));
    tokio::time::sleep(Duration::from_millis(2000)).await;

    let _ = app.emit(crate::events::TRANSCRIPTION_COMPLETE, serde_json::json!({ "text": "Test single recording" }));
    pill::set_mode(PillMode::Success);
    tokio::time::sleep(Duration::from_millis(800)).await;
    crate::ui::tray::close_pill_window(&app);
    tokio::time::sleep(Duration::from_millis(500)).await;

    // ── 2. Recording → transcribing → error ──
    log::info!("[2/5] Single recording → error");
    crate::ui::tray::open_pill_window(&app);
    tokio::time::sleep(Duration::from_millis(100)).await;

    recording_phase(&app, 1.5).await;

    pill::set_pending(1);
    pill::set_mode(PillMode::Transcribing);
    let _ = app.emit(crate::events::RECORDING_STOPPED, serde_json::json!({ "queue_count": 1 }));
    tokio::time::sleep(Duration::from_millis(1500)).await;

    pill::set_mode(PillMode::Error);
    tokio::time::sleep(Duration::from_millis(1000)).await;
    crate::ui::tray::close_pill_window(&app);
    tokio::time::sleep(Duration::from_millis(500)).await;

    // ── 3. Queue: record while transcribing (2 items queued) ──
    log::info!("[3/5] Queue: record during transcription (pending=2 then 3)");
    crate::ui::tray::open_pill_window(&app);
    tokio::time::sleep(Duration::from_millis(100)).await;

    // First recording
    recording_phase(&app, 1.5).await;
    pill::set_pending(1);
    pill::set_mode(PillMode::Transcribing);
    let _ = app.emit(crate::events::RECORDING_STOPPED, serde_json::json!({ "queue_count": 1 }));
    tokio::time::sleep(Duration::from_millis(800)).await;

    // Second recording while first is transcribing
    pill::set_mode(PillMode::Recording);
    recording_phase(&app, 1.0).await;
    pill::set_pending(2);
    pill::set_mode(PillMode::Transcribing);
    let _ = app.emit(crate::events::RECORDING_STOPPED, serde_json::json!({ "queue_count": 2 }));
    tokio::time::sleep(Duration::from_millis(800)).await;

    // Third recording while queue has 2
    pill::set_mode(PillMode::Recording);
    recording_phase(&app, 1.0).await;
    pill::set_pending(3);
    pill::set_mode(PillMode::Transcribing);
    let _ = app.emit(crate::events::RECORDING_STOPPED, serde_json::json!({ "queue_count": 3 }));

    // Process queue: 3 → 2 → 1 → done
    for remaining in (0..3).rev() {
        tokio::time::sleep(Duration::from_millis(1200)).await;
        pill::set_pending(remaining + 1);
        let _ = app.emit(crate::events::TRANSCRIPTION_COMPLETE, serde_json::json!({ "text": format!("Queue item {}", 3 - remaining) }));
        if remaining > 0 {
            pill::set_pending(remaining);
            let _ = app.emit(crate::events::TRANSCRIPTION_STARTED, serde_json::json!({ "queue_count": remaining }));
        }
    }

    pill::set_mode(PillMode::Success);
    tokio::time::sleep(Duration::from_millis(800)).await;
    crate::ui::tray::close_pill_window(&app);
    tokio::time::sleep(Duration::from_millis(500)).await;

    // ── 4. Preparing mode (model loading) ──
    log::info!("[4/5] Preparing mode");
    crate::ui::tray::open_pill_window(&app);
    tokio::time::sleep(Duration::from_millis(100)).await;
    pill::set_mode(PillMode::Preparing);
    tokio::time::sleep(Duration::from_millis(2000)).await;

    // Transition to recording after model loaded
    recording_phase(&app, 1.5).await;
    pill::set_pending(1);
    pill::set_mode(PillMode::Transcribing);
    let _ = app.emit(crate::events::RECORDING_STOPPED, serde_json::json!({ "queue_count": 1 }));
    tokio::time::sleep(Duration::from_millis(1500)).await;

    let _ = app.emit(crate::events::TRANSCRIPTION_COMPLETE, serde_json::json!({ "text": "After preparing" }));
    pill::set_mode(PillMode::Success);
    tokio::time::sleep(Duration::from_millis(800)).await;
    crate::ui::tray::close_pill_window(&app);
    tokio::time::sleep(Duration::from_millis(500)).await;

    // ── 5. Rapid fire: quick record → immediate re-record ──
    log::info!("[5/5] Rapid fire: 3 quick recordings back-to-back");
    crate::ui::tray::open_pill_window(&app);
    tokio::time::sleep(Duration::from_millis(100)).await;

    for i in 1..=3u32 {
        recording_phase(&app, 0.5).await;
        pill::set_pending(i);
        pill::set_mode(PillMode::Transcribing);
        let _ = app.emit(crate::events::RECORDING_STOPPED, serde_json::json!({ "queue_count": i }));
        tokio::time::sleep(Duration::from_millis(200)).await;
    }

    // Drain queue
    for remaining in (0..3).rev() {
        tokio::time::sleep(Duration::from_millis(1000)).await;
        let _ = app.emit(crate::events::TRANSCRIPTION_COMPLETE, serde_json::json!({ "text": format!("Rapid {}", 3 - remaining) }));
        if remaining > 0 {
            pill::set_pending(remaining as u32);
            let _ = app.emit(crate::events::TRANSCRIPTION_STARTED, serde_json::json!({ "queue_count": remaining }));
        }
    }

    pill::set_mode(PillMode::Success);
    tokio::time::sleep(Duration::from_millis(800)).await;
    crate::ui::tray::close_pill_window(&app);

    log::info!("=== Pill test complete ===");
}
