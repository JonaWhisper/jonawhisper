mod audio;
mod engines;
mod hotkey;
mod model_downloader;
mod paste;
mod platform;
mod post_processor;
mod process_runner;
mod state;
mod transcriber;

use engines::{EngineCatalog, EngineInfo, Language};
use state::{ApiServerConfig, AppState, HistoryEntry};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
    AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder,
};

// -- Tauri Commands --

#[tauri::command]
fn get_audio_devices() -> Vec<audio::AudioDevice> {
    audio::AudioRecorder::list_devices()
}

#[tauri::command]
fn get_engines(state: tauri::State<'_, Arc<AppState>>) -> Vec<EngineInfo> {
    let api_servers = state.api_servers.lock().unwrap().clone();
    let catalog = EngineCatalog::new(&api_servers);
    catalog.engine_infos()
}

#[tauri::command]
fn get_models(state: tauri::State<'_, Arc<AppState>>) -> Vec<engines::ASRModel> {
    let api_servers = state.api_servers.lock().unwrap().clone();
    let catalog = EngineCatalog::new(&api_servers);
    catalog.all_models()
}

#[tauri::command]
fn get_downloaded_models(state: tauri::State<'_, Arc<AppState>>) -> Vec<engines::ASRModel> {
    let api_servers = state.api_servers.lock().unwrap().clone();
    let catalog = EngineCatalog::new(&api_servers);
    catalog.downloaded_models()
}

#[tauri::command]
fn select_model(id: String, state: tauri::State<'_, Arc<AppState>>) {
    *state.selected_model_id.lock().unwrap() = id;
}

#[tauri::command]
fn get_selected_model_id(state: tauri::State<'_, Arc<AppState>>) -> String {
    state.selected_model_id.lock().unwrap().clone()
}

#[tauri::command]
async fn download_model_cmd(
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
    Ok(model_downloader::download_model(app, state_clone, model).await)
}

#[tauri::command]
fn delete_model_cmd(id: String, state: tauri::State<'_, Arc<AppState>>) -> bool {
    let api_servers = state.api_servers.lock().unwrap().clone();
    let catalog = EngineCatalog::new(&api_servers);
    if let Some(model) = catalog.model_by_id(&id) {
        model_downloader::delete_model(&model)
    } else {
        false
    }
}

#[tauri::command]
fn get_languages(state: tauri::State<'_, Arc<AppState>>) -> Vec<Language> {
    let api_servers = state.api_servers.lock().unwrap().clone();
    let catalog = EngineCatalog::new(&api_servers);
    catalog.supported_languages()
}

#[tauri::command]
fn select_language(code: String, state: tauri::State<'_, Arc<AppState>>) {
    *state.selected_language.lock().unwrap() = code;
}

#[tauri::command]
fn get_selected_language(state: tauri::State<'_, Arc<AppState>>) -> String {
    state.selected_language.lock().unwrap().clone()
}

#[tauri::command]
fn get_permissions() -> platform::PermissionReport {
    platform::check_permissions()
}

#[tauri::command]
fn request_permission(kind: String) -> bool {
    platform::request_permission(&kind)
}

#[tauri::command]
fn get_post_processing_enabled(state: tauri::State<'_, Arc<AppState>>) -> bool {
    *state.post_processing_enabled.lock().unwrap()
}

#[tauri::command]
fn set_post_processing_enabled(enabled: bool, state: tauri::State<'_, Arc<AppState>>) {
    *state.post_processing_enabled.lock().unwrap() = enabled;
}

#[tauri::command]
fn get_hotkey(state: tauri::State<'_, Arc<AppState>>) -> String {
    state.hotkey_option.lock().unwrap().clone()
}

#[tauri::command]
fn set_hotkey(hotkey: String, state: tauri::State<'_, Arc<AppState>>) {
    *state.hotkey_option.lock().unwrap() = hotkey;
}

#[tauri::command]
fn get_history(state: tauri::State<'_, Arc<AppState>>) -> Vec<HistoryEntry> {
    state.transcription_history.lock().unwrap().clone()
}

#[tauri::command]
fn clear_history(state: tauri::State<'_, Arc<AppState>>) {
    state.transcription_history.lock().unwrap().clear();
}

#[tauri::command]
fn add_api_server(config: ApiServerConfig, state: tauri::State<'_, Arc<AppState>>) {
    state.api_servers.lock().unwrap().push(config);
}

#[tauri::command]
fn remove_api_server(id: String, state: tauri::State<'_, Arc<AppState>>) {
    state.api_servers.lock().unwrap().retain(|s| s.id != id);
}

#[tauri::command]
fn get_api_servers(state: tauri::State<'_, Arc<AppState>>) -> Vec<ApiServerConfig> {
    state.api_servers.lock().unwrap().clone()
}

