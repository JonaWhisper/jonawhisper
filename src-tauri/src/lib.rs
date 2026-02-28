mod audio;
mod commands;
mod engines;
mod errors;
mod events;
mod llm_cleanup;
mod menu_icons;
mod platform;
mod post_processor;
mod process_runner;
mod recording;
mod state;
mod transcriber;
mod tray;

rust_i18n::i18n!("../src/i18n");

use state::AppState;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::Manager;

/// Resolve the effective locale ("fr" or "en") from preferences.
pub fn resolve_locale(app_locale: &str) -> String {
    if app_locale != "auto" {
        return app_locale.to_string();
    }
    let sys = sys_locale::get_locale().unwrap_or_else(|| "en".to_string());
    if sys.starts_with("fr") { "fr".to_string() } else { "en".to_string() }
}

/// Wrapper to store the hotkey update channel sender in Tauri managed state.
pub struct HotkeyUpdateSender(pub crossbeam_channel::Sender<platform::hotkey::HotkeyUpdate>);

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    env_logger::init();
    recording::cleanup_orphan_audio_files();

    let app_state = Arc::new(AppState::default());

    // Set Rust i18n locale from saved preferences
    {
        let locale = app_state.settings.lock().unwrap().app_locale.clone();
        rust_i18n::set_locale(&resolve_locale(&locale));
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_clipboard_manager::init())


        .manage(app_state.clone())
        .invoke_handler(tauri::generate_handler![
            commands::get_audio_devices,
            commands::get_engines,
            commands::get_models,
            commands::get_downloaded_models,
            commands::download_model_cmd,
            commands::delete_model_cmd,
            commands::get_languages,
            commands::get_permissions,
            commands::request_permission,
            commands::start_monitoring,
            commands::get_history,
            commands::search_history,
            commands::delete_history_entry,
            commands::delete_history_day,
            commands::clear_history,
            commands::add_api_server,
            commands::remove_api_server,
            commands::get_api_servers,
            commands::get_settings,
            commands::set_setting,
            commands::set_llm_config,
            commands::get_app_state,
            commands::start_mic_test,
            commands::stop_mic_test,
            commands::start_shortcut_capture,
            commands::stop_shortcut_capture,
            commands::simulate_pill_test,
            commands::get_system_locale,
        ])
        .setup(move |app| {
            #[cfg(target_os = "macos")]
            {
                app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            }

            tray::setup_tray(app.handle())?;

            // Audio thread (cpal::Stream is not Send)
            let (cmd_tx, spectrum_data, reply_rx, stream_error) =
                recording::spawn_audio_thread();

            // Mic test sender (clone before cmd_tx is moved)
            app.manage(recording::MicTestSender(cmd_tx.clone()));

            // Recording state (Send-safe: only channels, no cpal::Stream)
            let rec_state = Arc::new(std::sync::Mutex::new(recording::new_recording_state(
                cmd_tx.clone(),
                reply_rx,
            )));

            // Deferred monitoring flag — hotkey thread waits for this
            let monitor_enabled = Arc::new(AtomicBool::new(false));
            app.manage(monitor_enabled.clone());

            // Start CGEvent tap hotkey monitor (with cancel key support)
            let (hotkey_str, cancel_str) = {
                let s = app_state.settings.lock().unwrap();
                (s.hotkey_option.clone(), s.cancel_shortcut.clone())
            };
            let initial_record = platform::hotkey::Shortcut::parse(&hotkey_str);
            let initial_cancel = platform::hotkey::Shortcut::parse(&cancel_str);
            let (hotkey_rx, hotkey_update_tx) =
                platform::hotkey::start_monitor(initial_record, initial_cancel, monitor_enabled.clone());
            app.manage(HotkeyUpdateSender(hotkey_update_tx));

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
                tray::open_fixed_window(app.handle(), "setup", &rust_i18n::t!("window.setup"), "/setup", 420.0, 420.0);
            }

            // Spectrum emission (30fps) + stream error detection
            recording::spawn_spectrum_emitter(
                app.handle().clone(),
                app_state,
                cmd_tx,
                spectrum_data,
                stream_error,
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
