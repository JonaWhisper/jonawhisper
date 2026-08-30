//! Live preview overlay — a subtitle strip under the pill, pure AppKit.
//!
//! Display only: it never becomes key window, so the app the user is dictating
//! into keeps focus and the paste path is untouched.

#[cfg(target_os = "macos")]
use objc2::msg_send;
#[cfg(target_os = "macos")]
use objc2::runtime::{AnyClass, AnyObject};
use std::sync::Mutex;
use tauri::AppHandle;

#[cfg(target_os = "macos")]
const WIDTH: f64 = 560.0;
/// One line of 15pt system font, as laid out by the text cell.
#[cfg(target_os = "macos")]
const LINE_HEIGHT: f64 = 19.0;
#[cfg(target_os = "macos")]
const MAX_LINES: f64 = 5.0;
/// Height for a given number of lines, padding included.
#[cfg(target_os = "macos")]
fn height_for_lines(lines: f64) -> f64 {
    lines.clamp(1.0, MAX_LINES) * LINE_HEIGHT + PADDING * 2.0
}
/// Sits below the pill: pill top offset (40) + pill height (32) + a gap.
#[cfg(target_os = "macos")]
const TOP_OFFSET: f64 = 80.0;
#[cfg(target_os = "macos")]
const PADDING: f64 = 12.0;

#[cfg(target_os = "macos")]
struct MainThreadPtr(*mut AnyObject);

#[cfg(target_os = "macos")]
unsafe impl Send for MainThreadPtr {}

#[cfg(target_os = "macos")]
struct SubtitleInner {
    ns_window: MainThreadPtr,
    text_field: MainThreadPtr,
}

#[cfg(target_os = "macos")]
static SUBTITLE: Mutex<Option<SubtitleInner>> = Mutex::new(None);

// -- Public API --

pub fn open(app: &AppHandle) {
    #[cfg(target_os = "macos")]
    {
        if SUBTITLE.lock().unwrap().is_some() {
            return;
        }
        let _ = app.run_on_main_thread(move || {
            let (ns_win, field) = unsafe { create_window() };
            unsafe {
                let _: () = msg_send![ns_win, orderFrontRegardless];
            }
            *SUBTITLE.lock().unwrap() = Some(SubtitleInner {
                ns_window: MainThreadPtr(ns_win),
                text_field: MainThreadPtr(field),
            });
        });
    }
    #[cfg(not(target_os = "macos"))]
    let _ = app;
}

/// Replace the displayed text. No-op when the overlay is closed.
pub fn set_text(app: &AppHandle, text: &str) {
    #[cfg(target_os = "macos")]
    {
        let field_addr = {
            let guard = SUBTITLE.lock().unwrap();
            match guard.as_ref() {
                Some(s) => s.text_field.0 as usize,
                None => return,
            }
        };
        let win_addr = {
            let guard = SUBTITLE.lock().unwrap();
            match guard.as_ref() {
                Some(s) => s.ns_window.0 as usize,
                None => return,
            }
        };
        let owned = text.to_string();
        let _ = app.run_on_main_thread(move || {
            let field = field_addr as *mut AnyObject;
            let ns_win = win_addr as *mut AnyObject;
            let ns = objc2_foundation::NSString::from_str(&owned);
            unsafe {
                let _: () = msg_send![field, setStringValue: &*ns];
                resize_to_fit(ns_win, field);
            }
        });
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (app, text);
    }
}

pub fn close(app: &AppHandle) {
    #[cfg(target_os = "macos")]
    {
        let addr = {
            let mut guard = SUBTITLE.lock().unwrap();
            guard.take().map(|s| s.ns_window.0 as usize)
        };
        if let Some(addr) = addr {
            let _ = app.run_on_main_thread(move || {
                let ns_win = addr as *mut AnyObject;
                unsafe {
                    let _: () = msg_send![ns_win, orderOut: std::ptr::null::<AnyObject>()];
                    let _: () = msg_send![ns_win, close];
                }
            });
        }
    }
    #[cfg(not(target_os = "macos"))]
    let _ = app;
}

