//! Native pill overlay window — no WebView, pure AppKit + RGBA bitmap.
//! Eliminates the WKWebView white flash entirely.

use crate::menu_icons::{sdf_aa, sdf_circle, sdf_rrect, sdf_segment};
#[cfg(target_os = "macos")]
use objc2::msg_send;
#[cfg(target_os = "macos")]
use objc2::runtime::{AnyClass, AnyObject};
use std::sync::Mutex;
use std::time::Duration;
use tauri::AppHandle;

const PILL_WIDTH: f64 = 80.0;
const PILL_HEIGHT: f64 = 32.0;
const PILL_TOP_OFFSET: f64 = 40.0;
const DPR: f32 = 2.0; // Retina
const PX_W: usize = (PILL_WIDTH as f32 * DPR) as usize; // 160
const PX_H: usize = (PILL_HEIGHT as f32 * DPR) as usize; // 64

#[derive(Clone, Copy, PartialEq)]
pub enum PillMode {
    Preparing,
    Recording,
    Transcribing,
    Success,
    Error,
    #[allow(dead_code)]
    Idle,
}

#[cfg(target_os = "macos")]
struct PillInner {
    ns_window: *mut AnyObject,
    image_view: *mut AnyObject,
    mode: PillMode,
    spectrum: [f32; 12],
    smoothed: [f32; 12],
    dot_phase: f32,
    pending_count: u32,
}

#[cfg(target_os = "macos")]
unsafe impl Send for PillInner {}

#[cfg(target_os = "macos")]
static PILL: Mutex<Option<PillInner>> = Mutex::new(None);

// -- Public API --

pub fn open(app: &AppHandle, initial_mode: PillMode) {
    #[cfg(target_os = "macos")]
    {
        if PILL.lock().unwrap().is_some() {
            return;
        }
        let handle = app.clone();
        let _ = app.run_on_main_thread(move || {
            let (ns_win, image_view) = unsafe { create_pill_window() };
            let mut inner = PillInner {
                ns_window: ns_win,
                image_view,
                mode: initial_mode,
                spectrum: [0.0; 12],
                smoothed: [0.0; 12],
                dot_phase: 0.0,
                pending_count: 0,
            };
            // Render first frame, then show — no flash possible
            let rgba = render_frame(&inner);
            unsafe { update_image_view(image_view, &rgba) };
            unsafe {
                let _: () = msg_send![ns_win, orderFrontRegardless];
            }
            // Store state (animation thread will take over)
            inner.dot_phase = 0.0;
            *PILL.lock().unwrap() = Some(inner);
        });
        // Start animation thread
        let anim_handle = handle.clone();
        std::thread::spawn(move || animation_loop(anim_handle));
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = app;
        let _ = initial_mode;
    }
}

pub fn close(app: &AppHandle) {
    #[cfg(target_os = "macos")]
    {
        let ns_win_addr = {
            let mut pill = PILL.lock().unwrap();
            pill.take().map(|p| p.ns_window as usize)
        };
        if let Some(addr) = ns_win_addr {
            let _ = app.run_on_main_thread(move || unsafe {
                let ns_win = addr as *mut AnyObject;
                let _: () = msg_send![ns_win, close];
            });
        }
    }
    #[cfg(not(target_os = "macos"))]
    let _ = app;
}

pub fn set_mode(mode: PillMode) {
    #[cfg(target_os = "macos")]
    if let Some(ref mut p) = *PILL.lock().unwrap() {
        p.mode = mode;
    }
    #[cfg(not(target_os = "macos"))]
    let _ = mode;
}

pub fn set_spectrum(data: &[f32]) {
    #[cfg(target_os = "macos")]
    if let Some(ref mut p) = *PILL.lock().unwrap() {
        let n = data.len().min(12);
        p.spectrum[..n].copy_from_slice(&data[..n]);
    }
    #[cfg(not(target_os = "macos"))]
    let _ = data;
}

