use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
    AppHandle, Manager, WebviewUrl, WebviewWindowBuilder,
};

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
                let _ = win.show();
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

pub fn setup_tray(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let quit = MenuItem::with_id(app, "quit", "Quit WhisperDictate", true, Some("CmdOrCtrl+Q"))?;
    let model_manager =
        MenuItem::with_id(app, "model_manager", "Manage Models\u{2026}", true, None::<&str>)?;
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
        .on_menu_event(move |app, event| match event.id().as_ref() {
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
            _ => {}
        })
        .build(app)?;

    Ok(())
}
