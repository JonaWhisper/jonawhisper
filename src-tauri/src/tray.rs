use crate::engines::{common_languages, EngineCatalog};
use crate::platform::audio_devices;
use crate::state::AppState;
use std::sync::Arc;
use tauri::{
    image::Image,
    menu::{Menu, MenuItem, PredefinedMenuItem, Submenu},
    tray::TrayIconBuilder,
    AppHandle, Listener, Manager, WebviewUrl, WebviewWindowBuilder,
};

const PILL_WIDTH: f64 = 80.0;
const PILL_HEIGHT: f64 = 32.0;
const PILL_TOP_OFFSET: f64 = 40.0;

// Tray icon size (44px for @2x Retina)
const TRAY_ICON_SIZE: u32 = 44;

fn get_state(app: &AppHandle) -> Arc<AppState> {
    app.state::<Arc<AppState>>().inner().clone()
}

// -- Window management --

pub fn open_window(app: &AppHandle, label: &str, title: &str, url: &str, width: f64, height: f64) {
    open_window_with_min(app, label, title, url, width, height, None);
}

pub fn open_window_with_min(
    app: &AppHandle,
    label: &str,
    title: &str,
    url: &str,
    width: f64,
    height: f64,
    min_size: Option<(f64, f64)>,
) {
    if let Some(window) = app.get_webview_window(label) {
        activate_app();
        let _ = window.set_focus();
        return;
    }

    let mut builder = WebviewWindowBuilder::new(app, label, WebviewUrl::App(url.into()))
        .title(title)
        .inner_size(width, height)
        .resizable(true);

    if let Some((min_w, min_h)) = min_size {
        builder = builder.min_inner_size(min_w, min_h);
    }

    if let Ok(win) = builder.build() {
        activate_app();
        let _ = win.set_focus();
    }
}

/// Activate the app so windows can become key/focused (needed for Accessory policy).
#[cfg(target_os = "macos")]
fn activate_app() {
    use objc2::msg_send;
    use objc2::runtime::AnyObject;
    unsafe {
        let ns_app: *mut AnyObject = msg_send![
            objc2::runtime::AnyClass::get(c"NSApplication").unwrap(),
            sharedApplication
        ];
        let _: () = msg_send![ns_app, activateIgnoringOtherApps: true];
    }
}

