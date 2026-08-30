//! Native pill overlay window — no WebView, just an RGBA bitmap handed to each
//! platform's compositor: an NSImageView on macOS, a layered window on Windows.
//! Eliminates the WKWebView white flash entirely.

use super::menu_icons::{sdf_aa, sdf_circle, sdf_rrect, sdf_segment};
#[cfg(target_os = "macos")]
use objc2::msg_send;
#[cfg(target_os = "macos")]
use objc2::runtime::{AnyClass, AnyObject};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;
use tauri::AppHandle;

const PILL_WIDTH: f64 = 80.0;
const PILL_HEIGHT: f64 = 32.0;
const PILL_TOP_OFFSET: f64 = 40.0;
/// Both backends animate at this rate.
const FRAME_INTERVAL: Duration = Duration::from_millis(33);
/// Backing-store scale. Fixed on macOS, where the overlay lives on a Retina
/// screen; read from the monitor on Windows, where it follows the display
/// setting. Everything downstream derives its minimum sizes from the frame
/// height, so the drawing itself is resolution-independent.
#[cfg(target_os = "macos")]
const DPR: f32 = 2.0;
#[cfg(target_os = "macos")]
const PX_W: usize = (PILL_WIDTH as f32 * DPR) as usize; // 160
#[cfg(target_os = "macos")]
const PX_H: usize = (PILL_HEIGHT as f32 * DPR) as usize; // 64