// -- Window construction --

#[cfg(target_os = "macos")]
unsafe fn create_window() -> (*mut AnyObject, *mut AnyObject) {
    use objc2_foundation::{NSPoint, NSRect, NSSize};

    let rect = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(WIDTH, height_for_lines(1.0)));
    let cls = AnyClass::get(c"NSWindow").unwrap();
    let ns_win: *mut AnyObject = msg_send![cls, alloc];
    let ns_win: *mut AnyObject = msg_send![ns_win,
        initWithContentRect: rect,
        styleMask: 0u64,
        backing: 2u64,
        defer: false
    ];

    let clear: *mut AnyObject = msg_send![AnyClass::get(c"NSColor").unwrap(), clearColor];
    let _: () = msg_send![ns_win, setOpaque: false];
    let _: () = msg_send![ns_win, setBackgroundColor: clear];
    let _: () = msg_send![ns_win, setHasShadow: true];
    let _: () = msg_send![ns_win, setIgnoresMouseEvents: true];
    let _: () = msg_send![ns_win, setLevel: 3i64]; // NSFloatingWindowLevel
    let _: () = msg_send![ns_win, setCollectionBehavior: 17u64]; // canJoinAllSpaces|stationary

    position_under_pill(ns_win, height_for_lines(1.0));

    // Rounded translucent backdrop, drawn by the layer rather than a bitmap.
    let backdrop: *mut AnyObject = msg_send![AnyClass::get(c"NSView").unwrap(), alloc];
    let backdrop: *mut AnyObject = msg_send![backdrop, initWithFrame: rect];
    let _: () = msg_send![backdrop, setWantsLayer: true];
    let layer: *mut AnyObject = msg_send![backdrop, layer];
    let bg: *mut AnyObject = msg_send![
        AnyClass::get(c"NSColor").unwrap(),
        colorWithCalibratedRed: 0.0f64, green: 0.0f64, blue: 0.0f64, alpha: 0.78f64
    ];
    let cg: *mut AnyObject = msg_send![bg, CGColor];
    let _: () = msg_send![layer, setBackgroundColor: cg];
    let _: () = msg_send![layer, setCornerRadius: 12.0f64];

    let text_rect = NSRect::new(
        NSPoint::new(PADDING, PADDING),
        NSSize::new(WIDTH - PADDING * 2.0, height_for_lines(1.0) - PADDING * 2.0),
    );
    let field: *mut AnyObject = msg_send![AnyClass::get(c"NSTextField").unwrap(), alloc];
    let field: *mut AnyObject = msg_send![field, initWithFrame: text_rect];
    let _: () = msg_send![field, setEditable: false];
    let _: () = msg_send![field, setSelectable: false];
    let _: () = msg_send![field, setBezeled: false];
    let _: () = msg_send![field, setDrawsBackground: false];
    let white: *mut AnyObject = msg_send![AnyClass::get(c"NSColor").unwrap(), whiteColor];
    let _: () = msg_send![field, setTextColor: white];
    let font: *mut AnyObject = msg_send![
        AnyClass::get(c"NSFont").unwrap(),
        systemFontOfSize: 15.0f64
    ];
    let _: () = msg_send![field, setFont: font];
    // 4 = NSTextAlignmentNatural: left for French and English, right for RTL
    // scripts. Centring made every new word shift the ones already displayed.
    let _: () = msg_send![field, setAlignment: 4i64];
    // 0 = NSLineBreakByWordWrapping
    let cell: *mut AnyObject = msg_send![field, cell];
    let _: () = msg_send![cell, setLineBreakMode: 0i64];
    let _: () = msg_send![cell, setWraps: true];
    let empty = objc2_foundation::NSString::from_str("");
    let _: () = msg_send![field, setStringValue: &*empty];

    let _: () = msg_send![backdrop, addSubview: field];
    let _: () = msg_send![ns_win, setContentView: backdrop];

    (ns_win, field)
}

