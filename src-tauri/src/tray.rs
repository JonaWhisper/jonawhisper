use crate::engines::{common_languages, EngineCatalog};
use crate::platform::audio_devices;
use crate::state::AppState;
use std::sync::Arc;
use tauri::{
    menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu},
    tray::TrayIconBuilder,
    AppHandle, Manager, WebviewUrl, WebviewWindowBuilder,
};

/// Helper: get a cloned Arc<AppState> from the app handle.
fn get_state(app: &AppHandle) -> Arc<AppState> {
    app.state::<Arc<AppState>>().inner().clone()
}

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
    // Window creation must happen on the main thread for Accessory apps
    let _ = app.run_on_main_thread(move || {
        match WebviewWindowBuilder::new(&handle, "pill", WebviewUrl::App("/pill".into()))
            .decorations(false)
            .transparent(true)
            .always_on_top(true)
            .inner_size(140.0, 56.0)
            .resizable(false)
            .visible(false)
            .build()
        {
            Ok(win) => {
                // Configure NSWindow for true transparency (like the Swift version)
                #[cfg(target_os = "macos")]
                {
                    configure_pill_nswindow(&win);
                }
                // Delay show to let webview render with transparent background
                // (avoids white flash on first appearance)
                let handle_for_show = handle.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_millis(150));
                    if let Some(w) = handle_for_show.get_webview_window("pill") {
                        let _ = w.show();
                    }
                });
                log::info!("Pill window created");
            }
            Err(e) => log::error!("Failed to create pill window: {}", e),
        }
    });
}

/// Configure the pill NSWindow for transparent floating appearance.
/// Mirrors the Swift FloatingPill: borderless, transparent, floating, ignores mouse.
#[cfg(target_os = "macos")]
fn configure_pill_nswindow(win: &tauri::WebviewWindow) {
    use objc2::msg_send;
    use objc2::runtime::{AnyObject, Bool};
    use objc2_foundation::NSPoint;

    let ns_win: *mut AnyObject = win.ns_window().unwrap() as *mut AnyObject;

    // SAFETY: ns_win is a valid NSWindow pointer from Tauri's ns_window().
    // All msg_send! calls use standard NSWindow/NSColor selectors.
    // setLevel:3 = NSFloatingWindowLevel, collectionBehavior:17 = canJoinAllSpaces|stationary.
    unsafe {
        // isOpaque = false
        let _: () = msg_send![ns_win, setOpaque: false];
        // backgroundColor = [NSColor clearColor]
        let clear_color: *mut AnyObject =
            msg_send![objc2::runtime::AnyClass::get(c"NSColor").unwrap(), clearColor];
        let _: () = msg_send![ns_win, setBackgroundColor: clear_color];
        // hasShadow = true
        let _: () = msg_send![ns_win, setHasShadow: true];
        // ignoresMouseEvents = true
        let _: () = msg_send![ns_win, setIgnoresMouseEvents: true];
        // level = NSFloatingWindowLevel (3)
        let _: () = msg_send![ns_win, setLevel: 3i64];
        // collectionBehavior = .canJoinAllSpaces | .stationary (1 << 0 | 1 << 4 = 17)
        let _: () = msg_send![ns_win, setCollectionBehavior: 17u64];

        // Position near top-center of screen (like Swift: 40px from top)
        let screen: *mut AnyObject = msg_send![ns_win, screen];
        if !screen.is_null() {
            let screen_frame: objc2_foundation::NSRect = msg_send![screen, frame];
            let pill_w = 140.0_f64;
            let pill_h = 56.0_f64;
            let x = (screen_frame.size.width - pill_w) / 2.0;
            // NSWindow origin is bottom-left, so top = maxY - height - offset
            let y = screen_frame.origin.y + screen_frame.size.height - pill_h - 40.0;
            let origin = NSPoint::new(x, y);
            let _: () = msg_send![ns_win, setFrameOrigin: origin];
        }

        // Make the webview background transparent: find subviews that respond to setDrawsBackground:
        let content_view: *mut AnyObject = msg_send![ns_win, contentView];
        if !content_view.is_null() {
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
    }
}

pub fn close_pill_window(app: &AppHandle) {
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        if let Some(win) = handle.get_webview_window("pill") {
            let _ = win.destroy();
            log::info!("Pill window closed");
        }
    });
}