#[cfg(not(target_os = "macos"))]
fn activate_app() {}

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
    set_tray_state(app, "idle");
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
    let (selected_uid, selected_lang, api_servers, selected_model_id) = {
        let s = state.settings.lock().unwrap();
        (
            s.selected_input_device_uid.clone(),
            s.selected_language.clone(),
            s.api_servers.clone(),
            s.selected_model_id.clone(),
        )
    };
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
        let check = if is_selected { "✓ " } else { "   " };
        let label = format!("{}{} {}{}", check, device.transport_type.icon(), device.name, default_tag);
        mic_submenu.append(&MenuItem::with_id(
            app,
            format!("device_{}", device.uid),
            &label,
            true,
            None::<&str>,
        )?)?;
    }
    if devices.is_empty() {
        mic_submenu.append(&MenuItem::with_id(app, "no_devices", "No input devices", false, None::<&str>)?)?;
    }

    // Language submenu
    let languages = common_languages();
    let active_lang = languages
        .iter()
        .find(|l| l.code == selected_lang)
        .map(|l| l.label.clone())
        .unwrap_or_else(|| "Language".to_string());

    let lang_submenu = Submenu::with_id(app, "lang_submenu", &active_lang, true)?;
    for lang in &languages {
        let check = if lang.code == selected_lang { "✓ " } else { "   " };
        let label = format!("{}{}", check, lang.label);
        lang_submenu.append(&MenuItem::with_id(
            app,
            format!("lang_{}", lang.code),
            &label,
            true,
            None::<&str>,
        )?)?;
    }

    // Model submenu
    let downloaded = EngineCatalog::new(&api_servers).downloaded_models();
    let active_model = downloaded
        .iter()
        .find(|m| m.id == selected_model_id)
        .map(|m| m.label.clone())
        .unwrap_or_else(|| "Model".to_string());

    let model_submenu = Submenu::with_id(app, "model_submenu", &active_model, true)?;
    for model in &downloaded {
        let check = if model.id == selected_model_id { "✓ " } else { "   " };
        let label = format!("{}{}", check, model.label);
        model_submenu.append(&MenuItem::with_id(
            app,
            format!("model_{}", model.id),
            &label,
            true,
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
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(app, "settings", "Settings\u{2026}", true, None::<&str>)?,
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
    {
        let mut s = state.settings.lock().unwrap();
        match prefix {
            "device" => s.selected_input_device_uid = Some(value.to_string()),
            "model" => s.selected_model_id = value.to_string(),
            "lang" => s.selected_language = value.to_string(),
            _ => return,
        }
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

// -- Dynamic tray icon --

/// Update tray icon and tooltip based on app state.
pub fn set_tray_state(app: &AppHandle, state: &str) {
    let Some(tray) = app.tray_by_id("main") else { return };

    match state {
        "recording" => {
            let _ = tray.set_icon(Some(make_recording_icon()));
            let _ = tray.set_icon_as_template(true);
            let _ = tray.set_tooltip(Some("Recording\u{2026}"));
        }
        "transcribing" => {
            let _ = tray.set_icon(Some(make_transcribing_icon()));
            let _ = tray.set_icon_as_template(true);
            let _ = tray.set_tooltip(Some("Transcribing\u{2026}"));
        }
        _ => {
            if let Some(icon) = app.default_window_icon() {
                let _ = tray.set_icon(Some(icon.clone()));
                let _ = tray.set_icon_as_template(true);
            }
            let _ = tray.set_tooltip(Some("WhisperDictate"));
        }
    }
}

// -- Icon SDF helpers --

fn sdf_aa(d: f32) -> f32 {
    (0.5 - d).clamp(0.0, 1.0)
}

/// Signed distance to a rounded rectangle.
fn sdf_rrect(px: f32, py: f32, cx: f32, cy: f32, hw: f32, hh: f32, r: f32) -> f32 {
    let qx = (px - cx).abs() - (hw - r).max(0.0);
    let qy = (py - cy).abs() - (hh - r).max(0.0);
    (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt() + qx.max(qy).min(0.0) - r
}

fn sdf_circle(px: f32, py: f32, cx: f32, cy: f32, r: f32) -> f32 {
    ((px - cx).powi(2) + (py - cy).powi(2)).sqrt() - r
}

#[allow(clippy::too_many_arguments)]
fn point_in_triangle(px: f32, py: f32, x1: f32, y1: f32, x2: f32, y2: f32, x3: f32, y3: f32) -> bool {
    let d1 = (px - x2) * (y1 - y2) - (x1 - x2) * (py - y2);
    let d2 = (px - x3) * (y2 - y3) - (x2 - x3) * (py - y3);
    let d3 = (px - x1) * (y3 - y1) - (x3 - x1) * (py - y1);
    !(d1 < 0.0 && (d2 > 0.0 || d3 > 0.0)) && !(d1 > 0.0 && (d2 < 0.0 || d3 < 0.0))
}

/// Microphone with sound wave arcs (recording state), 44x44 RGBA template.
fn make_recording_icon() -> Image<'static> {
    let s = TRAY_ICON_SIZE as usize;
    let mut rgba = vec![0u8; s * s * 4];
    let lw = 2.2_f32;

    for y in 0..s {
        for x in 0..s {
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;
            let mut a = 0.0_f32;

            // Filled mic capsule (pill shape)
            a = a.max(sdf_aa(sdf_rrect(px, py, 22.0, 13.0, 5.0, 9.0, 5.0)));

            // Holder arc (U-shape below capsule)
            if py >= 22.0 {
                let ring = sdf_circle(px, py, 22.0, 22.0, 9.0).abs() - lw / 2.0;
                a = a.max(sdf_aa(ring));
            }
            // Vertical connections from capsule to holder
            a = a.max(sdf_aa(sdf_rrect(px, py, 13.0, 20.5, lw / 2.0, 2.5, 0.0)));
            a = a.max(sdf_aa(sdf_rrect(px, py, 31.0, 20.5, lw / 2.0, 2.5, 0.0)));

            // Stand
            a = a.max(sdf_aa(sdf_rrect(px, py, 22.0, 34.0, lw / 2.0, 3.0, 0.0)));

            // Base
            a = a.max(sdf_aa(sdf_rrect(px, py, 22.0, 37.5, 5.5, lw / 2.0, lw / 2.0)));

            // Sound wave arcs (recording indicator)
            for &(radius, min_dx) in &[(14.0_f32, 8.0_f32), (18.0, 12.0)] {
                if (px - 22.0).abs() > min_dx && py < 19.0 {
                    let ring = sdf_circle(px, py, 22.0, 13.0, radius).abs() - lw * 0.45;
                    a = a.max(sdf_aa(ring));
                }
            }

            if a > 0.0 {
                rgba[(y * s + x) * 4 + 3] = (a * 255.0) as u8;
            }
        }
    }

    Image::new_owned(rgba, TRAY_ICON_SIZE, TRAY_ICON_SIZE)
}

/// Speech bubble with three dots (transcribing state), 44x44 RGBA template.
fn make_transcribing_icon() -> Image<'static> {
    let s = TRAY_ICON_SIZE as usize;
    let mut rgba = vec![0u8; s * s * 4];

    for y in 0..s {
        for x in 0..s {
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;

            // Filled rounded-rect bubble
            let bubble = sdf_rrect(px, py, 22.0, 17.0, 18.0, 11.0, 6.0);
            // Small tail at bottom-left (triangle)
            let in_tail = point_in_triangle(px, py, 8.0, 26.5, 16.0, 26.5, 6.0, 35.0);

            let shape_alpha = if in_tail { 1.0 } else { sdf_aa(bubble) };

            // Three dot cutouts
            let dots = sdf_circle(px, py, 14.0, 17.0, 2.8)
                .min(sdf_circle(px, py, 22.0, 17.0, 2.8))
                .min(sdf_circle(px, py, 30.0, 17.0, 2.8));
            let dot_alpha = sdf_aa(dots);

            // Shape minus dots
            let a = shape_alpha * (1.0 - dot_alpha);

            if a > 0.001 {
                rgba[(y * s + x) * 4 + 3] = (a * 255.0) as u8;
            }
        }
    }

    Image::new_owned(rgba, TRAY_ICON_SIZE, TRAY_ICON_SIZE)
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
                "quit" => app.exit(0),
                "model_manager" => {
                    open_window(app, "model-manager", "Model Manager", "/model-manager", 700.0, 500.0);
                }
                "settings" => {
                    open_window_with_min(app, "settings", "Settings", "/settings", 580.0, 420.0, Some((460.0, 320.0)));
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