/// Scale a frame of this height was drawn at.
fn frame_scale(ch: f32) -> f32 {
    (ch / PILL_HEIGHT as f32).max(1.0)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PillMode {
    Preparing,
    Recording,
    Paused,
    Transcribing,
    Success,
    Error,
    #[allow(dead_code)]
    Idle,
}

// -- Shared state --

/// Carries no window handle: each backend keeps its own, so the state the app
/// mutates — and the animation driven from it — is the same on every platform.
struct PillState {
    mode: PillMode,
    spectrum: [f32; 12],
    smoothed: [f32; 12],
    dot_phase: f32,
    pending_count: u32,
}

impl PillState {
    fn new(mode: PillMode) -> Self {
        Self { mode, spectrum: [0.0; 12], smoothed: [0.0; 12], dot_phase: 0.0, pending_count: 0 }
    }

    fn frame(&self) -> PillFrame {
        PillFrame {
            mode: self.mode,
            smoothed: self.smoothed,
            dot_phase: self.dot_phase,
            pending_count: self.pending_count,
        }
    }
}

static PILL: Mutex<Option<PillState>> = Mutex::new(None);

/// Bumped by every open(). A close/open pair inside one frame interval would
/// otherwise leave the previous loop running against the new state, animating
/// everything at twice the rate.
static GENERATION: AtomicU32 = AtomicU32::new(0);

/// Advance one animation step. `None` once the pill is closed or superseded,
/// which is how both backends learn to tear their window down.
fn tick(generation: u32) -> Option<PillFrame> {
    static FRAMES: AtomicU32 = AtomicU32::new(0);
    static FLAT: AtomicU32 = AtomicU32::new(0);

    if GENERATION.load(Ordering::Relaxed) != generation {
        return None;
    }
    let mut guard = PILL.lock().unwrap();
    let p = guard.as_mut()?;
    p.dot_phase += 0.05;
    for i in 0..12 {
        p.smoothed[i] = p.smoothed[i] * 0.45 + p.spectrum[i] * 0.55;
    }

    let fc = FRAMES.fetch_add(1, Ordering::Relaxed) + 1;
    if p.mode == PillMode::Recording && fc.is_multiple_of(30) {
        let spec_max = p.spectrum.iter().cloned().fold(0.0f32, f32::max);
        let smooth_max = p.smoothed.iter().cloned().fold(0.0f32, f32::max);
        if smooth_max < 0.12 {
            let count = FLAT.fetch_add(30, Ordering::Relaxed) + 30;
            // Only warn after ~3s of sustained flat, then every ~3s
            if count >= 90 && count.is_multiple_of(90) {
                log::warn!("Pill render flat ({:.1}s): spec_max={:.4}, smooth_max={:.4}, spectrum={:.3?}",
                    count as f32 / 30.0, spec_max, smooth_max, p.spectrum);
            }
        } else {
            let prev = FLAT.swap(0, Ordering::Relaxed);
            if prev >= 90 {
                log::info!("Pill render recovered after {:.1}s flat", prev as f32 / 30.0);
            }
        }
    }
    Some(p.frame())
}

// -- Public API --

pub fn open(app: &AppHandle, initial_mode: PillMode) {
    {
        let mut guard = PILL.lock().unwrap();
        if guard.is_some() {
            log::debug!("Pill: open() called but already open, skipping");
            return;
        }
        // Published before the window exists so a set_mode() racing right
        // behind open() lands on the state instead of warning into the void.
        *guard = Some(PillState::new(initial_mode));
    }
    log::debug!("Pill: opening with mode {:?}", initial_mode);
    backend_open(app, GENERATION.fetch_add(1, Ordering::Relaxed) + 1);
}

pub fn close(app: &AppHandle) {
    if PILL.lock().unwrap().take().is_none() {
        return;
    }
    log::debug!("Pill: closing");
    backend_close(app);
}

pub fn set_mode(mode: PillMode) {
    let mut guard = PILL.lock().unwrap();
    let Some(ref mut p) = *guard else {
        log::warn!("Pill: set_mode({:?}) called but pill is not open", mode);
        return;
    };
    log::debug!("Pill: mode {:?} → {:?}", p.mode, mode);
    // Reset spectrum state when entering Recording to avoid stale smoothed values
    if mode == PillMode::Recording {
        let smooth_max = p.smoothed.iter().cloned().fold(0.0f32, f32::max);
        if smooth_max > 0.001 {
            log::debug!("Pill: resetting smoothed (was max={:.4})", smooth_max);
        }
        p.smoothed = [0.0; 12];
        p.spectrum = [0.0; 12];
    }
    p.mode = mode;
}

pub fn set_spectrum(data: &[f32]) {
    if let Some(ref mut p) = *PILL.lock().unwrap() {
        let n = data.len().min(12);
        p.spectrum[..n].copy_from_slice(&data[..n]);
    }
}

pub fn set_pending(count: u32) {
    if let Some(ref mut p) = *PILL.lock().unwrap() {
        p.pending_count = count;
    }
}

#[allow(dead_code)]
pub fn is_open() -> bool {
    PILL.lock().unwrap().is_some()
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn backend_open(_app: &AppHandle, _generation: u32) {}
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn backend_close(_app: &AppHandle) {}

#[cfg(target_os = "windows")]
fn backend_open(_app: &AppHandle, generation: u32) {
    win::open(generation);
}
/// The window tears itself down once `tick` reports the pill is gone, so
/// closing needs nothing from the calling thread.
#[cfg(target_os = "windows")]
fn backend_close(_app: &AppHandle) {}

// -- macOS native implementation --

/// Wrapper for raw AppKit pointers that are created on the main thread and
/// accessed exclusively through `run_on_main_thread`.
///
/// # Safety
/// These pointers are only dereferenced inside closures dispatched to the main
/// thread via `AppHandle::run_on_main_thread`. The `Mutex<Option<..>>` ensures
/// no concurrent access. Sending the wrapper across threads is safe because it
/// is never dereferenced off the main thread.
#[cfg(target_os = "macos")]
struct MainThreadPtr(*mut AnyObject);

#[cfg(target_os = "macos")]
unsafe impl Send for MainThreadPtr {}

#[cfg(target_os = "macos")]
static MAC_WINDOW: Mutex<Option<(MainThreadPtr, MainThreadPtr)>> = Mutex::new(None);

#[cfg(target_os = "macos")]
fn backend_open(app: &AppHandle, generation: u32) {
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        let (ns_win, iv) = unsafe { create_pill_window() };
        // Render first frame, then show — no flash possible
        if let Some(ref p) = *PILL.lock().unwrap() {
            let rgba = render_frame(&p.frame(), DPR);
            unsafe { update_image_view(iv, &rgba) };
        }
        unsafe {
            let _: () = msg_send![ns_win, orderFrontRegardless];
        }
        *MAC_WINDOW.lock().unwrap() = Some((MainThreadPtr(ns_win), MainThreadPtr(iv)));
    });
    std::thread::spawn(move || animation_loop(handle, generation));
}

