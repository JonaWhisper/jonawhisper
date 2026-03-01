use crate::menu_icons::{self, sdf_aa, sdf_circle, sdf_rrect, sdf_segment};
use crate::platform::audio_devices;
use crate::state::AppState;
use rust_i18n::t;
use std::sync::Arc;
use tauri::{
    image::Image,
    menu::{IconMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu},
    tray::TrayIconBuilder,
    AppHandle, Listener, Manager, WebviewUrl, WebviewWindowBuilder,
};

/// Holds references to menu items for incremental updates.
pub struct TrayMenuState {
    pub menu: Menu<tauri::Wry>,
    pub mic_submenu: Submenu<tauri::Wry>,
    pub settings_item: MenuItem<tauri::Wry>,
    pub models_item: MenuItem<tauri::Wry>,
    pub history_item: MenuItem<tauri::Wry>,
    pub setup_item: MenuItem<tauri::Wry>,
    pub quit_item: MenuItem<tauri::Wry>,
}

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

pub fn open_fixed_window(app: &AppHandle, label: &str, title: &str, url: &str, width: f64, height: f64) {
    if let Some(window) = app.get_webview_window(label) {
        activate_app();
        let _ = window.set_focus();
        return;
    }

    if let Ok(win) = WebviewWindowBuilder::new(app, label, WebviewUrl::App(url.into()))
        .title(title)
        .inner_size(width, height)
        .resizable(false)
        .build()
    {
        activate_app();
        let _ = win.set_focus();
    }
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
/// Uses `activate()` on macOS 14+ (cooperative activation), falls back to
/// `activateIgnoringOtherApps:` on macOS 13.
/// See: https://developer.apple.com/documentation/appkit/nsapplication/activate()
#[cfg(target_os = "macos")]
fn activate_app() {
    use objc2::msg_send;
    use objc2::runtime::AnyObject;
    unsafe {
        let ns_app: *mut AnyObject = msg_send![
            objc2::runtime::AnyClass::get(c"NSApplication").unwrap(),
            sharedApplication
        ];
        let info: *mut AnyObject = msg_send![
            objc2::runtime::AnyClass::get(c"NSProcessInfo").unwrap(),
            processInfo
        ];
        let version: objc2_foundation::NSOperatingSystemVersion = msg_send![info, operatingSystemVersion];
        if version.majorVersion >= 14 {
            let _: () = msg_send![ns_app, activate];
        } else {
            #[allow(deprecated)]
            let _: () = msg_send![ns_app, activateIgnoringOtherApps: true];
        }
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

fn build_initial_menu(app: &AppHandle) -> Result<TrayMenuState, Box<dyn std::error::Error>> {
    let mic_submenu = Submenu::with_id(app, "mic_submenu", &t!("menu.microphone"), true)?;

    let settings_item = MenuItem::with_id(app, "settings", &t!("menu.settings"), true, None::<&str>)?;
    let models_item = MenuItem::with_id(app, "model_manager", &t!("menu.manageModels"), true, None::<&str>)?;
    let history_item = MenuItem::with_id(app, "history", &t!("menu.history"), true, None::<&str>)?;
    let setup_item = MenuItem::with_id(app, "setup", &t!("menu.setup"), true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", &t!("menu.quit"), true, Some("CmdOrCtrl+Q"))?;

    let menu = Menu::with_items(
        app,
        &[
            &MenuItem::with_id(app, "title", "WhisperDictate", false, None::<&str>)?,
            &PredefinedMenuItem::separator(app)?,
            &mic_submenu,
            &PredefinedMenuItem::separator(app)?,
            &settings_item,
            &models_item,
            &history_item,
            &setup_item,
            &PredefinedMenuItem::separator(app)?,
            #[cfg(debug_assertions)]
            &MenuItem::with_id(app, "test_pill", "Test Pill States", true, None::<&str>)?,
            #[cfg(debug_assertions)]
            &PredefinedMenuItem::separator(app)?,
            &quit_item,
        ],
    )?;

    Ok(TrayMenuState {
        menu,
        mic_submenu,
        settings_item,
        models_item,
        history_item,
        setup_item,
        quit_item,
    })
}

/// Incrementally update the microphone submenu (no full menu rebuild).
pub fn update_mic_submenu(app: &AppHandle) {
    let state = get_state(app);
    let devices = audio_devices::list_input_devices();
    let selected_uid = {
        let s = state.settings.lock().unwrap();
        s.selected_input_device_uid.clone()
    };
    let uid_valid = selected_uid
        .as_ref()
        .is_some_and(|uid| devices.iter().any(|d| &d.uid == uid));
    let effective_uid = if uid_valid { selected_uid } else { None };

    let tray_menu = state.tray_menu.lock().unwrap();
    let Some(ref m) = *tray_menu else { return };

    // Remove all existing items from the submenu
    if let Ok(items) = m.mic_submenu.items() {
        for item in &items {
            let _ = m.mic_submenu.remove(item);
        }
    }

    // Add device items with colored bubble icons (blue=selected, gray=other)
    for device in &devices {
        let is_selected = match &effective_uid {
            Some(uid) => uid == &device.uid,
            None => device.is_default,
        };
        let default_tag = if device.is_default {
            format!(" ({})", t!("settings.microphone.defaultTag"))
        } else {
            String::new()
        };
        let label = format!("{}{}", device.name, default_tag);
        let icon = menu_icons::transport_icon(&device.transport_type, is_selected);
        if let Ok(item) = IconMenuItem::with_id(
            app,
            &format!("device_{}", device.uid),
            &label,
            true,
            Some(icon),
            None::<&str>,
        ) {
            let _ = m.mic_submenu.append(&item);
        }
    }

    if devices.is_empty() {
        if let Ok(item) = MenuItem::with_id(app, "no_devices", &t!("menu.noDevices"), false, None::<&str>) {
            let _ = m.mic_submenu.append(&item);
        }
    }

    // Update submenu header: blue icon of active device + name
    let active_device = devices.iter().find(|d| match &effective_uid {
        Some(uid) => uid == &d.uid,
        None => d.is_default,
    });
    if let Some(d) = active_device {
        let _ = m.mic_submenu.set_icon(Some(menu_icons::transport_icon_plain(&d.transport_type)));
        let _ = m.mic_submenu.set_text(&d.name);
    } else {
        let _ = m.mic_submenu.set_icon(None::<Image<'_>>);
        let _ = m.mic_submenu.set_text(&t!("menu.microphone"));
    }
}

/// Update all static menu item labels (called when locale changes).
pub fn update_tray_labels(app: &AppHandle) {
    let state = get_state(app);
    let tray_menu = state.tray_menu.lock().unwrap();
    let Some(ref m) = *tray_menu else { return };

    let _ = m.settings_item.set_text(&t!("menu.settings"));
    let _ = m.models_item.set_text(&t!("menu.manageModels"));
    let _ = m.history_item.set_text(&t!("menu.history"));
    let _ = m.setup_item.set_text(&t!("menu.setup"));
    let _ = m.quit_item.set_text(&t!("menu.quit"));

    drop(tray_menu);
    // Also refresh mic submenu (translated default tag)
    update_mic_submenu(app);
}

/// Update a preference from a menu selection and refresh the mic submenu.
fn handle_selection(app: &AppHandle, prefix: &str, value: &str) {
    let state = get_state(app);
    {
        let mut s = state.settings.lock().unwrap();
        match prefix {
            "device" => s.selected_input_device_uid = Some(value.to_string()),
            _ => return,
        }
    }
    state.save_preferences();
    log::info!("Selected {}: {}", prefix, value);
    update_mic_submenu(app);
}

// -- Dynamic tray icon --

/// Update tray icon and tooltip based on app state.
pub fn set_tray_state(app: &AppHandle, state: &str) {
    let Some(tray) = app.tray_by_id("main") else { return };

    match state {
        "recording" => {
            let _ = tray.set_icon(Some(make_recording_icon()));
            let _ = tray.set_icon_as_template(true);
            let _ = tray.set_tooltip(Some(&t!("pill.recording")));
        }
        "transcribing" => {
            let _ = tray.set_icon(Some(make_transcribing_icon()));
            let _ = tray.set_icon_as_template(true);
            let _ = tray.set_tooltip(Some(&t!("pill.transcribing")));
        }
        _ => {
            let _ = tray.set_icon(Some(make_idle_icon()));
            let _ = tray.set_icon_as_template(true);
            let _ = tray.set_tooltip(Some(&t!("app.name")));
        }
    }
}

// -- Tray bar icons (44×44, Lucide-inspired SDF) --

/// Idle state: simple microphone (Lucide mic.svg), 44×44 RGBA template.
fn make_idle_icon() -> Image<'static> {
    let s = TRAY_ICON_SIZE as usize;
    let mut rgba = vec![0u8; s * s * 4];
    let lw = 2.2_f32;

    for y in 0..s {
        for x in 0..s {
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;
            let mut a = 0.0_f32;

            // Mic capsule (filled pill)
            a = a.max(sdf_aa(sdf_rrect(px, py, 22.0, 13.0, 5.0, 9.0, 5.0)));

            // Holder arc (U-shape) — only below capsule
            if py >= 22.0 {
                let ring = sdf_circle(px, py, 22.0, 22.0, 9.0).abs() - lw / 2.0;
                a = a.max(sdf_aa(ring));
            }
            // Vertical connections from capsule to holder
            a = a.max(sdf_aa(sdf_rrect(px, py, 13.0, 20.5, lw / 2.0, 2.5, 0.0)));
            a = a.max(sdf_aa(sdf_rrect(px, py, 31.0, 20.5, lw / 2.0, 2.5, 0.0)));

            // Stand
            a = a.max(sdf_aa(sdf_segment(px, py, 22.0, 31.0, 22.0, 37.0) - lw / 2.0));

            if a > 0.0 {
                rgba[(y * s + x) * 4 + 3] = (a * 255.0) as u8;
            }
        }
    }
    Image::new_owned(rgba, TRAY_ICON_SIZE, TRAY_ICON_SIZE)
}

/// Recording state: audio bars (Lucide audio-lines.svg), 44×44 RGBA template.
fn make_recording_icon() -> Image<'static> {
    let s = TRAY_ICON_SIZE as usize;
    let mut rgba = vec![0u8; s * s * 4];
    let lw = 2.4_f32;

    // 5 vertical bars at different heights, centered on 22
    let bars: [(f32, f32, f32); 5] = [
        (8.0, 14.0, 30.0),   // bar 1
        (14.0, 8.0, 36.0),   // bar 2
        (22.0, 4.0, 40.0),   // bar 3 (tallest, center)
        (30.0, 10.0, 34.0),  // bar 4
        (36.0, 16.0, 28.0),  // bar 5 (shortest)
    ];

    for y in 0..s {
        for x in 0..s {
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;
            let mut a = 0.0_f32;

            for &(bx, top, bot) in &bars {
                let seg = sdf_segment(px, py, bx, top, bx, bot) - lw / 2.0;
                a = a.max(sdf_aa(seg));
            }

            // Round caps at ends
            for &(bx, top, bot) in &bars {
                a = a.max(sdf_aa(sdf_circle(px, py, bx, top, lw / 2.0)));
                a = a.max(sdf_aa(sdf_circle(px, py, bx, bot, lw / 2.0)));
            }

            if a > 0.0 {
                rgba[(y * s + x) * 4 + 3] = (a * 255.0) as u8;
            }
        }
    }
    Image::new_owned(rgba, TRAY_ICON_SIZE, TRAY_ICON_SIZE)
}

/// Transcribing state: speech bubble with text lines (Lucide message-square-text.svg), 44×44 RGBA template.
fn make_transcribing_icon() -> Image<'static> {
    let s = TRAY_ICON_SIZE as usize;
    let mut rgba = vec![0u8; s * s * 4];
    let lw = 2.2_f32;

    for y in 0..s {
        for x in 0..s {
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;
            let mut a = 0.0_f32;

            // Bubble outline (rounded rect)
            let bubble = sdf_rrect(px, py, 22.0, 18.0, 17.0, 13.0, 4.0).abs() - lw / 2.0;
            a = a.max(sdf_aa(bubble));

            // Tail: two lines forming a pointer at bottom-left
            let tail1 = sdf_segment(px, py, 10.0, 31.0, 6.0, 39.0) - lw / 2.0;
            a = a.max(sdf_aa(tail1));
            let tail2 = sdf_segment(px, py, 6.0, 39.0, 18.0, 31.0) - lw / 2.0;
            a = a.max(sdf_aa(tail2));

            // Three horizontal text lines inside bubble
            let line1 = sdf_segment(px, py, 12.0, 14.0, 32.0, 14.0) - lw * 0.4;
            a = a.max(sdf_aa(line1));
            let line2 = sdf_segment(px, py, 12.0, 19.0, 32.0, 19.0) - lw * 0.4;
            a = a.max(sdf_aa(line2));
            let line3 = sdf_segment(px, py, 12.0, 24.0, 26.0, 24.0) - lw * 0.4;
            a = a.max(sdf_aa(line3));

            if a > 0.0 {
                rgba[(y * s + x) * 4 + 3] = (a * 255.0) as u8;
            }
        }
    }
    Image::new_owned(rgba, TRAY_ICON_SIZE, TRAY_ICON_SIZE)
}

pub fn setup_tray(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let menu_state = build_initial_menu(app)?;

    let _tray = TrayIconBuilder::with_id("main")
        .icon(make_idle_icon())
        .icon_as_template(true)
        .tooltip(&t!("app.name"))
        .on_menu_event(move |app, event| {
            let id = event.id().0.as_str();
            match id {
                "quit" => app.exit(0),
                "settings" => {
                    open_window_with_min(app, "settings", &t!("window.settings"), "/settings", 600.0, 440.0, Some((580.0, 380.0)));
                }
                "model_manager" => {
                    open_window(app, "model-manager", &t!("window.modelManager"), "/model-manager", 700.0, 500.0);
                }
                "history" => {
                    open_window(app, "history", &t!("window.history"), "/history", 500.0, 500.0);
                }
                "setup" => {
                    open_fixed_window(app, "setup", &t!("window.setup"), "/setup", 420.0, 450.0);
                }
                "test_pill" => {
                    let app_clone = app.clone();
                    tauri::async_runtime::spawn(async move {
                        crate::commands::simulate_pill_test(app_clone, Some(3)).await;
                    });
                }
                _ => {
                    // Handle prefixed selections: device_*
                    if let Some((prefix, value)) = id.split_once('_') {
                        handle_selection(app, prefix, value);
                    }
                }
            }
        })
        .build(app)?;

    // Attach menu after build (avoids macOS first-click-closes quirk)
    if let Some(tray) = app.tray_by_id("main") {
        let _ = tray.set_menu(Some(menu_state.menu.clone()));
    }

    // Store menu refs for incremental updates
    {
        let state = get_state(app);
        *state.tray_menu.lock().unwrap() = Some(menu_state);
    }

    // Populate mic submenu with initial devices
    update_mic_submenu(app);

    // Update mic submenu on audio device changes (incremental, no flash)
    let app_handle = app.clone();
    audio_devices::start_device_change_listener(move || {
        log::info!("Audio devices changed, updating mic submenu");
        update_mic_submenu(&app_handle);
    });

    Ok(())
}
