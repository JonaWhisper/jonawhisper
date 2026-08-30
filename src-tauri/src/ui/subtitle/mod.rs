//! Live preview overlay — a subtitle strip under the pill.
//!
//! Draws itself into an RGBA buffer rather than handing text to a native
//! control, so the strip shows the same glyphs and the same wrapping wherever
//! it runs. Display only: it never becomes key window, so the app the user is
//! dictating into keeps focus and the paste path is untouched.

mod render;
use render::render_strip;

use super::overlay::Shared;
use std::sync::atomic::{AtomicU8, Ordering};
use tauri::AppHandle;

const WIDTH: f64 = 560.0;
const LINE_HEIGHT: f64 = 19.0;
const FONT_SIZE: f64 = 15.0;
/// Sits below the pill: pill top offset (40) + pill height (32) + a gap.
const TOP_OFFSET: f64 = 80.0;
const PADDING: f64 = 12.0;
const CORNER_RADIUS: f64 = 12.0;
/// The pill's own background, so the two overlays match.
const GREY: f32 = 30.0 / 255.0;
const BACKDROP_ALPHA: f32 = 0.9;
/// Set from preferences when the strip opens: how tall it may get before it
/// stops growing, whatever the text.
static MAX_LINES: AtomicU8 = AtomicU8::new(5);

fn line_cap() -> u8 {
    MAX_LINES.load(Ordering::Relaxed).max(1)
}

#[cfg(target_os = "macos")]
#[path = "macos.rs"]
mod backend;
#[cfg(target_os = "windows")]
#[path = "windows.rs"]
mod backend;

/// Nothing to draw on with, so the strip simply never appears.
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
mod backend {
    use tauri::AppHandle;
    pub(super) fn open(_app: &AppHandle, _generation: u32) {}
    pub(super) fn set_text(_app: &AppHandle) {}
    pub(super) fn close(_app: &AppHandle) {}
}

// -- Shared state --

/// Carries no window handle: each backend keeps its own, so what the app sets
/// is the same on every platform.
struct StripState {
    text: String,
    /// Bumped on every change, so a backend that polls knows what it has drawn.
    revision: u64,
}

static STRIP: Shared<StripState> = Shared::new();

// -- Public API --

pub fn open(app: &AppHandle, max_lines: u8) {
    MAX_LINES.store(max_lines.clamp(1, 10), Ordering::Relaxed);
    let state = StripState { text: String::new(), revision: 0 };
    let Some(generation) = STRIP.open(state) else { return };
    backend::open(app, generation);
}

/// Replace the displayed text. No-op when the overlay is closed.
pub fn set_text(app: &AppHandle, text: &str) {
    let changed = STRIP.write(|state| {
        if state.text == text {
            return false;
        }
        state.text.clear();
        state.text.push_str(text);
        state.revision += 1;
        true
    });
    if changed == Some(true) {
        backend::set_text(app);
    }
}

pub fn close(app: &AppHandle) {
    if !STRIP.close() {
        return;
    }
    backend::close(app);
}

#[cfg(target_os = "windows")]
fn backend_open(_app: &AppHandle, generation: u32) {
    win::open(generation);
}
/// The Windows backend polls `current`, so a change needs nothing pushed to it,
/// and the window tears itself down once that returns None.
#[cfg(target_os = "windows")]
fn backend_set_text(_app: &AppHandle) {}
#[cfg(target_os = "windows")]
fn backend_close(_app: &AppHandle) {}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn backend_open(_app: &AppHandle, _generation: u32) {}
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn backend_set_text(_app: &AppHandle) {}
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn backend_close(_app: &AppHandle) {}