#[tauri::command]
fn start_monitoring(enabled: tauri::State<'_, Arc<AtomicBool>>) {
    if !enabled.load(Ordering::SeqCst) {
        enabled.store(true, Ordering::SeqCst);
        log::info!("Monitoring enabled by start_monitoring command");
    }
}

#[tauri::command]
fn get_app_state(state: tauri::State<'_, Arc<AppState>>) -> serde_json::Value {
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

// -- Audio commands for the dedicated audio thread --

enum AudioCmd {
    StartRecording { device_uid: Option<String> },
    StopRecording,
    GetSpectrum,
}

enum AudioReply {
    Started,
    Stopped { path: Option<std::path::PathBuf> },
    Spectrum(#[allow(dead_code)] Vec<f32>),
}

// -- Recording state (Send-safe, does not hold AudioRecorder) --

struct RecordingState {
    key_down_time: Option<Instant>,
    last_short_tap_time: Option<Instant>,
    audio_tx: std::sync::mpsc::Sender<AudioCmd>,
    audio_rx: std::sync::mpsc::Receiver<AudioReply>,
}

// SAFETY: The channel endpoints are Send+Sync safe
unsafe impl Send for RecordingState {}
unsafe impl Sync for RecordingState {}

fn start_recording(app: &AppHandle, state: &Arc<AppState>, rec: &mut RecordingState) {
    if *state.is_recording.lock().unwrap() {
        return;
    }
    *state.is_recording.lock().unwrap() = true;
    *state.transcription_cancelled.lock().unwrap() = false;
    rec.key_down_time = Some(Instant::now());

    let device_uid = state.selected_input_device_uid.lock().unwrap().clone();
    let _ = rec.audio_tx.send(AudioCmd::StartRecording { device_uid });
    // Wait for ack
    let _ = rec.audio_rx.recv();

    platform::play_sound("Tink");
    let _ = app.emit("recording-started", ());
}

fn stop_recording_and_enqueue(app: &AppHandle, state: &Arc<AppState>, rec: &mut RecordingState) {
    if !*state.is_recording.lock().unwrap() {
        return;
    }
    *state.is_recording.lock().unwrap() = false;

    let _ = rec.audio_tx.send(AudioCmd::StopRecording);
    let audio_path = match rec.audio_rx.recv() {
        Ok(AudioReply::Stopped { path }) => path,
        _ => None,
    };

    // Detect short tap (< 300ms)
    let is_short_tap = rec
        .key_down_time
        .map(|t| t.elapsed() < Duration::from_millis(300))
        .unwrap_or(false);
    rec.key_down_time = None;

    if is_short_tap {
        if let Some(ref path) = audio_path {
            let _ = std::fs::remove_file(path);
        }

        if let Some(last) = rec.last_short_tap_time {
            if last.elapsed() < Duration::from_millis(500) {
                rec.last_short_tap_time = None;
                cancel_transcription(app, state);
                return;
            }
        }
        rec.last_short_tap_time = Some(Instant::now());

        let _ = app.emit("recording-stopped", ());
        return;
    }

    rec.last_short_tap_time = None;

    let audio_path = match audio_path {
        Some(p) => p,
        None => {
            let _ = app.emit("recording-stopped", ());
            return;
        }
    };

    platform::play_sound("Pop");

    let count = state.enqueue(audio_path);
    let _ = app.emit("recording-stopped", serde_json::json!({ "queue_count": count }));

    let app_clone = app.clone();
    let state_clone = Arc::clone(state);
    tauri::async_runtime::spawn(async move {
        process_next_in_queue(&app_clone, &state_clone).await;
    });
}

async fn process_next_in_queue(app: &AppHandle, state: &Arc<AppState>) {
    if *state.is_transcribing.lock().unwrap() {
        return;
    }
    if state.transcription_queue.lock().unwrap().is_empty() {
        return;
    }

    *state.is_transcribing.lock().unwrap() = true;
    let audio_path = match state.dequeue() {
        Some(p) => p,
        None => {
            *state.is_transcribing.lock().unwrap() = false;
            return;
        }
    };

    let _ = app.emit(
        "transcription-started",
        serde_json::json!({ "queue_count": state.queue_count() }),
    );

    let state_clone = Arc::clone(state);
    let audio_path_clone = audio_path.clone();
    let result = tokio::task::spawn_blocking(move || {
        transcriber::transcribe(&state_clone, &audio_path_clone)
    })
    .await;

    let _ = std::fs::remove_file(&audio_path);

    let had_error;
    match result {
        Ok(Ok(text)) => {
            had_error = false;
            if *state.transcription_cancelled.lock().unwrap() {
                log::info!("Transcription result discarded (cancelled)");
            } else {
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    let processed = if *state.post_processing_enabled.lock().unwrap() {
                        let lang = state.selected_language.lock().unwrap().clone();
                        post_processor::process(trimmed, &lang)
                    } else {
                        trimmed.to_string()
                    };

                    paste::paste_text(&processed);
                    state.add_history(processed.clone());
                    platform::play_sound("Glass");

                    let _ = app.emit(
                        "transcription-complete",
                        serde_json::json!({ "text": processed }),
                    );
                } else {
                    platform::play_sound("Basso");
                    let _ = app.emit("transcription-complete", serde_json::json!({ "text": "" }));
                }
            }
        }
        Ok(Err(e)) => {
            had_error = true;
            log::error!("Transcription error: {}", e);
            platform::play_sound("Basso");
            let _ = app.emit(
                "transcription-error",
                serde_json::json!({ "error": e.to_string() }),
            );
        }
        Err(e) => {
            had_error = true;
            log::error!("Transcription task panicked: {}", e);
            platform::play_sound("Basso");
            let _ = app.emit(
                "transcription-error",
                serde_json::json!({ "error": "Internal error" }),
            );
        }
    }

    *state.is_transcribing.lock().unwrap() = false;

    if !had_error {
        let app_clone = app.clone();
        let state_clone = Arc::clone(state);
        Box::pin(process_next_in_queue(&app_clone, &state_clone)).await;
    }
}

fn cancel_transcription(app: &AppHandle, state: &Arc<AppState>) {
    while let Some(path) = state.dequeue() {
        let _ = std::fs::remove_file(&path);
    }
    *state.transcription_cancelled.lock().unwrap() = true;
    platform::play_sound("Funk");
    let _ = app.emit("transcription-cancelled", ());
}

fn cleanup_orphan_audio_files() {
    let tmp_dir = std::env::temp_dir();
    if let Ok(entries) = std::fs::read_dir(&tmp_dir) {
        let cutoff = std::time::SystemTime::now() - Duration::from_secs(300);
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with("whisper_dictate_") && name.ends_with(".wav") {
                if let Ok(metadata) = entry.metadata() {
                    if let Ok(modified) = metadata.modified() {
                        if modified < cutoff {
                            let _ = std::fs::remove_file(entry.path());
                            log::info!("Cleaned orphan audio: {}", name);
                        }
                    }
                }
            }
        }
    }
}