pub fn set_pending(count: u32) {
    #[cfg(target_os = "macos")]
    if let Some(ref mut p) = *PILL.lock().unwrap() {
        p.pending_count = count;
    }
    #[cfg(not(target_os = "macos"))]
    let _ = count;
}

#[allow(dead_code)]
pub fn is_open() -> bool {
    #[cfg(target_os = "macos")]
    {
        PILL.lock().unwrap().is_some()
    }
    #[cfg(not(target_os = "macos"))]
    false
}

// -- macOS native implementation --

#[cfg(target_os = "macos")]
fn animation_loop(app: AppHandle) {
    loop {
        std::thread::sleep(Duration::from_millis(33));
        if PILL.lock().unwrap().is_none() {
            break;
        }
        let h = app.clone();
        let _ = app.run_on_main_thread(move || {
            let mut pill = PILL.lock().unwrap();
            let Some(ref mut p) = *pill else { return };
            // Advance animation state
            p.dot_phase += 0.05;
            for i in 0..12 {
                p.smoothed[i] = p.smoothed[i] * 0.45 + p.spectrum[i] * 0.55;
            }
            let rgba = render_frame(p);
            let iv = p.image_view;
            drop(pill); // unlock before AppKit call
            unsafe { update_image_view(iv, &rgba) };
            let _ = h; // keep handle alive
        });
    }
}

#[cfg(target_os = "macos")]
unsafe fn create_pill_window() -> (*mut AnyObject, *mut AnyObject) {
    use objc2_foundation::{NSPoint, NSRect, NSSize};

    let rect = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(PILL_WIDTH, PILL_HEIGHT));
    let cls = AnyClass::get(c"NSWindow").unwrap();
    let ns_win: *mut AnyObject = msg_send![cls, alloc];
    let ns_win: *mut AnyObject = msg_send![ns_win,
        initWithContentRect: rect,
        styleMask: 0u64,
        backing: 2u64,
        defer: false
    ];

    let clear: *mut AnyObject =
        msg_send![AnyClass::get(c"NSColor").unwrap(), clearColor];
    let _: () = msg_send![ns_win, setOpaque: false];
    let _: () = msg_send![ns_win, setBackgroundColor: clear];
    let _: () = msg_send![ns_win, setHasShadow: true];
    let _: () = msg_send![ns_win, setIgnoresMouseEvents: true];
    let _: () = msg_send![ns_win, setLevel: 3i64]; // NSFloatingWindowLevel
    let _: () = msg_send![ns_win, setCollectionBehavior: 17u64]; // canJoinAllSpaces|stationary

    // Position top-center
    let screen: *mut AnyObject = msg_send![ns_win, screen];
    if !screen.is_null() {
        let frame: NSRect = msg_send![screen, frame];
        let x = (frame.size.width - PILL_WIDTH) / 2.0;
        let y = frame.origin.y + frame.size.height - PILL_HEIGHT - PILL_TOP_OFFSET;
        let _: () = msg_send![ns_win, setFrameOrigin: NSPoint::new(x, y)];
    }

    // NSImageView as content
    let iv: *mut AnyObject = msg_send![AnyClass::get(c"NSImageView").unwrap(), alloc];
    let iv: *mut AnyObject = msg_send![iv, initWithFrame: rect];
    let _: () = msg_send![ns_win, setContentView: iv];

    (ns_win, iv)
}