/// Grow downwards from one line to MAX_LINES as the text wraps. The cell already
/// knows the font and wrapping mode, so it measures what will actually be drawn
/// rather than an estimate from character counts.
#[cfg(target_os = "macos")]
unsafe fn resize_to_fit(ns_win: *mut AnyObject, field: *mut AnyObject) {
    use objc2_foundation::{NSRect, NSSize};

    let cell: *mut AnyObject = msg_send![field, cell];
    let probe = NSRect::new(
        objc2_foundation::NSPoint::new(0.0, 0.0),
        NSSize::new(WIDTH - PADDING * 2.0, 10_000.0),
    );
    let needed: NSSize = msg_send![cell, cellSizeForBounds: probe];
    let lines = (needed.height / LINE_HEIGHT).ceil();
    let height = height_for_lines(lines);

    let frame: NSRect = msg_send![ns_win, frame];
    if (frame.size.height - height).abs() < 0.5 {
        return;
    }

    // Keep the top edge where it is: the strip hangs under the pill.
    let top = frame.origin.y + frame.size.height;
    let new_frame = NSRect::new(
        objc2_foundation::NSPoint::new(frame.origin.x, top - height),
        NSSize::new(WIDTH, height),
    );
    let _: () = msg_send![ns_win, setFrame: new_frame, display: true];

    let content: *mut AnyObject = msg_send![ns_win, contentView];
    let content_rect = NSRect::new(
        objc2_foundation::NSPoint::new(0.0, 0.0),
        NSSize::new(WIDTH, height),
    );
    let _: () = msg_send![content, setFrame: content_rect];
    let text_rect = NSRect::new(
        objc2_foundation::NSPoint::new(PADDING, PADDING),
        NSSize::new(WIDTH - PADDING * 2.0, height - PADDING * 2.0),
    );
    let _: () = msg_send![field, setFrame: text_rect];
}

/// Same screen-picking as the pill: NSScreen.mainScreen returns the menu bar
/// screen for Accessory apps (Apple bug FB11506568), so follow the cursor.
#[cfg(target_os = "macos")]
unsafe fn position_under_pill(ns_win: *mut AnyObject, height: f64) {
    use objc2_foundation::{NSPoint, NSRect};

    let mouse_loc: NSPoint = msg_send![AnyClass::get(c"NSEvent").unwrap(), mouseLocation];
    let screens: *mut AnyObject = msg_send![AnyClass::get(c"NSScreen").unwrap(), screens];
    let count: usize = msg_send![screens, count];
    for i in 0..count {
        let scr: *mut AnyObject = msg_send![screens, objectAtIndex: i];
        let frame: NSRect = msg_send![scr, frame];
        if mouse_loc.x >= frame.origin.x
            && mouse_loc.x < frame.origin.x + frame.size.width
            && mouse_loc.y >= frame.origin.y
            && mouse_loc.y < frame.origin.y + frame.size.height
        {
            let x = frame.origin.x + (frame.size.width - WIDTH) / 2.0;
            let y = frame.origin.y + frame.size.height - height - TOP_OFFSET;
            let _: () = msg_send![ns_win, setFrameOrigin: NSPoint::new(x, y)];
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    #[cfg(target_os = "macos")]
    #[test]
    fn height_grows_from_one_line_to_five() {
        let one = super::height_for_lines(1.0);
        let five = super::height_for_lines(5.0);
        assert!(five > one);
        assert_eq!(super::height_for_lines(0.0), one, "jamais moins d'une ligne");
        assert_eq!(super::height_for_lines(99.0), five, "plafonne a cinq lignes");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn sits_below_the_pill() {
        // The pill occupies 40..72 from the top; the strip must clear it.
        assert!(super::TOP_OFFSET >= 40.0 + 32.0);
    }
}
