mod audio;
mod commands;
mod engines;
mod platform;
mod post_processor;
mod process_runner;
mod recording;
mod state;
mod transcriber;
mod tray;

use state::AppState;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    env_logger::init();
    recording::cleanup_orphan_audio_files();

    let app_state = Arc::new(AppState::default());

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_fs::init())
        .manage(app_state.clone())
        .invoke_handler(tauri::generate_handler![
            commands::get_audio_devices,
            commands::get_engines,
            commands::get_models,
            commands::get_downloaded_models,
            commands::select_model,
            commands::get_selected_model_id,
            commands::download_model_cmd,
            commands::delete_model_cmd,
            commands::get_languages,
            commands::select_language,
            commands::get_selected_language,
            commands::get_permissions,
            commands::request_permission,
            commands::start_monitoring,
            commands::get_post_processing_enabled,
            commands::set_post_processing_enabled,
            commands::get_hotkey,
            commands::set_hotkey,
            commands::get_history,
            commands::clear_history,
            commands::add_api_server,
            commands::remove_api_server,
            commands::get_api_servers,
            commands::get_app_state,
        ])
        .setup(move |app| {
            #[cfg(target_os = "macos")]
            {
                app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            }

            tray::setup_tray(app.handle())?;

            // Audio thread (cpal::Stream is not Send)
            let (cmd_tx, spectrum_data, reply_rx) = recording::spawn_audio_thread();

            // Recording state (Send-safe: only channels, no cpal::Stream)
            let rec_state = Arc::new(std::sync::Mutex::new(recording::new_recording_state(
                cmd_tx.clone(),
                reply_rx,
            )));

            // Deferred monitoring flag — hotkey thread waits for this
            let monitor_enabled = Arc::new(AtomicBool::new(false));
            app.manage(monitor_enabled.clone());

            // Start CGEvent tap hotkey monitor
            let hotkey_name = app_state.hotkey_option.lock().unwrap().clone();
            let initial_hotkey = platform::hotkey::HotkeyOption::from_name(&hotkey_name);
            let (hotkey_rx, _hotkey_update_tx) =
                platform::hotkey::start_monitor(initial_hotkey, monitor_enabled.clone());

            // Hotkey event processing thread
            recording::spawn_hotkey_handler(
                hotkey_rx,
                app.handle().clone(),
                app_state.clone(),
                rec_state,
            );

            // Check permissions and show setup if needed
            let report = platform::check_permissions();
            if report.all_granted() {
                monitor_enabled.store(true, Ordering::SeqCst);
            } else {
                tray::open_window(app.handle(), "setup", "Setup", "/setup", 420.0, 380.0);
            }

            // Spectrum emission (30fps)
            recording::spawn_spectrum_emitter(
                app.handle().clone(),
                app_state.clone(),
                cmd_tx,
                spectrum_data,
            );

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|_app, event| {
            // Keep the app running when the last window closes (menu bar app)
            if let tauri::RunEvent::ExitRequested { api, .. } = event {
                api.prevent_exit();
            }
        });
}