#[cfg(target_os = "macos")]
unsafe fn update_image_view(iv: *mut AnyObject, rgba: &[u8]) {
    let null_planes: *const *mut u8 = std::ptr::null();
    let cs = objc2_foundation::NSString::from_str("NSDeviceRGBColorSpace");

    let rep: *mut AnyObject = msg_send![AnyClass::get(c"NSBitmapImageRep").unwrap(), alloc];
    let rep: *mut AnyObject = msg_send![rep,
        initWithBitmapDataPlanes: null_planes,
        pixelsWide: PX_W as i64,
        pixelsHigh: PX_H as i64,
        bitsPerSample: 8i64,
        samplesPerPixel: 4i64,
        hasAlpha: true,
        isPlanar: false,
        colorSpaceName: &*cs,
        bytesPerRow: (PX_W * 4) as i64,
        bitsPerPixel: 32i64
    ];

    let bitmap_data: *mut u8 = msg_send![rep, bitmapData];
    std::ptr::copy_nonoverlapping(rgba.as_ptr(), bitmap_data, rgba.len());

    let size = objc2_foundation::NSSize::new(PILL_WIDTH, PILL_HEIGHT);
    let img: *mut AnyObject = msg_send![AnyClass::get(c"NSImage").unwrap(), alloc];
    let img: *mut AnyObject = msg_send![img, initWithSize: size];
    let _: () = msg_send![img, addRepresentation: rep];
    let _: () = msg_send![iv, setImage: img];
    let _: () = msg_send![img, release];
    let _: () = msg_send![rep, release];
}

// -- Rendering --

#[cfg(target_os = "macos")]
fn render_frame(p: &PillInner) -> Vec<u8> {
    let w = PX_W;
    let h = PX_H;
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
fn over(dr: &mut f32, dg: &mut f32, db: &mut f32, da: &mut f32, sr: f32, sg: f32, sb: f32, sa: f32) {
    let inv = 1.0 - sa;
    *dr = sr + *dr * inv;
    *dg = sg + *dg * inv;
    *db = sb + *db * inv;
    *da = sa + *da * inv;
}

// -- Drawing helpers --

fn spectrum_alpha(px: f32, py: f32, spectrum: &[f32; 12], cw: f32, ch: f32) -> f32 {
    let bar_w = (cw * 0.035).max(2.0 * DPR);
    let gap = (cw * 0.025).max(1.0 * DPR);
    let total = 12.0 * bar_w + 11.0 * gap;
    let start_x = (cw - total) / 2.0;
    let max_h = ch * 0.6;
    let cy = ch / 2.0;

    let mut a = 0.0f32;
    for i in 0..12 {
        let bh = (spectrum[i] * max_h).max(2.0 * DPR);
        let cx = start_x + i as f32 * (bar_w + gap) + bar_w / 2.0;
        let d = sdf_rrect(px, py, cx, cy, bar_w / 2.0, bh / 2.0, bar_w / 2.0);
        a = a.max(sdf_aa(d));
    }
    a
}

fn dots_pixel(px: f32, py: f32, phase: f32, cw: f32, ch: f32) -> (f32, f32, f32, f32) {
    let dot_r = (ch * 0.12).max(3.0 * DPR) / 2.0;
    let gap = (cw * 0.08).max(4.0 * DPR);
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
    let size = (ch * 0.45).round();
    let cx = cw / 2.0;
    let cy = ch / 2.0;
    let lw = (ch * 0.07).max(1.5 * DPR);

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
    let size = (ch * 0.45).round();
    let cx = cw / 2.0;
    let cy = ch / 2.0;
    let lw = (ch * 0.07).max(1.5 * DPR);

    let d1 = sdf_segment(px, py, cx - size / 2.0, cy - size / 2.0, cx + size / 2.0, cy + size / 2.0) - lw / 2.0;
    let d2 = sdf_segment(px, py, cx + size / 2.0, cy - size / 2.0, cx - size / 2.0, cy + size / 2.0) - lw / 2.0;
    sdf_aa(d1).max(sdf_aa(d2))
}

fn badge_pixel(px: f32, py: f32, count: u32, cw: f32, ch: f32) -> (f32, f32, f32, f32) {
    let badge_r = (ch * 0.4 / 2.0).round();
    let bx = cw - badge_r - 2.0 * DPR;
    let by = badge_r + 2.0 * DPR;

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
    if lx >= 0 && lx < 3 && ly >= 0 && ly < 5 {
        if DIGITS[digit][(ly * 3 + lx) as usize] == 1 {
            over(&mut r, &mut g, &mut b, &mut a, 1.0, 1.0, 1.0, 1.0);
        }
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
