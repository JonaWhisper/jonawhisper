use crate::engines::{common_languages, EngineCatalog};
use crate::platform::audio_devices;
use crate::state::AppState;
use std::sync::Arc;
use tauri::{
    menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu},
    tray::TrayIconBuilder,
    AppHandle, Listener, Manager, WebviewUrl, WebviewWindowBuilder,
};

const PILL_WIDTH: f64 = 140.0;
const PILL_HEIGHT: f64 = 56.0;
const PILL_TOP_OFFSET: f64 = 40.0;

fn get_state(app: &AppHandle) -> Arc<AppState> {
    app.state::<Arc<AppState>>().inner().clone()
}

// -- Window management --

pub fn open_window(app: &AppHandle, label: &str, title: &str, url: &str, width: f64, height: f64) {
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

pub fn open_pill_window(app: &AppHandle) {
    if app.get_webview_window("pill").is_some() {
        return;
    }

    let handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        match WebviewWindowBuilder::new(&handle, "pill", WebviewUrl::App("/pill".into()))
            .decorations(false)
            .transparent(true)
            .always_on_top(true)
            .inner_size(PILL_WIDTH, PILL_HEIGHT)
            .resizable(false)
            .visible(false)
            .build()
        {
            Ok(win) => {
                #[cfg(target_os = "macos")]
                configure_pill_nswindow(&win);

                // Show when the webview signals it's ready (avoids white flash)
                let handle_for_show = handle.clone();
                handle.once("pill-ready", move |_| {
                    if let Some(w) = handle_for_show.get_webview_window("pill") {
                        let _ = w.show();
                    }
                });
            }
            Err(e) => log::error!("Failed to create pill window: {}", e),
        }
    });
}

pub fn close_pill_window(app: &AppHandle) {
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        if let Some(win) = handle.get_webview_window("pill") {
            let _ = win.destroy();
        }
    });
}

// -- Pill NSWindow configuration (macOS) --

#[cfg(target_os = "macos")]
fn configure_pill_nswindow(win: &tauri::WebviewWindow) {
    use objc2::msg_send;
    use objc2::runtime::AnyObject;
    use objc2_foundation::NSPoint;

    let ns_win: *mut AnyObject = win.ns_window().unwrap() as *mut AnyObject;

    // SAFETY: ns_win is a valid NSWindow pointer from Tauri's ns_window().
    // All msg_send! calls use standard NSWindow/NSColor selectors.
    // setLevel:3 = NSFloatingWindowLevel, collectionBehavior:17 = canJoinAllSpaces|stationary.
    unsafe {
        let _: () = msg_send![ns_win, setOpaque: false];
        let clear_color: *mut AnyObject =
            msg_send![objc2::runtime::AnyClass::get(c"NSColor").unwrap(), clearColor];
        let _: () = msg_send![ns_win, setBackgroundColor: clear_color];
        let _: () = msg_send![ns_win, setHasShadow: true];
        let _: () = msg_send![ns_win, setIgnoresMouseEvents: true];
        let _: () = msg_send![ns_win, setLevel: 3i64];
        let _: () = msg_send![ns_win, setCollectionBehavior: 17u64];

        // Position near top-center of screen (like Swift: 40px from top)
        let screen: *mut AnyObject = msg_send![ns_win, screen];
        if !screen.is_null() {
            let screen_frame: objc2_foundation::NSRect = msg_send![screen, frame];
            let x = (screen_frame.size.width - PILL_WIDTH) / 2.0;
            let y = screen_frame.origin.y + screen_frame.size.height - PILL_HEIGHT - PILL_TOP_OFFSET;
            let _: () = msg_send![ns_win, setFrameOrigin: NSPoint::new(x, y)];
        }

        // Make the webview background transparent
        let content_view: *mut AnyObject = msg_send![ns_win, contentView];
        if !content_view.is_null() {
            set_subviews_transparent(content_view);
        }
    }
}

#[cfg(target_os = "macos")]
unsafe fn set_subviews_transparent(content_view: *mut objc2::runtime::AnyObject) {
    use objc2::msg_send;
    use objc2::runtime::{AnyObject, Bool};

    let sel = objc2::sel!(setDrawsBackground:);
    let subviews: *mut AnyObject = msg_send![content_view, subviews];
    let count: usize = msg_send![subviews, count];
    for i in 0..count {
        let subview: *mut AnyObject = msg_send![subviews, objectAtIndex: i];
        let responds: Bool = msg_send![subview, respondsToSelector: sel];
        if responds.as_bool() {
            let _: () = msg_send![subview, setDrawsBackground: false];
        }
    }
}

// -- Tray menu --