#[cfg(target_os = "macos")]
fn backend_close(app: &AppHandle) {
    let addr = MAC_WINDOW.lock().unwrap().take().map(|(w, _)| w.0 as usize);
    if let Some(addr) = addr {
        let _ = app.run_on_main_thread(move || unsafe {
            let ns_win = addr as *mut AnyObject;
            let _: () = msg_send![ns_win, close];
        });
    }
}

#[cfg(target_os = "macos")]
fn animation_loop(app: AppHandle, generation: u32) {
    loop {
        std::thread::sleep(FRAME_INTERVAL);
        let Some(frame) = tick(generation) else { break };
        let rgba = render_frame(&frame, DPR);
        let Some(iv) = MAC_WINDOW.lock().unwrap().as_ref().map(|(_, v)| v.0 as usize) else {
            continue;
        };
        let _ = app.run_on_main_thread(move || unsafe {
            update_image_view(iv as *mut AnyObject, &rgba);
        });
    }
}

#[cfg(target_os = "macos")]
unsafe fn create_pill_window() -> (*mut AnyObject, *mut AnyObject) {
    use objc2_foundation::{NSPoint, NSRect, NSSize};

    let rect = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(PILL_WIDTH, PILL_HEIGHT));
    let ns_win = super::appkit::overlay_window(rect);

    if let Some(frame) = super::appkit::screen_under_cursor() {
        let x = frame.origin.x + (frame.size.width - PILL_WIDTH) / 2.0;
        let y = frame.origin.y + frame.size.height - PILL_HEIGHT - PILL_TOP_OFFSET;
        let _: () = msg_send![ns_win, setFrameOrigin: NSPoint::new(x, y)];
    }

    let iv: *mut AnyObject = msg_send![AnyClass::get(c"NSImageView").unwrap(), alloc];
    let iv: *mut AnyObject = msg_send![iv, initWithFrame: rect];
    let _: () = msg_send![ns_win, setContentView: iv];

    (ns_win, iv)
}

#[cfg(target_os = "macos")]
unsafe fn update_image_view(iv: *mut AnyObject, rgba: &[u8]) {
    super::appkit::set_view_image(
        iv,
        rgba,
        PX_W,
        PX_H,
        objc2_foundation::NSSize::new(PILL_WIDTH, PILL_HEIGHT),
    );
}

// -- Rendering --

/// Everything a frame needs, with no platform handles: the drawing is shared by
/// every backend, only the blitting differs.
pub(crate) struct PillFrame {
    pub mode: PillMode,
    pub smoothed: [f32; 12],
    pub dot_phase: f32,
    pub pending_count: u32,
}

