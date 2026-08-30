//! Layered-window backend: the compositor blends our premultiplied buffer, so
//! there is no window background to flash through.

use super::{FRAME_INTERVAL, PILL, PILL_HEIGHT, PILL_TOP_OFFSET, PILL_WIDTH, render_frame, tick};
use crate::ui::layered::{Overlay, cursor_display};

/// The window tears itself down once `tick` reports the pill is gone, so
/// closing needs nothing from the calling thread.
pub(super) fn close(_app: &tauri::AppHandle) {}
use windows_sys::core::w;

pub(super) fn open(_app: &tauri::AppHandle, generation: u32) {
    std::thread::spawn(move || unsafe { run(generation) });
}

unsafe fn run(generation: u32) {
    let (scale, screen) = cursor_display();
    let width = (PILL_WIDTH as f32 * scale).round() as i32;
    let height = (PILL_HEIGHT as f32 * scale).round() as i32;
    let x = screen.left + (screen.right - screen.left - width) / 2;
    let y = screen.top + (PILL_TOP_OFFSET as f32 * scale).round() as i32;

    let Some(mut overlay) = Overlay::new(w!("JonaWhisperPill"), x, y, width, height) else {
        return;
    };

    // First frame before the window is shown — no flash possible.
    let first = PILL.read(|p| render_frame(&p.frame(), scale));
    if let Some(rgba) = first {
        overlay.present(&rgba, width, height, x, y);
    }
    overlay.show();

    loop {
        overlay.pump();
        std::thread::sleep(FRAME_INTERVAL);
        let Some(frame) = tick(generation) else { break };
        overlay.present(&render_frame(&frame, scale), width, height, x, y);
    }
}