fn build_menu(app: &AppHandle) -> Result<Menu<tauri::Wry>, Box<dyn std::error::Error>> {
    let state = get_state(app);

    // Audio device submenu
    let devices = audio_devices::list_input_devices();
    let selected_uid = state.selected_input_device_uid.lock().unwrap().clone();
    let uid_valid = selected_uid
        .as_ref()
        .is_some_and(|uid| devices.iter().any(|d| &d.uid == uid));
    let effective_uid = if uid_valid { selected_uid } else { None };

    let active_device = devices
        .iter()
        .find(|d| match &effective_uid {
            Some(uid) => uid == &d.uid,
            None => d.is_default,
        })
        .map(|d| format!("{} {}", d.transport_type.icon(), d.name))
        .unwrap_or_else(|| "Microphone".to_string());

    let mic_submenu = Submenu::with_id(app, "mic_submenu", &active_device, true)?;
    for device in &devices {
        let is_selected = match &effective_uid {
            Some(uid) => uid == &device.uid,
            None => device.is_default,
        };
        let default_tag = if device.is_default { " (Default)" } else { "" };
        let label = format!("{} {}{}", device.transport_type.icon(), device.name, default_tag);
        mic_submenu.append(&CheckMenuItem::with_id(
            app,
            format!("device_{}", device.uid),
            &label,
            true,
            is_selected,
            None::<&str>,
        )?)?;
    }
    if devices.is_empty() {
        mic_submenu.append(&MenuItem::with_id(app, "no_devices", "No input devices", false, None::<&str>)?)?;
    }

    // Language submenu
    let languages = common_languages();
    let selected_lang = state.selected_language.lock().unwrap().clone();
    let active_lang = languages
        .iter()
        .find(|l| l.code == selected_lang)
        .map(|l| l.label.clone())
        .unwrap_or_else(|| "Language".to_string());

    let lang_submenu = Submenu::with_id(app, "lang_submenu", &active_lang, true)?;
    for lang in &languages {
        lang_submenu.append(&CheckMenuItem::with_id(
            app,
            format!("lang_{}", lang.code),
            &lang.label,
            true,
            lang.code == selected_lang,
            None::<&str>,
        )?)?;
    }

    // Model submenu
    let api_servers = state.api_servers.lock().unwrap().clone();
    let downloaded = EngineCatalog::new(&api_servers).downloaded_models();
    let selected_model_id = state.selected_model_id.lock().unwrap().clone();
    let active_model = downloaded
        .iter()
        .find(|m| m.id == selected_model_id)
        .map(|m| m.label.clone())
        .unwrap_or_else(|| "Model".to_string());

    let model_submenu = Submenu::with_id(app, "model_submenu", &active_model, true)?;
    for model in &downloaded {
        model_submenu.append(&CheckMenuItem::with_id(
            app,
            format!("model_{}", model.id),
            &model.label,
            true,
            model.id == selected_model_id,
            None::<&str>,
        )?)?;
    }
    if !downloaded.is_empty() {
        model_submenu.append(&PredefinedMenuItem::separator(app)?)?;
    }
    model_submenu.append(&MenuItem::with_id(
        app,
        "model_manager",
        "Manage Models\u{2026}",
        true,
        None::<&str>,
    )?)?;

    let menu = Menu::with_items(
        app,
        &[
            &MenuItem::with_id(app, "title", "WhisperDictate", false, None::<&str>)?,
            &PredefinedMenuItem::separator(app)?,
            &mic_submenu,
            &model_submenu,
            &lang_submenu,
            &MenuItem::with_id(app, "setup", "Setup\u{2026}", true, None::<&str>)?,
            &PredefinedMenuItem::separator(app)?,
            #[cfg(debug_assertions)]
            &MenuItem::with_id(app, "test_pill", "Test Pill States", true, None::<&str>)?,
            #[cfg(debug_assertions)]
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(app, "quit", "Quit", true, Some("CmdOrCtrl+Q"))?,
        ],
    )?;

    Ok(menu)
}

/// Update a preference from a menu selection and rebuild the tray menu.
fn handle_selection(app: &AppHandle, prefix: &str, value: &str) {
    let state = get_state(app);
    match prefix {
        "device" => *state.selected_input_device_uid.lock().unwrap() = Some(value.to_string()),
        "model" => *state.selected_model_id.lock().unwrap() = value.to_string(),
        "lang" => *state.selected_language.lock().unwrap() = value.to_string(),
        _ => return,
    }
    state.save_preferences();
    log::info!("Selected {}: {}", prefix, value);
    rebuild_menu(app);
}

fn rebuild_menu(app: &AppHandle) {
    if let Ok(new_menu) = build_menu(app) {
        if let Some(tray) = app.tray_by_id("main") {
            let _ = tray.set_menu(Some(new_menu));
        }
    }
}

pub fn setup_tray(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let menu = build_menu(app)?;

    let _tray = TrayIconBuilder::with_id("main")
        .icon(app.default_window_icon().unwrap().clone())
        .icon_as_template(true)
        .tooltip("WhisperDictate")
        .on_menu_event(move |app, event| {
            let id = event.id().0.as_str();
            match id {
                "quit" => std::process::exit(0),
                "model_manager" => {
                    open_window(app, "model-manager", "Model Manager", "/model-manager", 700.0, 500.0);
                }
                "setup" => {
                    open_window(app, "setup", "Setup", "/setup", 420.0, 380.0);
                }
                "test_pill" => {
                    let app_clone = app.clone();
                    tauri::async_runtime::spawn(async move {
                        crate::commands::simulate_pill_test(app_clone, Some(3)).await;
                    });
                }
                _ => {
                    // Handle prefixed selections: device_*, model_*, lang_*
                    if let Some((prefix, value)) = id.split_once('_') {
                        handle_selection(app, prefix, value);
                    }
                }
            }
        })
        .build(app)?;

    // Attach menu after build (avoids macOS first-click-closes quirk)
    if let Some(tray) = app.tray_by_id("main") {
        let _ = tray.set_menu(Some(menu));
    }

    // Rebuild menu on audio device changes
    let app_handle = app.clone();
    audio_devices::start_device_change_listener(move || {
        log::info!("Audio devices changed, rebuilding tray menu");
        rebuild_menu(&app_handle);
    });

    Ok(())
}