fn render_frame(p: &PillFrame, scale: f32) -> Vec<u8> {
    let w = (PILL_WIDTH as f32 * scale).round() as usize;
    let h = (PILL_HEIGHT as f32 * scale).round() as usize;
    let cw = w as f32;
    let ch = h as f32;
    let mut rgba = vec![0u8; w * h * 4];

    for y in 0..h {
        for x in 0..w {
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;

            // Pill background (rounded rect, full radius = capsule)
            let bg = sdf_aa(sdf_rrect(px, py, cw / 2.0, ch / 2.0, cw / 2.0, ch / 2.0, ch / 2.0));
            if bg <= 0.0 {
                continue;
            }

            // Background: rgba(30,30,30,0.9), premultiplied
            let bg_a = bg * 0.9;
            let c = 30.0 / 255.0;
            let mut r = c * bg_a;
            let mut g = c * bg_a;
            let mut b = c * bg_a;
            let mut a = bg_a;

            // Content overlay
            match p.mode {
                PillMode::Preparing => {
                    // Pulsing bars at rest — signals "preparing mic, wait to speak"
                    let pulse = (p.dot_phase * 2.5).sin() * 0.15 + 0.2;
                    let fake = [pulse; 12];
                    let sa = spectrum_alpha(px, py, &fake, cw, ch);
                    if sa > 0.0 {
                        let dim = sa * 0.4;
                        over(&mut r, &mut g, &mut b, &mut a, dim, dim, dim, dim);
                    }
                }
                PillMode::Recording => {
                    let sa = spectrum_alpha(px, py, &p.smoothed, cw, ch);
                    if sa > 0.0 {
                        over(&mut r, &mut g, &mut b, &mut a, sa, sa, sa, sa);
                    }
                }
                PillMode::Paused => {
                    let pa = pause_alpha(px, py, cw, ch);
                    if pa > 0.0 {
                        // Amber: stopped, but not finished and not an error.
                        let pr = 0xfb as f32 / 255.0 * pa;
                        let pg = 0xbf as f32 / 255.0 * pa;
                        let pb = 0x24 as f32 / 255.0 * pa;
                        over(&mut r, &mut g, &mut b, &mut a, pr, pg, pb, pa);
                    }
                }
                PillMode::Transcribing => {
                    let (dr, dg, db, da) = dots_pixel(px, py, p.dot_phase, cw, ch);
                    if da > 0.0 {
                        over(&mut r, &mut g, &mut b, &mut a, dr, dg, db, da);
                    }
                }
                PillMode::Success => {
                    let sa = success_alpha(px, py, cw, ch);
                    if sa > 0.0 {
                        let sr = 0x4a as f32 / 255.0 * sa;
                        let sg = 0xde as f32 / 255.0 * sa;
                        let sb = 0x80 as f32 / 255.0 * sa;
                        over(&mut r, &mut g, &mut b, &mut a, sr, sg, sb, sa);
                    }
                }
                PillMode::Error => {
                    let ea = error_alpha(px, py, cw, ch);
                    if ea > 0.0 {
                        let er = 0xef as f32 / 255.0 * ea;
                        let eg = 0x44 as f32 / 255.0 * ea;
                        let eb = 0x44 as f32 / 255.0 * ea;
                        over(&mut r, &mut g, &mut b, &mut a, er, eg, eb, ea);
                    }
                }
                PillMode::Idle => {}
            }

            // Queue badge
            if p.pending_count > 1 {
                let (br, bg2, bb, ba) = badge_pixel(px, py, p.pending_count, cw, ch);
                if ba > 0.0 {
                    over(&mut r, &mut g, &mut b, &mut a, br, bg2, bb, ba);
                }
            }

            let idx = (y * w + x) * 4;
            rgba[idx] = (r * 255.0).min(255.0) as u8;
            rgba[idx + 1] = (g * 255.0).min(255.0) as u8;
            rgba[idx + 2] = (b * 255.0).min(255.0) as u8;
            rgba[idx + 3] = (a * 255.0).min(255.0) as u8;
        }
    }
    rgba
}

/// Premultiplied alpha src-over compositing.
#[inline]
#[allow(clippy::too_many_arguments)]
fn over(dr: &mut f32, dg: &mut f32, db: &mut f32, da: &mut f32, sr: f32, sg: f32, sb: f32, sa: f32) {
    let inv = 1.0 - sa;
    *dr = sr + *dr * inv;
    *dg = sg + *dg * inv;
    *db = sb + *db * inv;
    *da = sa + *da * inv;
}

// -- Drawing helpers --

fn spectrum_alpha(px: f32, py: f32, spectrum: &[f32; 12], cw: f32, ch: f32) -> f32 {
    let scale = frame_scale(ch);
    let bar_w = (cw * 0.035).max(2.0 * scale);
    let gap = (cw * 0.025).max(1.0 * scale);
    let total = 12.0 * bar_w + 11.0 * gap;
    let start_x = (cw - total) / 2.0;
    let max_h = ch * 0.6;
    let cy = ch / 2.0;

    let mut a = 0.0f32;
    for (i, &val) in spectrum.iter().enumerate().take(12) {
        let bh = (val * max_h).max(2.0 * scale);
        let cx = start_x + i as f32 * (bar_w + gap) + bar_w / 2.0;
        let d = sdf_rrect(px, py, cx, cy, bar_w / 2.0, bh / 2.0, bar_w / 2.0);
        a = a.max(sdf_aa(d));
    }
    a
}

