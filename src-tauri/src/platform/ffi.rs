//! Shared CoreGraphics / CoreFoundation FFI declarations used by multiple modules
//! (hotkey.rs, macos.rs). Centralised here to avoid duplication.
//!
//! Apple docs:
//! - CGEventTapCreate: https://developer.apple.com/documentation/coregraphics/1454426-cgeventtapcreate
//! - CGEventGetFlags: https://developer.apple.com/documentation/coregraphics/1455885-cgeventgetflags
//! - CGEventGetIntegerValueField: https://developer.apple.com/documentation/coregraphics/1455506-cgeventgetintegervaluefield
//! - CFMachPortCreateRunLoopSource: https://developer.apple.com/documentation/corefoundation/1400919-cfmachportcreaterunloopsource
//! - CFRunLoop: https://developer.apple.com/documentation/corefoundation/cfrunloop

use std::os::raw::c_void;

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    pub fn CGEventTapCreate(
        tap: u32,
        place: u32,
        options: u32,
        events_of_interest: u64,
        callback: extern "C" fn(*mut c_void, u32, *mut c_void, *mut c_void) -> *mut c_void,
        user_info: *mut c_void,
    ) -> *mut c_void;

    pub fn CGEventTapEnable(tap: *mut c_void, enable: bool);

    pub fn CGEventGetIntegerValueField(event: *mut c_void, field: u32) -> i64;

    pub fn CGEventGetFlags(event: *mut c_void) -> u64;

    /// Returns the current modifier flags from a given event source state.
    /// stateID: 0 = private, 1 = combined session, 2 = HID system
    pub fn CGEventSourceFlagsState(stateID: u32) -> u64;
}

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    pub fn CFMachPortCreateRunLoopSource(
        allocator: *const c_void,
        port: *mut c_void,
        order: i64,
    ) -> *mut c_void;

    pub fn CFRunLoopAddSource(
        rl: *mut c_void,
        source: *mut c_void,
        mode: *const c_void,
    );

    pub fn CFRunLoopGetCurrent() -> *mut c_void;

    pub fn CFRunLoopRunInMode(
        mode: *const c_void,
        seconds: f64,
        return_after_source_handled: bool,
    ) -> i32;

    pub static kCFRunLoopCommonModes: *const c_void;
    pub static kCFRunLoopDefaultMode: *const c_void;
}
