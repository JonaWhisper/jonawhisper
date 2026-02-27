//! Global hotkey monitoring via CGEvent tap (macOS).
//! Detects modifier-only keys (Right Command, Right Option, etc.)
//! by watching flagsChanged events — same approach as the Swift KeyMonitor.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};

/// Hotkey options matching the Swift HotkeyOption
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HotkeyOption {
    pub key_code: u16,
    pub flag_mask: u64,
    pub label: &'static str,
}

#[allow(dead_code)]
impl HotkeyOption {
    // NX masks (unused but kept for reference)
    const MASK_COMMAND: u64 = 0x00000100;
    const MASK_ALTERNATE: u64 = 0x00000020;
    const MASK_CONTROL: u64 = 0x00000001;
    const MASK_SHIFT: u64 = 0x00000002;

    // macOS CGEventFlags values (from IOKit/hidsystem)
    const CG_MASK_COMMAND: u64 = 1 << 20; // 0x100000
    const CG_MASK_ALTERNATE: u64 = 1 << 19; // 0x80000
    const CG_MASK_CONTROL: u64 = 1 << 18; // 0x40000
    const CG_MASK_SHIFT: u64 = 1 << 17; // 0x20000

    pub const RIGHT_COMMAND: HotkeyOption = HotkeyOption {
        key_code: 0x36,
        flag_mask: Self::CG_MASK_COMMAND,
        label: "Right Command",
    };

    pub const RIGHT_OPTION: HotkeyOption = HotkeyOption {
        key_code: 0x3D,
        flag_mask: Self::CG_MASK_ALTERNATE,
        label: "Right Option",
    };

    pub const RIGHT_CONTROL: HotkeyOption = HotkeyOption {
        key_code: 0x3E,
        flag_mask: Self::CG_MASK_CONTROL,
        label: "Right Control",
    };

    pub const RIGHT_SHIFT: HotkeyOption = HotkeyOption {
        key_code: 0x3C,
        flag_mask: Self::CG_MASK_SHIFT,
        label: "Right Shift",
    };