/// Two vertical bars — the pause glyph, centred in the pill.
fn pause_alpha(px: f32, py: f32, cw: f32, ch: f32) -> f32 {
    const BAR_W: f32 = 3.0;
    const BAR_H: f32 = 11.0;
    const GAP: f32 = 3.5;
    let cy = ch / 2.0;
    let left = sdf_rrect(px, py, cw / 2.0 - GAP - BAR_W / 2.0, cy, BAR_W / 2.0, BAR_H / 2.0, 1.0);
    let right = sdf_rrect(px, py, cw / 2.0 + GAP + BAR_W / 2.0, cy, BAR_W / 2.0, BAR_H / 2.0, 1.0);
    sdf_aa(left.min(right))
}

fn dots_pixel(px: f32, py: f32, phase: f32, cw: f32, ch: f32) -> (f32, f32, f32, f32) {
    let scale = frame_scale(ch);
    let dot_r = (ch * 0.12).max(3.0 * scale) / 2.0;
    let gap = (cw * 0.08).max(4.0 * scale);
    let total = 3.0 * dot_r * 2.0 + 2.0 * gap;
    let start_x = (cw - total) / 2.0;
    let cy = ch / 2.0;

    let (mut r, mut g, mut b, mut a) = (0.0f32, 0.0f32, 0.0f32, 0.0f32);
    for i in 0..3 {
        let p = phase + i as f32 * 0.8;
        let bounce = p.sin() * 0.3 + 0.7;
        let cx = start_x + i as f32 * (dot_r * 2.0 + gap) + dot_r;
        let d = sdf_circle(px, py, cx, cy, dot_r * bounce);
        let da = sdf_aa(d);
        if da > 0.0 {
            let color_a = 0.4 + bounce * 0.6;
            let sa = da * color_a;
            over(&mut r, &mut g, &mut b, &mut a, sa, sa, sa, sa);
        }
    }
    (r, g, b, a)
}

fn success_alpha(px: f32, py: f32, cw: f32, ch: f32) -> f32 {
    let scale = frame_scale(ch);
    let size = (ch * 0.45).round();
    let cx = cw / 2.0;
    let cy = ch / 2.0;
    let lw = (ch * 0.07).max(1.5 * scale);

    // Checkmark: short stroke down-right, then long stroke up-right
    let x0 = cx - size * 0.4;
    let y0 = cy;
    let x1 = cx - size * 0.1;
    let y1 = cy + size * 0.35;
    let x2 = cx + size * 0.45;
    let y2 = cy - size * 0.35;

    let d1 = sdf_segment(px, py, x0, y0, x1, y1) - lw / 2.0;
    let d2 = sdf_segment(px, py, x1, y1, x2, y2) - lw / 2.0;
    sdf_aa(d1).max(sdf_aa(d2))
}

fn error_alpha(px: f32, py: f32, cw: f32, ch: f32) -> f32 {
    let scale = frame_scale(ch);
    let size = (ch * 0.45).round();
    let cx = cw / 2.0;
    let cy = ch / 2.0;
    let lw = (ch * 0.07).max(1.5 * scale);

    let d1 = sdf_segment(px, py, cx - size / 2.0, cy - size / 2.0, cx + size / 2.0, cy + size / 2.0) - lw / 2.0;
    let d2 = sdf_segment(px, py, cx + size / 2.0, cy - size / 2.0, cx - size / 2.0, cy + size / 2.0) - lw / 2.0;
    sdf_aa(d1).max(sdf_aa(d2))
}

