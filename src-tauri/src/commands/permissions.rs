use super::catalog;
use crate::platform;
use crate::state::AppState;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};

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

#[tauri::command]
pub fn start_monitoring(
    app: AppHandle,
    enabled: tauri::State<'_, Arc<AtomicBool>>,
    state: tauri::State<'_, Arc<AppState>>,
) {
    if !enabled.load(Ordering::Relaxed) {
        enabled.store(true, Ordering::Relaxed);
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
        crate::ui::tray::show_panel(&app);
    }
}

/// Enable the hotkey event tap without closing the setup window.
/// Called when permissions are granted so that shortcut capture works in setup step 2.
#[tauri::command]
pub fn enable_monitoring(enabled: tauri::State<'_, Arc<AtomicBool>>) {
    if !enabled.load(Ordering::Relaxed) {
        enabled.store(true, Ordering::Relaxed);
        log::info!("Monitoring enabled (pre-start)");
    }
}
