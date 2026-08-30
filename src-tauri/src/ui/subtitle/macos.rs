//! AppKit backend: an NSImageView resized as the text wraps, hanging from a
//! fixed top edge under the pill.

use super::super::appkit::{self, DPR, MainThreadPtr};
use super::render::height_for_lines;
use super::{STRIP, TOP_OFFSET, WIDTH, line_cap, render_strip, worth_showing};
use std::sync::atomic::{AtomicBool, Ordering};
use objc2::msg_send;
use objc2::runtime::AnyObject;
use tauri::AppHandle;

struct SubtitleInner {
    ns_window: MainThreadPtr,
    image_view: MainThreadPtr,
}

static WINDOW: std::sync::Mutex<Option<SubtitleInner>> = std::sync::Mutex::new(None);

/// Whether the window has been ordered front yet. Reset on every open.
static REVEALED: AtomicBool = AtomicBool::new(false);

pub(super) fn open(app: &AppHandle, _generation: u32) {
    REVEALED.store(false, Ordering::Relaxed);
    // Built, not shown: set_text orders it front once there is something to read.
    let _ = app.run_on_main_thread(move || unsafe {
        let (ns_win, iv) = create_window();
        *WINDOW.lock().unwrap() = Some(SubtitleInner {
            ns_window: MainThreadPtr(ns_win),
            image_view: MainThreadPtr(iv),
        });
    });
}

pub(super) fn set_text(app: &AppHandle) {
    let Some(text) = STRIP.read(|s| s.text.clone()) else { return };
    if !REVEALED.load(Ordering::Relaxed) && !worth_showing(&text) {
        return;
    }
    let reveal = !REVEALED.swap(true, Ordering::Relaxed);
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
        appkit::set_view_image(
            iv,
            &strip.rgba,
            strip.width,
            strip.height,
            objc2_foundation::NSSize::new(WIDTH, strip.points),
        );
        if reveal {
            let _: () = msg_send![ns_win, orderFrontRegardless];
        }
    });
}

pub(super) fn close(app: &AppHandle) {
    let addr = WINDOW.lock().unwrap().take().map(|w| w.ns_window.0 as usize);
    if let Some(addr) = addr {
        let _ = app.run_on_main_thread(move || unsafe {
            let ns_win = addr as *mut AnyObject;
            let _: () = msg_send![ns_win, orderOut: std::ptr::null::<AnyObject>()];
            let _: () = msg_send![ns_win, close];
        });
    }
}

unsafe fn create_window() -> (*mut AnyObject, *mut AnyObject) {
    use objc2::runtime::AnyClass;
    use objc2_foundation::{NSPoint, NSRect, NSSize};

    let height = height_for_lines(1.0, line_cap());
    let rect = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(WIDTH, height));
    let ns_win = appkit::overlay_window(rect);
    position_under_pill(ns_win, height);

    let iv: *mut AnyObject = msg_send![AnyClass::get(c"NSImageView").unwrap(), alloc];
    let iv: *mut AnyObject = msg_send![iv, initWithFrame: rect];
    let _: () = msg_send![ns_win, setContentView: iv];
    (ns_win, iv)
}

/// Grow downwards as the text wraps, keeping the top edge where it is: the
/// strip hangs under the pill.
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

unsafe fn position_under_pill(ns_win: *mut AnyObject, height: f64) {
    use objc2_foundation::NSPoint;

    let Some(frame) = appkit::screen_under_cursor() else { return };
    let x = frame.origin.x + (frame.size.width - WIDTH) / 2.0;
    let y = frame.origin.y + frame.size.height - height - TOP_OFFSET;
    let _: () = msg_send![ns_win, setFrameOrigin: NSPoint::new(x, y)];
}