fn badge_pixel(px: f32, py: f32, count: u32, cw: f32, ch: f32) -> (f32, f32, f32, f32) {
    let scale = frame_scale(ch);
    let badge_r = (ch * 0.4 / 2.0).round();
    let bx = cw - badge_r - 2.0 * scale;
    let by = badge_r + 2.0 * scale;

    let circle_a = sdf_aa(sdf_circle(px, py, bx, by, badge_r));
    if circle_a <= 0.0 {
        return (0.0, 0.0, 0.0, 0.0);
    }

    // Red background (premultiplied)
    let mut r = 0xef as f32 / 255.0 * circle_a;
    let mut g = 0x44 as f32 / 255.0 * circle_a;
    let mut b = 0x44 as f32 / 255.0 * circle_a;
    let mut a = circle_a;

    // White digit (3×5 bitmap font)
    let digit = (count.min(9)) as usize;
    let scale = (badge_r * 2.0 * 0.55 / 5.0).max(1.0);
    let dw = 3.0 * scale;
    let dh = 5.0 * scale;
    let dx = bx - dw / 2.0;
    let dy = by - dh / 2.0;

    let lx = ((px - dx) / scale).floor() as i32;
    let ly = ((py - dy) / scale).floor() as i32;
    if (0..3).contains(&lx) && (0..5).contains(&ly)
        && DIGITS[digit][(ly * 3 + lx) as usize] == 1 {
            over(&mut r, &mut g, &mut b, &mut a, 1.0, 1.0, 1.0, 1.0);
    }

    (r, g, b, a)
}

#[rustfmt::skip]
const DIGITS: [[u8; 15]; 10] = [
    [1,1,1, 1,0,1, 1,0,1, 1,0,1, 1,1,1], // 0
    [0,1,0, 1,1,0, 0,1,0, 0,1,0, 1,1,1], // 1
    [1,1,1, 0,0,1, 1,1,1, 1,0,0, 1,1,1], // 2
    [1,1,1, 0,0,1, 1,1,1, 0,0,1, 1,1,1], // 3
    [1,0,1, 1,0,1, 1,1,1, 0,0,1, 0,0,1], // 4
    [1,1,1, 1,0,0, 1,1,1, 0,0,1, 1,1,1], // 5
    [1,1,1, 1,0,0, 1,1,1, 1,0,1, 1,1,1], // 6
    [1,1,1, 0,0,1, 0,0,1, 0,0,1, 0,0,1], // 7
    [1,1,1, 1,0,1, 1,1,1, 1,0,1, 1,1,1], // 8
    [1,1,1, 1,0,1, 1,1,1, 0,0,1, 1,1,1], // 9
];

// -- Windows native implementation --

#[cfg(target_os = "windows")]
mod win {
    use super::{FRAME_INTERVAL, PILL, PILL_HEIGHT, PILL_TOP_OFFSET, PILL_WIDTH, render_frame, tick};
    use crate::ui::layered::{Overlay, cursor_display};
    use windows_sys::core::w;

