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

    if let Err(e) = simulate_paste() {
        log::error!("Failed to simulate paste: {}", e);
        return;
    }

    // Allow paste to complete before next operation
    std::thread::sleep(std::time::Duration::from_millis(50));
}

/// Simulate Cmd+V on macOS.
#[cfg(target_os = "macos")]
fn simulate_paste() -> Result<(), String> {
    use core_graphics::event::{CGEvent, CGEventFlags};
    use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};

    let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
        .map_err(|_| "Failed to create CGEventSource".to_string())?;

    // Key code 9 = 'V'
    let key_down = CGEvent::new_keyboard_event(source.clone(), 9, true)
        .map_err(|_| "Failed to create key down event".to_string())?;
    key_down.set_flags(CGEventFlags::CGEventFlagCommand);

    let key_up = CGEvent::new_keyboard_event(source, 9, false)
        .map_err(|_| "Failed to create key up event".to_string())?;
    key_up.set_flags(CGEventFlags::CGEventFlagCommand);

    key_down.post(core_graphics::event::CGEventTapLocation::HID);
    key_up.post(core_graphics::event::CGEventTapLocation::HID);
    Ok(())
}

/// Simulate Ctrl+V on Windows.
#[cfg(target_os = "windows")]
fn simulate_paste() -> Result<(), String> {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, SendInput, VK_CONTROL, VK_V,
        VIRTUAL_KEY,
    };

    fn key(vk: VIRTUAL_KEY, released: bool) -> INPUT {
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: vk,
                    wScan: 0,
                    dwFlags: if released { KEYEVENTF_KEYUP } else { 0 },
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        }
    }

    // Ctrl down, V down, V up, Ctrl up — sent as one batch so nothing can be
    // injected between them and leave Ctrl stuck down.
    let inputs = [
        key(VK_CONTROL, false),
        key(VK_V, false),
        key(VK_V, true),
        key(VK_CONTROL, true),
    ];

    // SAFETY: SendInput reads `inputs.len()` records of `size_of::<INPUT>()`
    // bytes from the pointer, which is exactly what the array provides.
    let sent = unsafe {
        SendInput(
            inputs.len() as u32,
            inputs.as_ptr(),
            std::mem::size_of::<INPUT>() as i32,
        )
    };

    if sent as usize != inputs.len() {
        // Blocked rather than broken, usually: UIPI drops injected input aimed
        // at a window running with higher privileges than this process.
        return Err(format!(
            "SendInput delivered {sent} of {} events; the focused window may be elevated",
            inputs.len()
        ));
    }
    Ok(())
}

/// Stub for unsupported platforms.
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn simulate_paste() -> Result<(), String> {
    log::warn!("Paste simulation not implemented for this platform");
    Ok(())
}