    pub const ALL: &'static [HotkeyOption] = &[
        Self::RIGHT_COMMAND,
        Self::RIGHT_OPTION,
        Self::RIGHT_CONTROL,
        Self::RIGHT_SHIFT,
    ];

    pub fn from_name(name: &str) -> HotkeyOption {
        match name {
            "right_option" => Self::RIGHT_OPTION,
            "right_control" => Self::RIGHT_CONTROL,
            "right_shift" => Self::RIGHT_SHIFT,
            _ => Self::RIGHT_COMMAND,
        }
    }

    pub fn name(&self) -> &'static str {
        match self.key_code {
            0x3D => "right_option",
            0x3E => "right_control",
            0x3C => "right_shift",
            _ => "right_command",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum HotkeyEvent {
    KeyDown,
    KeyUp,
}

/// Start monitoring the hotkey on a background thread.
/// Returns a receiver for hotkey events and a sender to update the hotkey option.
/// The monitoring thread waits for `enabled` to become true before creating the CGEvent tap.
#[cfg(target_os = "macos")]
pub fn start_monitor(
    initial_hotkey: HotkeyOption,
    enabled: Arc<AtomicBool>,
) -> (mpsc::Receiver<HotkeyEvent>, mpsc::Sender<HotkeyOption>) {
    let (event_tx, event_rx) = mpsc::channel::<HotkeyEvent>();
    let (hotkey_tx, hotkey_rx) = mpsc::channel::<HotkeyOption>();

    std::thread::spawn(move || {
        // Wait until monitoring is enabled (permissions confirmed)
        while !enabled.load(Ordering::SeqCst) {
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
        log::info!("Hotkey monitoring enabled, starting event tap");
        run_event_tap(initial_hotkey, event_tx, hotkey_rx);
    });

    (event_rx, hotkey_tx)
}

#[cfg(target_os = "macos")]
fn run_event_tap(
    initial_hotkey: HotkeyOption,
    event_tx: mpsc::Sender<HotkeyEvent>,
    hotkey_rx: mpsc::Receiver<HotkeyOption>,
) {
    use std::os::raw::c_void;
    use std::sync::atomic::{AtomicBool, AtomicU16, AtomicU64, Ordering};

    // Shared state between callback and thread
    static KEY_CODE: AtomicU16 = AtomicU16::new(0x36);
    static FLAG_MASK: AtomicU64 = AtomicU64::new(HotkeyOption::CG_MASK_COMMAND);
    static KEY_HELD: AtomicBool = AtomicBool::new(false);

    KEY_CODE.store(initial_hotkey.key_code, Ordering::SeqCst);
    FLAG_MASK.store(initial_hotkey.flag_mask, Ordering::SeqCst);
    KEY_HELD.store(false, Ordering::SeqCst);

    // Store event_tx in a Box leaked into a raw pointer for the callback
    let tx_ptr = Box::into_raw(Box::new(event_tx.clone())) as *mut c_void;

    extern "C" fn callback(
        _proxy: *mut c_void,
        event_type: u32,
        event: *mut c_void,
        user_info: *mut c_void,
    ) -> *mut c_void {
        // CGEventType values
        const FLAGS_CHANGED: u32 = 12;
        const TAP_DISABLED_BY_TIMEOUT: u32 = 0xFFFFFFFE;
        const TAP_DISABLED_BY_USER: u32 = 0xFFFFFFFF;

        if event_type == TAP_DISABLED_BY_TIMEOUT || event_type == TAP_DISABLED_BY_USER {
            log::warn!("CGEvent tap disabled (type={}), will re-enable", event_type);
            return event;
        }

        if event_type != FLAGS_CHANGED {
            return event;
        }

        unsafe {
            // Get keycode from event
            // CGEventGetIntegerValueField(event, kCGKeyboardEventKeycode = 6)
            #[link(name = "CoreGraphics", kind = "framework")]
            extern "C" {
                fn CGEventGetIntegerValueField(event: *mut c_void, field: u32) -> i64;
                fn CGEventGetFlags(event: *mut c_void) -> u64;
            }

            let key_code = CGEventGetIntegerValueField(event, 9) as u16; // kCGKeyboardEventKeycode = 9
            let flags = CGEventGetFlags(event);

            log::debug!("flagsChanged: keycode=0x{:02x} flags=0x{:x}", key_code, flags);

            let expected_code = KEY_CODE.load(Ordering::SeqCst);
            let expected_mask = FLAG_MASK.load(Ordering::SeqCst);

            if key_code == expected_code {
                let tx = &*(user_info as *const mpsc::Sender<HotkeyEvent>);

                if (flags & expected_mask) != 0 {
                    // Key pressed
                    if !KEY_HELD.load(Ordering::SeqCst) {
                        KEY_HELD.store(true, Ordering::SeqCst);
                        log::debug!("Hotkey callback: KeyDown (code={}, flags=0x{:x})", key_code, flags);
                        let _ = tx.send(HotkeyEvent::KeyDown);
                    }
                } else {
                    // Key released
                    if KEY_HELD.load(Ordering::SeqCst) {
                        KEY_HELD.store(false, Ordering::SeqCst);
                        log::debug!("Hotkey callback: KeyUp (code={}, flags=0x{:x})", key_code, flags);
                        let _ = tx.send(HotkeyEvent::KeyUp);
                    }
                }
            }
        }

        event
    }

    unsafe {
        #[link(name = "CoreGraphics", kind = "framework")]
        extern "C" {
            fn CGEventTapCreate(
                tap: u32,
                place: u32,
                options: u32,
                events_of_interest: u64,
                callback: extern "C" fn(*mut c_void, u32, *mut c_void, *mut c_void) -> *mut c_void,
                user_info: *mut c_void,
            ) -> *mut c_void; // CFMachPortRef

            fn CGEventTapEnable(tap: *mut c_void, enable: bool);
        }

        #[link(name = "CoreFoundation", kind = "framework")]
        extern "C" {
            fn CFMachPortCreateRunLoopSource(
                allocator: *const c_void,
                port: *mut c_void,
                order: i64,
            ) -> *mut c_void; // CFRunLoopSourceRef

            fn CFRunLoopAddSource(
                rl: *mut c_void,
                source: *mut c_void,
                mode: *const c_void,
            );

            fn CFRunLoopGetCurrent() -> *mut c_void;
            fn CFRunLoopRunInMode(
                mode: *const c_void,
                seconds: f64,
                return_after_source_handled: bool,
            ) -> i32;

            static kCFRunLoopCommonModes: *const c_void;
            static kCFRunLoopDefaultMode: *const c_void;
        }

        // CGEventMask for flagsChanged (bit 12)
        let event_mask: u64 = 1 << 12;

        let tap = CGEventTapCreate(
            1, // cgSessionEventTap
            0, // headInsertEventTap
            0, // defaultTap (not listenOnly — we need to intercept)
            event_mask,
            callback,
            tx_ptr,
        );

        if tap.is_null() {
            log::error!("Failed to create CGEvent tap. Input Monitoring permission required.");
            return;
        }

        let source = CFMachPortCreateRunLoopSource(std::ptr::null(), tap, 0);
        let rl = CFRunLoopGetCurrent();
        CFRunLoopAddSource(rl, source, kCFRunLoopCommonModes);
        CGEventTapEnable(tap, true);

        log::info!("Hotkey monitor started ({})", initial_hotkey.label);

        // Run the event loop, periodically checking for hotkey updates
        loop {
            // Run the loop for a short interval
            CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.5, false);

            // Re-enable tap in case macOS disabled it
            CGEventTapEnable(tap, true);

            // Check if hotkey was updated
            if let Ok(new_hotkey) = hotkey_rx.try_recv() {
                KEY_CODE.store(new_hotkey.key_code, Ordering::SeqCst);
                FLAG_MASK.store(new_hotkey.flag_mask, Ordering::SeqCst);
                KEY_HELD.store(false, Ordering::SeqCst);
                log::info!("Hotkey changed to {}", new_hotkey.label);
            }
        }
    }
}

#[cfg(not(target_os = "macos"))]
pub fn start_monitor(
    _initial_hotkey: HotkeyOption,
    _enabled: Arc<AtomicBool>,
) -> (mpsc::Receiver<HotkeyEvent>, mpsc::Sender<HotkeyOption>) {
    let (event_tx, event_rx) = mpsc::channel();
    let (hotkey_tx, _hotkey_rx) = mpsc::channel();
    log::warn!("Hotkey monitoring not implemented on this platform");
    (event_rx, hotkey_tx)
}