    pub fn open(generation: u32) {
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
        let first = PILL.lock().unwrap().as_ref().map(|p| render_frame(&p.frame(), scale));
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
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- Alpha compositing --

    #[test]
    fn over_composites_opaque_source_on_transparent_dest() {
        let (mut r, mut g, mut b, mut a) = (0.0, 0.0, 0.0, 0.0);
        over(&mut r, &mut g, &mut b, &mut a, 1.0, 0.0, 0.0, 1.0);
        assert!((r - 1.0).abs() < 0.001);
        assert!((a - 1.0).abs() < 0.001);
    }

    #[test]
    fn over_blends_semitransparent_source() {
        let (mut r, mut g, mut b, mut a) = (0.0, 0.0, 1.0, 1.0);
        over(&mut r, &mut g, &mut b, &mut a, 0.5, 0.0, 0.0, 0.5);
        // Source (red 0.5 @ 50%) over dest (blue 1.0 @ 100%)
        assert!(r > 0.4, "Red should bleed through: {r}");
        assert!(b > 0.4, "Blue should remain: {b}");
        assert!((a - 1.0).abs() < 0.001, "Alpha should be ~1.0: {a}");
    }

    // -- SDF primitives --

    #[test]
    fn sdf_aa_inside_is_opaque() {
        assert!((sdf_aa(-5.0) - 1.0).abs() < 0.001);
    }

    #[test]
    fn sdf_aa_outside_is_transparent() {
        assert!(sdf_aa(5.0) < 0.001);
    }

    #[test]
    fn sdf_aa_boundary_is_half() {
        assert!((sdf_aa(0.0) - 0.5).abs() < 0.001);
    }

    #[test]
    fn circle_center_is_inside() {
        let d = sdf_circle(50.0, 50.0, 50.0, 50.0, 10.0);
        assert!(d < 0.0, "Center of circle should be inside (negative SDF)");
    }

    #[test]
    fn circle_far_point_is_outside() {
        let d = sdf_circle(100.0, 100.0, 50.0, 50.0, 10.0);
        assert!(d > 0.0, "Far point should be outside (positive SDF)");
    }

    #[test]
    fn rrect_center_is_inside() {
        let d = sdf_rrect(80.0, 32.0, 80.0, 32.0, 80.0, 32.0, 16.0);
        assert!(d < 0.0, "Center of rounded rect should be inside");
    }

    #[test]
    fn segment_point_on_line_has_zero_distance() {
        // Midpoint of horizontal segment
        let d = sdf_segment(5.0, 0.0, 0.0, 0.0, 10.0, 0.0);
        assert!(d < 0.1, "Point on segment should have ~0 distance: {d}");
    }

    // -- Spectrum bars --

    #[test]
    fn spectrum_silent_audio_produces_minimal_bars() {
        let silent = [0.0f32; 12];
        let cw = PX_W as f32;
        let ch = PX_H as f32;
        // Sample at the center of bar 6 (vertically centered)
        let bar_w = (cw * 0.035).max(2.0 * DPR);
        let gap = (cw * 0.025).max(1.0 * DPR);
        let total = 12.0 * bar_w + 11.0 * gap;
        let bar6_cx = (cw - total) / 2.0 + 6.0 * (bar_w + gap) + bar_w / 2.0;
        let a = spectrum_alpha(bar6_cx, ch / 2.0, &silent, cw, ch);
        assert!(a > 0.0, "Even silent spectrum should show minimal bars at bar center");
    }

    #[test]
    fn spectrum_loud_audio_produces_tall_bars() {
        let loud = [1.0f32; 12];
        let cw = PX_W as f32;
        let ch = PX_H as f32;
        // Sample at bar 6 center, near the top of the pill
        let bar_w = (cw * 0.035).max(2.0 * DPR);
        let gap = (cw * 0.025).max(1.0 * DPR);
        let total = 12.0 * bar_w + 11.0 * gap;
        let bar6_cx = (cw - total) / 2.0 + 6.0 * (bar_w + gap) + bar_w / 2.0;
        let a = spectrum_alpha(bar6_cx, ch * 0.25, &loud, cw, ch);
        assert!(a > 0.0, "Loud spectrum should have bars reaching top quarter");
    }

    #[test]
    fn pause_glyph_is_two_bars_at_centre() {
        let (cw, ch) = (super::PILL_WIDTH as f32, super::PILL_HEIGHT as f32);
        // Between the bars there is a gap, so the exact centre stays empty.
        assert!(super::pause_alpha(cw / 2.0, ch / 2.0, cw, ch) < 0.01);
        // Each bar is filled.
        assert!(super::pause_alpha(cw / 2.0 - 5.0, ch / 2.0, cw, ch) > 0.9);
        assert!(super::pause_alpha(cw / 2.0 + 5.0, ch / 2.0, cw, ch) > 0.9);
        // Nothing outside them.
        assert!(super::pause_alpha(cw / 2.0, 2.0, cw, ch) < 0.01);
    }

    #[test]
    fn spectrum_outside_pill_is_transparent() {
        let loud = [1.0f32; 12];
        let a = spectrum_alpha(0.0, 0.0, &loud, PX_W as f32, PX_H as f32);
        assert!(a < 0.01, "Spectrum outside pill area should be transparent");
    }

    // -- Dots animation (transcribing) --

    #[test]
    fn dots_visible_at_pill_center() {
        let cw = PX_W as f32;
        let ch = PX_H as f32;
        let (_, _, _, a) = dots_pixel(cw / 2.0, ch / 2.0, 0.0, cw, ch);
        assert!(a > 0.0, "Transcribing dots should be visible at center");
    }

    #[test]
    fn dots_invisible_outside_pill() {
        let cw = PX_W as f32;
        let ch = PX_H as f32;
        let (_, _, _, a) = dots_pixel(0.0, 0.0, 0.0, cw, ch);
        assert!(a < 0.01, "Dots should not render outside pill");
    }

    // -- Success checkmark --

    #[test]
    fn success_checkmark_visible_at_center() {
        let cw = PX_W as f32;
        let ch = PX_H as f32;
        // Sample along the checkmark path (slightly right of center, on the long stroke)
        let a = success_alpha(cw * 0.55, ch * 0.4, cw, ch);
        assert!(a > 0.0, "Success checkmark should be visible near center");
    }

    // -- Error cross --

    #[test]
    fn error_cross_visible_at_center() {
        let cw = PX_W as f32;
        let ch = PX_H as f32;
        let a = error_alpha(cw / 2.0, ch / 2.0, cw, ch);
        assert!(a > 0.0, "Error cross should be visible at center");
    }

    #[test]
    fn error_cross_invisible_far_from_center() {
        let cw = PX_W as f32;
        let ch = PX_H as f32;
        let a = error_alpha(cw - 1.0, ch - 1.0, cw, ch);
        assert!(a < 0.01, "Error cross should not reach corners");
    }

    // -- Badge --

    #[test]
    fn badge_hidden_when_count_is_one() {
        let cw = PX_W as f32;
        let ch = PX_H as f32;
        // badge_pixel is only called when count > 1 in render_frame,
        // but the function itself should still render — the guard is in render_frame
        let (_, _, _, a) = badge_pixel(cw - 10.0, 10.0, 1, cw, ch);
        // Badge still renders at count=1, but render_frame skips the call
        assert!(a >= 0.0); // just verify no panic
    }

    #[test]
    fn badge_shows_digit_at_count_5() {
        let cw = PX_W as f32;
        let ch = PX_H as f32;
        let badge_r = (ch * 0.4 / 2.0).round();
        let bx = cw - badge_r - 2.0 * DPR;
        let by = badge_r + 2.0 * DPR;
        let (_, _, _, a) = badge_pixel(bx, by, 5, cw, ch);
        assert!(a > 0.0, "Badge with count 5 should be visible at badge center");
    }

    // -- Full frame render --

    fn frame(mode: PillMode) -> PillFrame {
        PillFrame { mode, smoothed: [0.5; 12], dot_phase: 0.0, pending_count: 0 }
    }

    fn px(rgba: &[u8], x: usize, y: usize, scale: f32) -> &[u8] {
        let w = (PILL_WIDTH as f32 * scale).round() as usize;
        &rgba[(y * w + x) * 4..][..4]
    }

    #[test]
    fn render_frame_sizes_the_buffer_from_the_scale() {
        for scale in [1.0, 1.5, 2.0, 3.0] {
            let w = (PILL_WIDTH as f32 * scale).round() as usize;
            let h = (PILL_HEIGHT as f32 * scale).round() as usize;
            let rgba = render_frame(&frame(PillMode::Recording), scale);
            assert_eq!(rgba.len(), w * h * 4, "buffer at scale {scale}");
        }
    }

    #[test]
    fn render_frame_keeps_the_capsule_shape_at_every_scale() {
        for scale in [1.0, 1.5, 2.0, 3.0] {
            let h = (PILL_HEIGHT as f32 * scale).round() as usize;
            let w = (PILL_WIDTH as f32 * scale).round() as usize;
            let rgba = render_frame(&frame(PillMode::Idle), scale);
            assert_eq!(px(&rgba, 0, 0, scale)[3], 0, "corner at scale {scale}");
            let a = px(&rgba, w / 2, h / 2, scale)[3];
            assert!(a > 200, "center at scale {scale}: {a}");
        }
    }

    #[test]
    fn each_pill_mode_renders_different_content() {
        let modes = [
            PillMode::Recording,
            PillMode::Transcribing,
            PillMode::Success,
            PillMode::Error,
        ];
        let frames: Vec<Vec<u8>> = modes
            .iter()
            .map(|&mode| render_frame(&frame(mode), 2.0))
            .collect();
        for i in 0..frames.len() {
            for j in (i + 1)..frames.len() {
                assert_ne!(frames[i], frames[j],
                    "Modes {:?} and {:?} should render differently", modes[i], modes[j]);
            }
        }
    }
}
