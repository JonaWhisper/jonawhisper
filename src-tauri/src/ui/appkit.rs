//! AppKit helpers shared by the bitmap overlays.

use objc2::msg_send;
use objc2::runtime::{AnyClass, AnyObject};
use objc2_foundation::{NSPoint, NSRect, NSSize};

/// Backing-store scale of the screens these overlays live on. Windows reads
/// its own from the monitor; macOS overlays are Retina.
pub(crate) const DPR: f32 = 2.0;

/// Wrapper for raw AppKit pointers that are created on the main thread and
/// accessed exclusively through `run_on_main_thread`.
///
/// # Safety
/// These pointers are only dereferenced inside closures dispatched to the main
/// thread via `AppHandle::run_on_main_thread`, and a `Mutex` around whatever
/// holds them keeps that access serial. Sending the wrapper across threads is
/// safe because it is never dereferenced off the main thread.
pub(crate) struct MainThreadPtr(pub(crate) *mut AnyObject);

unsafe impl Send for MainThreadPtr {}

/// Hand an RGBA buffer to an NSImageView. `points` is the size the bitmap is
/// drawn at, so a Retina buffer of twice the pixels lands at the same size.
///
/// # Safety
/// `iv` must be a live NSImageView, and `rgba` must hold `width * height * 4`
/// bytes.
pub(crate) unsafe fn set_view_image(
    iv: *mut AnyObject,
    rgba: &[u8],
    width: usize,
    height: usize,
    points: NSSize,
) {
    let null_planes: *const *mut u8 = std::ptr::null();
    // Device, not calibrated: a calibrated space converts the bytes on the way
    // in and lands visibly lighter than the values we drew.
    let cs = objc2_foundation::NSString::from_str("NSDeviceRGBColorSpace");

    let rep: *mut AnyObject = msg_send![AnyClass::get(c"NSBitmapImageRep").unwrap(), alloc];
    let rep: *mut AnyObject = msg_send![rep,
        initWithBitmapDataPlanes: null_planes,
        pixelsWide: width as i64,
        pixelsHigh: height as i64,
        bitsPerSample: 8i64,
        samplesPerPixel: 4i64,
        hasAlpha: true,
        isPlanar: false,
        colorSpaceName: &*cs,
        bytesPerRow: (width * 4) as i64,
        bitsPerPixel: 32i64
    ];

    let bitmap_data: *mut u8 = msg_send![rep, bitmapData];
    std::ptr::copy_nonoverlapping(rgba.as_ptr(), bitmap_data, rgba.len());

    let img: *mut AnyObject = msg_send![AnyClass::get(c"NSImage").unwrap(), alloc];
    let img: *mut AnyObject = msg_send![img, initWithSize: points];
    let _: () = msg_send![img, addRepresentation: rep];
    let _: () = msg_send![iv, setImage: img];
    let _: () = msg_send![img, release];
    let _: () = msg_send![rep, release];
}

/// A borderless, click-through, always-visible overlay window.
///
/// # Safety
/// Must run on the main thread.
pub(crate) unsafe fn overlay_window(rect: NSRect) -> *mut AnyObject {
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
    ns_win
}

/// Frame of the screen holding the cursor.
///
/// NSScreen.mainScreen returns the menu bar screen for Accessory apps (Apple
/// bug FB11506568), so the cursor is the reliable proxy for "where the user is
/// working".
///
/// # Safety
/// Must run on the main thread.
pub(crate) unsafe fn screen_under_cursor() -> Option<NSRect> {
    let mouse: NSPoint = msg_send![AnyClass::get(c"NSEvent").unwrap(), mouseLocation];
    let screens: *mut AnyObject = msg_send![AnyClass::get(c"NSScreen").unwrap(), screens];
    let count: usize = msg_send![screens, count];
    for i in 0..count {
        let scr: *mut AnyObject = msg_send![screens, objectAtIndex: i];
        let frame: NSRect = msg_send![scr, frame];
        if mouse.x >= frame.origin.x
            && mouse.x < frame.origin.x + frame.size.width
            && mouse.y >= frame.origin.y
            && mouse.y < frame.origin.y + frame.size.height
        {
            return Some(frame);
        }
    }
    None
}
