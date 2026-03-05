use tauri::AppHandle;
use tauri_plugin_clipboard_manager::ClipboardExt;

/// Write text to clipboard and simulate paste keystroke.
pub fn paste_text(app: &AppHandle, text: &str) {
    if let Err(e) = app.clipboard().write_text(text) {
        log::error!("Failed to write to clipboard: {}", e);
        return;
    }

    // Small delay to ensure clipboard is ready
    std::thread::sleep(std::time::Duration::from_millis(50));

    simulate_paste();

    // Allow paste to complete before next operation
    std::thread::sleep(std::time::Duration::from_millis(50));
}

/// Simulate Cmd+V on macOS.
#[cfg(target_os = "macos")]
fn simulate_paste() {
    use core_graphics::event::{CGEvent, CGEventFlags};
    use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};

    let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
        .expect("Failed to create event source");

    // Key code 9 = 'V'
    let key_down = CGEvent::new_keyboard_event(source.clone(), 9, true)
        .expect("Failed to create key down event");
    key_down.set_flags(CGEventFlags::CGEventFlagCommand);

    let key_up = CGEvent::new_keyboard_event(source, 9, false)
        .expect("Failed to create key up event");
    key_up.set_flags(CGEventFlags::CGEventFlagCommand);

    key_down.post(core_graphics::event::CGEventTapLocation::HID);
    key_up.post(core_graphics::event::CGEventTapLocation::HID);
}

/// Simulate Ctrl+V on Windows.
#[cfg(target_os = "windows")]
fn simulate_paste() {
    // TODO: implement with SendInput
    log::warn!("Paste simulation not yet implemented for Windows");
}

/// Stub for unsupported platforms.
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn simulate_paste() {
    log::warn!("Paste simulation not implemented for this platform");
}
