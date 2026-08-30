//! Layered-window backend. It polls the revision rather than being pushed to:
//! the window lives on its own thread, which is the only one allowed to destroy
//! it, and reaching it from the setter would mean a channel for no gain.

use super::{STRIP, TOP_OFFSET, line_cap, render_strip, worth_showing};
use crate::ui::layered::{Overlay, cursor_display};
use std::time::Duration;
use windows_sys::core::w;

/// The strip tears itself down once `current` reports it is gone, so closing
/// needs nothing from the calling thread.
pub(super) fn close(_app: &tauri::AppHandle) {}

/// A change needs nothing pushed: the thread sees the new revision.
pub(super) fn set_text(_app: &tauri::AppHandle) {}

/// The text changes at most once a second; 20 Hz is imperceptibly prompt
/// and leaves the CPU alone.
const POLL_INTERVAL: Duration = Duration::from_millis(50);

pub(super) fn open(_app: &tauri::AppHandle, generation: u32) {
    std::thread::spawn(move || unsafe { run(generation) });
}

/// What to show. `None` once the strip is closed or superseded, which is
/// how the thread learns to tear its window down.
fn current(generation: u32) -> Option<(String, u64)> {
    STRIP.update(generation, |s| (s.text.clone(), s.revision))
}

unsafe fn run(generation: u32) {
    let (scale, screen) = cursor_display();
    let top = screen.top + (TOP_OFFSET as f32 * scale).round() as i32;

    let mut drawn = None;
    let mut shown = false;
    let mut overlay = None;
    loop {
        let Some((text, revision)) = current(generation) else { break };
        // Nothing to read yet: no window, so no empty band under the pill.
        if drawn != Some(revision) && (shown || worth_showing(&text)) {
            drawn = Some(revision);
            let strip = render_strip(&text, scale, line_cap());
            let (width, height) = (strip.width as i32, strip.height as i32);
            let x = screen.left + (screen.right - screen.left - width) / 2;

            if overlay.is_none() {
                let Some(window) =
                    Overlay::new(w!("JonaWhisperSubtitle"), x, top, width, height)
                else {
                    return;
                };
                overlay = Some(window);
            }
            let window = overlay.as_mut().expect("just created");
            // Painted before the first show, so the strip never appears empty.
            window.present(&strip.rgba, width, height, x, top);
            if !shown {
                window.show();
                shown = true;
            }
        }
        if let Some(window) = overlay.as_ref() {
            window.pump();
        }
        std::thread::sleep(POLL_INTERVAL);
    }
}
