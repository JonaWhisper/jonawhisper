//! Live preview overlay — a subtitle strip under the pill.
//!
//! Draws itself into an RGBA buffer rather than handing text to a native
//! control, so the strip shows the same glyphs and the same wrapping wherever
//! it runs. Display only: it never becomes key window, so the app the user is
//! dictating into keeps focus and the paste path is untouched.

use super::menu_icons::{sdf_aa, sdf_rrect};
use super::text;
use super::overlay::Shared;
use std::sync::atomic::{AtomicU8, Ordering};
use tauri::AppHandle;

#[cfg(target_os = "macos")]
use super::appkit::{DPR, MainThreadPtr};
#[cfg(target_os = "macos")]
use objc2::msg_send;
#[cfg(target_os = "macos")]
use objc2::runtime::AnyObject;

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

/// Height for a given number of lines, padding included.
fn height_for_lines(lines: f64, cap: u8) -> f64 {
    lines.clamp(1.0, cap.max(1) as f64) * LINE_HEIGHT + PADDING * 2.0
}

fn line_cap() -> u8 {
    MAX_LINES.load(Ordering::Relaxed).max(1)
}

struct Strip {
    rgba: Vec<u8>,
    width: usize,
    height: usize,
    /// What the buffer measures once drawn at `scale`. AppKit sizes its window
    /// in points; the Windows overlay works in pixels from end to end.
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    points: f64,
}

/// Compose the rounded backdrop and the text into one premultiplied buffer.
fn render_strip(content: &str, scale: f32, cap: u8) -> Strip {
    let pad = (PADDING as f32 * scale).round();
    let line_px = LINE_HEIGHT as f32 * scale;
    let width = (WIDTH as f32 * scale).round() as usize;
    let text_width = (width as f32 - pad * 2.0).max(1.0) as usize;

    let img = text::render(content, FONT_SIZE as f32 * scale, text_width, line_px);
    let visible = img.lines.clamp(1, cap.max(1) as usize);
    // Show the tail, not the head: on a live transcript the newest words are
    // the ones the user is checking, and they are at the bottom.
    let skipped = img.lines.saturating_sub(visible) as f32 * line_px;

    let points = height_for_lines(visible as f64, cap);
    let height = (points as f32 * scale).round() as usize;
    let mut rgba = vec![0u8; width * height * 4];
    let (cw, ch) = (width as f32, height as f32);
    let radius = CORNER_RADIUS as f32 * scale;

    for y in 0..height {
        for x in 0..width {
            let backdrop = sdf_aa(sdf_rrect(
                x as f32 + 0.5,
                y as f32 + 0.5,
                cw / 2.0,
                ch / 2.0,
                cw / 2.0,
                ch / 2.0,
                radius,
            ));
            if backdrop <= 0.0 {
                continue;
            }
            let back_a = backdrop * BACKDROP_ALPHA;

            let tx = x as f32 - pad;
            let ty = y as f32 - pad + skipped;
            let ink = if tx >= 0.0 && ty >= 0.0 {
                let (tx, ty) = (tx as usize, ty as usize);
                if tx < img.width && ty < img.height {
                    img.alpha[ty * img.width + tx] * backdrop
                } else {
                    0.0
                }
            } else {
                0.0
            };

            // White text over the backdrop, both premultiplied.
            let a = back_a * (1.0 - ink) + ink;
            let c = GREY * back_a * (1.0 - ink) + ink;
            let i = (y * width + x) * 4;
            rgba[i] = (c * 255.0).round() as u8;
            rgba[i + 1] = (c * 255.0).round() as u8;
            rgba[i + 2] = (c * 255.0).round() as u8;
            rgba[i + 3] = (a * 255.0).round() as u8;
        }
    }

    Strip { rgba, width, height, points }
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
    backend_open(app, generation);
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
        backend_set_text(app);
    }
}

pub fn close(app: &AppHandle) {
    if !STRIP.close() {
        return;
    }
    backend_close(app);
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

// -- macOS native implementation --

#[cfg(target_os = "macos")]
struct SubtitleInner {
    ns_window: MainThreadPtr,
    image_view: MainThreadPtr,
}

#[cfg(target_os = "macos")]
static WINDOW: std::sync::Mutex<Option<SubtitleInner>> = std::sync::Mutex::new(None);

#[cfg(target_os = "macos")]
fn backend_open(app: &AppHandle, _generation: u32) {
    let _ = app.run_on_main_thread(move || unsafe {
        let (ns_win, iv) = create_window();
        let _: () = msg_send![ns_win, orderFrontRegardless];
        *WINDOW.lock().unwrap() = Some(SubtitleInner {
            ns_window: MainThreadPtr(ns_win),
            image_view: MainThreadPtr(iv),
        });
    });
}

#[cfg(target_os = "macos")]
fn backend_set_text(app: &AppHandle) {
    let Some(text) = STRIP.read(|s| s.text.clone()) else { return };
    let handles = {
        let guard = WINDOW.lock().unwrap();
        match guard.as_ref() {
            Some(w) => (w.ns_window.0 as usize, w.image_view.0 as usize),
            None => return,
        }
    };
    let strip = render_strip(&text, DPR, line_cap());
    let _ = app.run_on_main_thread(move || unsafe {
        let (ns_win, iv) = (handles.0 as *mut AnyObject, handles.1 as *mut AnyObject);
        resize_to(ns_win, iv, strip.points);
        super::appkit::set_view_image(
            iv,
            &strip.rgba,
            strip.width,
            strip.height,
            objc2_foundation::NSSize::new(WIDTH, strip.points),
        );
    });
}

#[cfg(target_os = "macos")]
fn backend_close(app: &AppHandle) {
    let addr = WINDOW.lock().unwrap().take().map(|w| w.ns_window.0 as usize);
    if let Some(addr) = addr {
        let _ = app.run_on_main_thread(move || unsafe {
            let ns_win = addr as *mut AnyObject;
            let _: () = msg_send![ns_win, orderOut: std::ptr::null::<AnyObject>()];
            let _: () = msg_send![ns_win, close];
        });
    }
}

#[cfg(target_os = "macos")]
unsafe fn create_window() -> (*mut AnyObject, *mut AnyObject) {
    use objc2::runtime::AnyClass;
    use objc2_foundation::{NSPoint, NSRect, NSSize};

    let height = height_for_lines(1.0, line_cap());
    let rect = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(WIDTH, height));
    let ns_win = super::appkit::overlay_window(rect);
    position_under_pill(ns_win, height);

    let iv: *mut AnyObject = msg_send![AnyClass::get(c"NSImageView").unwrap(), alloc];
    let iv: *mut AnyObject = msg_send![iv, initWithFrame: rect];
    let _: () = msg_send![ns_win, setContentView: iv];
    (ns_win, iv)
}