fn build_menu(app: &AppHandle) -> Result<Menu<tauri::Wry>, Box<dyn std::error::Error>> {
    let state = get_state(app);

    // Audio device submenu — title shows selected device name
    let devices = audio_devices::list_input_devices();
    let selected_uid = state.selected_input_device_uid.lock().unwrap().clone();

    // If saved device no longer exists, fall back to system default
    let uid_valid = selected_uid.as_ref().is_some_and(|uid| {
        devices.iter().any(|d| &d.uid == uid)
    });
    let effective_uid = if uid_valid { selected_uid } else { None };

    let mut active_name = String::from("Microphone");
    for device in &devices {
        let is_selected = match &effective_uid {
            Some(uid) => uid == &device.uid,
            None => device.is_default,
        };
        if is_selected {
            active_name = format!("{} {}", device.transport_type.icon(), device.name);
        }
    }

    let mic_submenu = Submenu::with_id(app, "mic_submenu", &active_name, true)?;

    for device in &devices {
        let is_selected = match &effective_uid {
            Some(uid) => uid == &device.uid,
            None => device.is_default,
        };
        let icon = device.transport_type.icon();
        let default_tag = if device.is_default { " (Default)" } else { "" };
        let label = format!("{} {}{}", icon, device.name, default_tag);
        let item = CheckMenuItem::with_id(
            app,
            format!("device_{}", device.uid),
            &label,
            true,
            is_selected,
            None::<&str>,
        )?;
        mic_submenu.append(&item)?;
    }

    if devices.is_empty() {
        let empty = MenuItem::with_id(app, "no_devices", "No input devices", false, None::<&str>)?;
        mic_submenu.append(&empty)?;
    }

    // Language submenu — title shows selected language
    let languages = common_languages();
    let selected_lang = state.selected_language.lock().unwrap().clone();

    let mut active_lang_label = String::from("Language");
    for lang in &languages {
        if lang.code == selected_lang {
            active_lang_label = lang.label.clone();
        }
    }

    let lang_submenu = Submenu::with_id(app, "lang_submenu", &active_lang_label, true)?;
    for lang in &languages {
        let is_selected = lang.code == selected_lang;
        let item = CheckMenuItem::with_id(
            app,
            format!("lang_{}", lang.code),
            &lang.label,
            true,
            is_selected,
            None::<&str>,
        )?;
        lang_submenu.append(&item)?;
    }

    // Model submenu — only downloaded models, title shows selected
    let api_servers = state.api_servers.lock().unwrap().clone();
    let catalog = EngineCatalog::new(&api_servers);
    let downloaded = catalog.downloaded_models();
    let selected_model_id = state.selected_model_id.lock().unwrap().clone();

    let mut active_model_label = String::from("Model");
    for model in &downloaded {
        if model.id == selected_model_id {
            active_model_label = model.label.clone();
        }
    }

    let model_submenu = Submenu::with_id(app, "model_submenu", &active_model_label, true)?;
    for model in &downloaded {
        let is_selected = model.id == selected_model_id;
        let item = CheckMenuItem::with_id(
            app,
            format!("model_{}", model.id),
            &model.label,
            true,
            is_selected,
            None::<&str>,
        )?;
        model_submenu.append(&item)?;
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

pub fn setup_tray(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let menu = build_menu(app)?;

    let tray = TrayIconBuilder::with_id("main")
        .icon(app.default_window_icon().unwrap().clone())
        .icon_as_template(true)
        .tooltip("WhisperDictate")
        .menu(&menu)
        .on_menu_event(move |app, event| {
            let id = event.id().0.as_str();
            match id {
                "quit" => {
                    std::process::exit(0);
                }
                "model_manager" => {
                    open_window(
                        app,
                        "model-manager",
                        "Model Manager",
                        "/model-manager",
                        700.0,
                        500.0,
                    );
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
                _ if id.starts_with("device_") => {
                    let uid = id.strip_prefix("device_").unwrap().to_string();
                    let state = get_state(app);
                    *state.selected_input_device_uid.lock().unwrap() = Some(uid.clone());
                    state.save_preferences();
                    log::info!("Selected audio device: {}", uid);
                    if let Ok(new_menu) = build_menu(app) {
                        if let Some(tray) = app.tray_by_id("main") {
                            let _ = tray.set_menu(Some(new_menu));
                        }
                    }
                }
                _ if id.starts_with("model_") => {
                    let model_id = id.strip_prefix("model_").unwrap().to_string();
                    let state = get_state(app);
                    *state.selected_model_id.lock().unwrap() = model_id.clone();
                    state.save_preferences();
                    log::info!("Selected model: {}", model_id);
                    if let Ok(new_menu) = build_menu(app) {
                        if let Some(tray) = app.tray_by_id("main") {
                            let _ = tray.set_menu(Some(new_menu));
                        }
                    }
                }
                _ if id.starts_with("lang_") => {
                    let code = id.strip_prefix("lang_").unwrap().to_string();
                    let state = get_state(app);
                    *state.selected_language.lock().unwrap() = code.clone();
                    state.save_preferences();
                    log::info!("Selected language: {}", code);
                    if let Ok(new_menu) = build_menu(app) {
                        if let Some(tray) = app.tray_by_id("main") {
                            let _ = tray.set_menu(Some(new_menu));
                        }
                    }
                }
                _ => {}
            }
        })
        .build(app)?;

    // Listen for audio device changes and rebuild menu
    let app_handle = app.clone();
    audio_devices::start_device_change_listener(move || {
        log::info!("Audio devices changed, rebuilding tray menu");
        if let Ok(new_menu) = build_menu(&app_handle) {
            let _ = tray.set_menu(Some(new_menu));
        }
    });

    Ok(())
}
