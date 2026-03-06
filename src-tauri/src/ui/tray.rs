use super::menu_icons::{self, sdf_aa, sdf_circle, sdf_rrect, sdf_segment};
use crate::platform::audio_devices;
use crate::state::AppState;
use rust_i18n::t;
use std::sync::Arc;
use std::sync::LazyLock;
use tauri::{
    image::Image,
    menu::{IconMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu},
    tray::TrayIconBuilder,
    AppHandle, Manager, WebviewUrl, WebviewWindowBuilder,
};

// Cached tray icons (static bitmaps, computed once)
static IDLE_ICON: LazyLock<Image<'static>> = LazyLock::new(render_idle_icon);
static RECORDING_ICON: LazyLock<Image<'static>> = LazyLock::new(render_recording_icon);
static TRANSCRIBING_ICON: LazyLock<Image<'static>> = LazyLock::new(render_transcribing_icon);

/// Holds references to menu items for incremental updates.
pub struct TrayMenuState {
    pub menu: Menu<tauri::Wry>,
    pub mic_submenu: Submenu<tauri::Wry>,
    pub panel_item: MenuItem<tauri::Wry>,
    pub quit_item: MenuItem<tauri::Wry>,
}

// Tray icon size (44px for @2x Retina)
const TRAY_ICON_SIZE: u32 = 44;

fn get_state(app: &AppHandle) -> Arc<AppState> {
    app.state::<Arc<AppState>>().inner().clone()
}

// -- Window management --

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
    super::pill::open(app, super::pill::PillMode::Recording);
}

pub fn close_pill_window(app: &AppHandle) {
    set_tray_state(app, "idle");
    super::pill::close(app);
}

// -- Tray menu --

fn build_initial_menu(app: &AppHandle) -> Result<TrayMenuState, Box<dyn std::error::Error>> {
    let mic_submenu = Submenu::with_id(app, "mic_submenu", &t!("menu.microphone"), true)?;

    let panel_item = MenuItem::with_id(app, "panel", &t!("menu.panel"), true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", &t!("menu.quit"), true, Some("CmdOrCtrl+Q"))?;

    let menu = Menu::with_items(
        app,
        &[
            &MenuItem::with_id(app, "title", "JonaWhisper", false, None::<&str>)?,
            &PredefinedMenuItem::separator(app)?,
            &mic_submenu,
            &PredefinedMenuItem::separator(app)?,
            &panel_item,
            &PredefinedMenuItem::separator(app)?,
            #[cfg(debug_assertions)]
            &MenuItem::with_id(app, "test_pill", "Test Pill States", true, None::<&str>)?,
            #[cfg(debug_assertions)]
            &MenuItem::with_id(app, "open_setup", "Setup Wizard", true, None::<&str>)?,
            #[cfg(debug_assertions)]
            &PredefinedMenuItem::separator(app)?,
            &quit_item,
        ],
    )?;

    Ok(TrayMenuState {
        menu,
        mic_submenu,
        panel_item,
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

    let _ = m.panel_item.set_text(&t!("menu.panel"));
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
            let _ = tray.set_icon(Some(RECORDING_ICON.clone()));
            let _ = tray.set_icon_as_template(true);
            let _ = tray.set_tooltip(Some(&t!("pill.recording")));
        }
        "transcribing" => {
            let _ = tray.set_icon(Some(TRANSCRIBING_ICON.clone()));
            let _ = tray.set_icon_as_template(true);
            let _ = tray.set_tooltip(Some(&t!("pill.transcribing")));
        }
        _ => {
            let _ = tray.set_icon(Some(IDLE_ICON.clone()));
            let _ = tray.set_icon_as_template(true);
            let _ = tray.set_tooltip(Some(&t!("app.name")));
        }
    }
}

// -- Tray bar icons (44×44, Lucide-inspired SDF) --

/// Idle state: simple microphone (Lucide mic.svg), 44×44 RGBA template.
fn render_idle_icon() -> Image<'static> {
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
fn render_recording_icon() -> Image<'static> {
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
fn render_transcribing_icon() -> Image<'static> {
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
        .icon(IDLE_ICON.clone())
        .icon_as_template(true)
        .tooltip(&t!("app.name"))
        .on_menu_event(move |app, event| {
            let id = event.id().0.as_str();
            match id {
                "quit" => app.exit(0),
                "panel" => {
                    open_window_with_min(app, "panel", &t!("window.panel"), "/panel", 750.0, 550.0, Some((680.0, 450.0)));
                }
                "test_pill" => {
                    let app_clone = app.clone();
                    tauri::async_runtime::spawn(async move {
                        crate::commands::simulate_pill_test(app_clone, Some(3)).await;
                    });
                }
                "open_setup" => {
                    open_fixed_window(app, "setup", &t!("window.setup"), "/setup", 420.0, 450.0);
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
