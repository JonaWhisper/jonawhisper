//! AppKit backend: an NSImageView fed a fresh bitmap 30 times a second.

use super::super::appkit::{self, DPR, MainThreadPtr};
use super::{FRAME_INTERVAL, PILL, PILL_HEIGHT, PILL_TOP_OFFSET, PILL_WIDTH, render_frame, tick};
use objc2::msg_send;
use objc2::runtime::{AnyClass, AnyObject};
use tauri::AppHandle;

const PX_W: usize = (PILL_WIDTH as f32 * DPR) as usize; // 160
const PX_H: usize = (PILL_HEIGHT as f32 * DPR) as usize; // 64

static MAC_WINDOW: std::sync::Mutex<Option<(MainThreadPtr, MainThreadPtr)>> =
    std::sync::Mutex::new(None);

pub(super) fn open(app: &AppHandle, generation: u32) {
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        let (ns_win, iv) = unsafe { create_pill_window() };
        // Render first frame, then show — no flash possible
        if let Some(rgba) = PILL.read(|p| render_frame(&p.frame(), DPR)) {
            unsafe { update_image_view(iv, &rgba) };
        }
        unsafe {
            let _: () = msg_send![ns_win, orderFrontRegardless];
        }
        *MAC_WINDOW.lock().unwrap() = Some((MainThreadPtr(ns_win), MainThreadPtr(iv)));
    });
    std::thread::spawn(move || animation_loop(handle, generation));
}

pub(super) fn close(app: &AppHandle) {
    let addr = MAC_WINDOW.lock().unwrap().take().map(|(w, _)| w.0 as usize);
    if let Some(addr) = addr {
        let _ = app.run_on_main_thread(move || unsafe {
            let ns_win = addr as *mut AnyObject;
            let _: () = msg_send![ns_win, close];
        });
    }
}

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

unsafe fn create_pill_window() -> (*mut AnyObject, *mut AnyObject) {
    use objc2_foundation::{NSPoint, NSRect, NSSize};

    let rect = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(PILL_WIDTH, PILL_HEIGHT));
    let ns_win = appkit::overlay_window(rect);

    if let Some(frame) = appkit::screen_under_cursor() {
        let x = frame.origin.x + (frame.size.width - PILL_WIDTH) / 2.0;
        let y = frame.origin.y + frame.size.height - PILL_HEIGHT - PILL_TOP_OFFSET;
        let _: () = msg_send![ns_win, setFrameOrigin: NSPoint::new(x, y)];
    }

    let iv: *mut AnyObject = msg_send![AnyClass::get(c"NSImageView").unwrap(), alloc];
    let iv: *mut AnyObject = msg_send![iv, initWithFrame: rect];
    let _: () = msg_send![ns_win, setContentView: iv];

    (ns_win, iv)
}

unsafe fn update_image_view(iv: *mut AnyObject, rgba: &[u8]) {
    appkit::set_view_image(
        iv,
        rgba,
        PX_W,
        PX_H,
        objc2_foundation::NSSize::new(PILL_WIDTH, PILL_HEIGHT),
    );
}

