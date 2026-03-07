mod audio;
mod cleanup;
mod commands;
mod errors;
mod events;
mod migrations;
mod platform;
mod recording;
mod state;
mod ui;

// Force the linker to include engine crates so their `inventory::submit!`
// registrations are present at runtime. Without these, the crates are
// optimized away as dead code and EngineCatalog sees 0 engines.
extern crate jona_engine_whisper;
extern crate jona_engine_qwen;
extern crate jona_engine_canary;
extern crate jona_engine_parakeet;
extern crate jona_engine_voxtral;
extern crate jona_engine_llama;
extern crate jona_engine_bert;
extern crate jona_engine_pcs;
extern crate jona_engine_correction;

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

    // Initialize the engine catalog from inventory auto-registration.
    // Each engine crate registers itself via `inventory::submit!`.
    jona_engines::EngineCatalog::init_auto();

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
            commands::audio::get_audio_devices,
            commands::audio::start_mic_test,
            commands::audio::stop_mic_test,
            commands::engines::get_engines,
            commands::engines::get_models,
            commands::engines::get_downloaded_models,
            commands::engines::download_model_cmd,
            commands::engines::delete_model_cmd,
            commands::engines::pause_download,
            commands::engines::cancel_download,
            commands::engines::get_languages,
            commands::history::get_history,
            commands::history::delete_history_entry,
            commands::history::delete_history_day,
            commands::history::clear_history,
            commands::providers::add_provider,
            commands::providers::remove_provider,
            commands::providers::update_provider,
            commands::providers::get_providers,
            commands::providers::fetch_provider_models,
            commands::settings::get_settings,
            commands::settings::set_setting,
            commands::settings::get_system_locale,
            commands::settings::get_launch_at_login_status,
            commands::settings::set_launch_at_login,
            commands::permissions::get_permissions,
            commands::permissions::request_permission,
            commands::permissions::start_monitoring,
            commands::permissions::enable_monitoring,
            commands::app::get_app_state,
            commands::app::start_shortcut_capture,
            commands::app::stop_shortcut_capture,
            commands::app::simulate_pill_test,
        ])
        .setup(move |app| {
            #[cfg(target_os = "macos")]
            {
                app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            }

            // Panel window is pre-created hidden (tauri.conf.json) — intercept close → hide
            if let Some(win) = app.get_webview_window("panel") {
                let win2 = win.clone();
                win.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        let _ = win2.hide();
                    }
                });
            } else {
                log::warn!("Panel window not found at startup — check tauri.conf.json");
            }

            ui::tray::setup_tray(app.handle())?;

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
            let capture_control = Arc::new(platform::hotkey::CaptureControl::new());
            let (hotkey_rx, hotkey_update_tx) =
                platform::hotkey::start_monitor(initial_record, initial_cancel, monitor_enabled.clone(), capture_control.clone());
            app.manage(capture_control);
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
                ui::tray::open_fixed_window(app.handle(), "setup", &rust_i18n::t!("window.setup"), "/setup", 420.0, 450.0);
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
            // Keep the app running when the last window closes (menu bar app).
            // code=None → window close, code=Some → explicit app.exit() (e.g. quit menu item)
            if let tauri::RunEvent::ExitRequested { api, code, .. } = event {
                if code.is_none() {
                    api.prevent_exit();
                }
            }
        });
}