fn open_window(app: &AppHandle, label: &str, title: &str, url: &str, width: f64, height: f64) {
    if let Some(window) = app.get_webview_window(label) {
        let _ = window.set_focus();
        return;
    }

    let _ = WebviewWindowBuilder::new(app, label, WebviewUrl::App(url.into()))
        .title(title)
        .inner_size(width, height)
        .resizable(true)
        .build();
}

// -- Tray setup --

fn setup_tray(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let quit = MenuItem::with_id(app, "quit", "Quit WhisperDictate", true, Some("CmdOrCtrl+Q"))?;
    let model_manager = MenuItem::with_id(app, "model_manager", "Manage Models\u{2026}", true, None::<&str>)?;
    let setup = MenuItem::with_id(app, "setup", "Setup\u{2026}", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let separator2 = PredefinedMenuItem::separator(app)?;

    let menu = Menu::with_items(
        app,
        &[
            &MenuItem::with_id(app, "title", "WhisperDictate", false, None::<&str>)?,
            &separator,
            &model_manager,
            &setup,
            &separator2,
            &quit,
        ],
    )?;

    let _tray = TrayIconBuilder::new()
        .icon(app.default_window_icon().unwrap().clone())
        .icon_as_template(true)
        .tooltip("WhisperDictate")
        .menu(&menu)
        .on_menu_event(move |app, event| {
            match event.id().as_ref() {
                "quit" => {
                    app.exit(0);
                }
                "model_manager" => {
                    open_window(app, "model-manager", "Model Manager", "/model-manager", 700.0, 500.0);
                }
                "setup" => {
                    open_window(app, "setup", "Setup", "/setup", 420.0, 380.0);
                }
                _ => {}
            }
        })
        .build(app)?;

    Ok(())
}

// -- App setup --

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    env_logger::init();
    cleanup_orphan_audio_files();

    let app_state = Arc::new(AppState::default());

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_fs::init())
        .manage(app_state.clone())
        .invoke_handler(tauri::generate_handler![
            get_audio_devices,
            get_engines,
            get_models,
            get_downloaded_models,
            select_model,
            get_selected_model_id,
            download_model_cmd,
            delete_model_cmd,
            get_languages,
            select_language,
            get_selected_language,
            get_permissions,
            request_permission,
            start_monitoring,
            get_post_processing_enabled,
            set_post_processing_enabled,
            get_hotkey,
            set_hotkey,
            get_history,
            clear_history,
            add_api_server,
            remove_api_server,
            get_api_servers,
            get_app_state,
        ])
        .setup(move |app| {
            #[cfg(target_os = "macos")]
            {
                app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            }

            setup_tray(app.handle())?;

            // Setup audio on a dedicated thread (cpal::Stream is not Send)
            let (cmd_tx, cmd_rx) = std::sync::mpsc::channel::<AudioCmd>();
            let (reply_tx, reply_rx) = std::sync::mpsc::channel::<AudioReply>();
            let spectrum_data = Arc::new(std::sync::Mutex::new(vec![0.0f32; 12]));
            let spectrum_clone = spectrum_data.clone();

            std::thread::spawn(move || {
                let mut recorder = audio::AudioRecorder::new();
                loop {
                    match cmd_rx.recv() {
                        Ok(AudioCmd::StartRecording { device_uid }) => {
                            recorder.start_recording(device_uid.as_deref());
                            let _ = reply_tx.send(AudioReply::Started);
                        }
                        Ok(AudioCmd::StopRecording) => {
                            let path = recorder.stop_recording();
                            let _ = reply_tx.send(AudioReply::Stopped { path });
                        }
                        Ok(AudioCmd::GetSpectrum) => {
                            let s = recorder.get_spectrum();
                            *spectrum_clone.lock().unwrap() = s.clone();
                            let _ = reply_tx.send(AudioReply::Spectrum(s));
                        }
                        Err(_) => break,
                    }
                }
            });

            // Hotkey handler state (Send-safe: only channels, no cpal::Stream)
            let rec_state = Arc::new(std::sync::Mutex::new(RecordingState {
                key_down_time: None,
                last_short_tap_time: None,
                audio_tx: cmd_tx.clone(),
                audio_rx: reply_rx,
            }));

            let app_handle = app.handle().clone();
            let state_for_shortcut = app_state.clone();

            // Monitoring is deferred until all permissions are granted.
            // The hotkey thread waits for this flag before creating the CGEvent tap.
            let monitor_enabled = Arc::new(AtomicBool::new(false));
            app.manage(monitor_enabled.clone());

            // Start CGEvent tap hotkey monitor (waits for monitor_enabled)
            let hotkey_name = state_for_shortcut.hotkey_option.lock().unwrap().clone();
            let initial_hotkey = hotkey::HotkeyOption::from_name(&hotkey_name);
            let (hotkey_rx, _hotkey_update_tx) =
                hotkey::start_monitor(initial_hotkey, monitor_enabled.clone());

            // Hotkey event processing thread
            let rec_for_hotkey = rec_state.clone();
            let state_for_hotkey = state_for_shortcut.clone();
            let app_for_hotkey = app_handle.clone();
            std::thread::spawn(move || {
                loop {
                    match hotkey_rx.recv() {
                        Ok(hotkey::HotkeyEvent::KeyDown) => {
                            let mut rec = rec_for_hotkey.lock().unwrap();
                            start_recording(&app_for_hotkey, &state_for_hotkey, &mut rec);
                        }
                        Ok(hotkey::HotkeyEvent::KeyUp) => {
                            let mut rec = rec_for_hotkey.lock().unwrap();
                            stop_recording_and_enqueue(&app_for_hotkey, &state_for_hotkey, &mut rec);
                        }
                        Err(_) => break,
                    }
                }
            });

            // Check permissions and show setup if needed
            let report = platform::check_permissions();
            let all_granted = report.microphone == platform::PermissionStatus::Granted
                && report.accessibility == platform::PermissionStatus::Granted
                && report.input_monitoring == platform::PermissionStatus::Granted;

            if all_granted {
                // All permissions already granted — enable monitoring immediately
                monitor_enabled.store(true, Ordering::SeqCst);
            } else {
                open_window(app.handle(), "setup", "Setup", "/setup", 420.0, 380.0);
            }

            // Spectrum emission timer (30fps)
            let app_spectrum = app.handle().clone();
            let state_spectrum = state_for_shortcut.clone();
            let cmd_tx_spectrum = cmd_tx;
            let spectrum_for_emit = spectrum_data;
            std::thread::spawn(move || {
                loop {
                    std::thread::sleep(Duration::from_millis(33));
                    if *state_spectrum.is_recording.lock().unwrap() {
                        // Request spectrum from audio thread (non-blocking: just read last cached value)
                        let spectrum = spectrum_for_emit.lock().unwrap().clone();
                        // Also ask audio thread to update the cache
                        let _ = cmd_tx_spectrum.send(AudioCmd::GetSpectrum);
                        let _ = app_spectrum.emit("spectrum-data", spectrum);
                    }
                }
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
