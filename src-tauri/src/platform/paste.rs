/// Clipboard write + Cmd+V simulation for pasting transcribed text.
/// macOS-specific implementation using CGEvent.

#[cfg(target_os = "macos")]
pub fn paste_text(text: &str) {
    {
        use core_graphics::event::{CGEvent, CGEventFlags};
        use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};

        // Use pbcopy for reliable clipboard write
        let mut child = std::process::Command::new("pbcopy")
            .stdin(std::process::Stdio::piped())
            .spawn()
            .expect("Failed to spawn pbcopy");
        if let Some(mut stdin) = child.stdin.take() {
            use std::io::Write;
            let _ = stdin.write_all(text.as_bytes());
        }
        let _ = child.wait();

        // Small delay to ensure clipboard is ready
        std::thread::sleep(std::time::Duration::from_millis(50));

        // Simulate Cmd+V
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
}

#[cfg(not(target_os = "macos"))]
pub fn paste_text(text: &str) {
    // TODO: Windows/Linux implementation
    log::warn!("Paste simulation not implemented for this platform");
}