/// Grow downwards as the text wraps, keeping the top edge where it is: the
/// strip hangs under the pill.
#[cfg(target_os = "macos")]
unsafe fn resize_to(ns_win: *mut AnyObject, iv: *mut AnyObject, height: f64) {
    use objc2_foundation::{NSPoint, NSRect, NSSize};

    let frame: NSRect = msg_send![ns_win, frame];
    if (frame.size.height - height).abs() < 0.5 {
        return;
    }
    let top = frame.origin.y + frame.size.height;
    let new_frame = NSRect::new(
        NSPoint::new(frame.origin.x, top - height),
        NSSize::new(WIDTH, height),
    );
    let _: () = msg_send![ns_win, setFrame: new_frame, display: true];
    let content = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(WIDTH, height));
    let _: () = msg_send![iv, setFrame: content];
}

#[cfg(target_os = "macos")]
unsafe fn position_under_pill(ns_win: *mut AnyObject, height: f64) {
    use objc2_foundation::NSPoint;

    let Some(frame) = super::appkit::screen_under_cursor() else { return };
    let x = frame.origin.x + (frame.size.width - WIDTH) / 2.0;
    let y = frame.origin.y + frame.size.height - height - TOP_OFFSET;
    let _: () = msg_send![ns_win, setFrameOrigin: NSPoint::new(x, y)];
}

// -- Windows native implementation --

#[cfg(target_os = "windows")]
mod win {
    use super::{STRIP, TOP_OFFSET, line_cap, render_strip};
    use crate::ui::layered::{Overlay, cursor_display};
    use std::time::Duration;
    use windows_sys::core::w;

    /// The text changes at most once a second; 20 Hz is imperceptibly prompt
    /// and leaves the CPU alone.
    const POLL_INTERVAL: Duration = Duration::from_millis(50);

    pub fn open(generation: u32) {
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
            if drawn != Some(revision) {
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn height_honours_the_configured_cap() {
        let one = height_for_lines(1.0, 5);
        assert_eq!(height_for_lines(0.0, 5), one, "jamais moins d'une ligne");
        assert_eq!(height_for_lines(99.0, 5), height_for_lines(5.0, 5), "plafonne au reglage");
        assert_eq!(height_for_lines(99.0, 2), height_for_lines(2.0, 2));
        assert!(height_for_lines(99.0, 2) < height_for_lines(5.0, 5));
        assert_eq!(height_for_lines(3.0, 0), one, "0 ne fait pas disparaitre la bande");
    }

    #[test]
    fn sits_below_the_pill() {
        // The pill occupies 40..72 from the top; the strip must clear it.
        const { assert!(TOP_OFFSET >= 40.0 + 32.0) };
    }

    #[test]
    fn strip_is_transparent_at_the_corners_and_opaque_inside() {
        let strip = render_strip("Bonjour", 2.0, 5);
        assert_eq!(strip.rgba.len(), strip.width * strip.height * 4);
        assert_eq!(strip.rgba[3], 0, "coin arrondi transparent");
        let centre = (strip.height / 2 * strip.width + strip.width / 2) * 4;
        assert!(strip.rgba[centre + 3] > 200, "fond opaque au centre");
    }

    #[test]
    fn strip_grows_with_the_text_then_stops_at_the_cap() {
        let short = render_strip("Bonjour", 2.0, 2);
        let long = render_strip(&"mot ".repeat(200), 2.0, 2);
        assert!(long.points > short.points, "la bande grandit");
        assert_eq!(long.points, height_for_lines(2.0, 2), "puis plafonne");
    }

    #[test]
    fn text_is_lighter_than_the_backdrop() {
        let strip = render_strip("IIIIIIIIIIIIIIII", 2.0, 5);
        let brightest = strip.rgba.as_chunks::<4>().0.iter().map(|p| p[0]).max().unwrap();
        assert!(brightest > 100, "de l'encre blanche est visible: {brightest}");
    }
}

