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

impl HotkeyOption {
    // CGEventFlags masks (from CGEvent.h / IOKit/hidsystem)
    const CG_MASK_COMMAND: u64 = 1 << 20; // kCGEventFlagMaskCommand = 0x100000
    const CG_MASK_ALTERNATE: u64 = 1 << 19; // kCGEventFlagMaskAlternate = 0x80000
    const CG_MASK_CONTROL: u64 = 1 << 18; // kCGEventFlagMaskControl = 0x40000
    const CG_MASK_SHIFT: u64 = 1 << 17; // kCGEventFlagMaskShift = 0x20000

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

    #[allow(dead_code)] // will be used by hotkey settings UI
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

    #[allow(dead_code)] // will be used by hotkey settings UI
    pub fn name(&self) -> &'static str {
        match self.key_code {
            0x3D => "right_option",
            0x3E => "right_control",
            0x3C => "right_shift",
            _ => "right_command",
        }
    }
}

/// Cancel shortcut key codes.
pub mod cancel_keys {
    pub const ESCAPE: u16 = 0x35;
    pub const NONE: u16 = 0; // disabled

    pub fn from_name(name: &str) -> u16 {
        match name {
            "escape" => ESCAPE,
            _ => NONE,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum HotkeyEvent {
    KeyDown,
    KeyUp,
    CancelPressed,
}

/// Messages to update hotkey configuration at runtime.
/// Variants are handled in the monitor loop; constructed by callers via the update_tx channel.
pub enum HotkeyUpdate {
    SetHotkey(HotkeyOption),
    SetCancelKey(u16),
}

/// Start monitoring the hotkey on a background thread.
/// Returns a receiver for hotkey events and a sender to update the hotkey/cancel key.
/// The monitoring thread waits for `enabled` to become true before creating the CGEvent tap.
#[cfg(target_os = "macos")]
pub fn start_monitor(
    initial_hotkey: HotkeyOption,
    initial_cancel_key: u16,
    enabled: Arc<AtomicBool>,
) -> (mpsc::Receiver<HotkeyEvent>, mpsc::Sender<HotkeyUpdate>) {
    let (event_tx, event_rx) = mpsc::channel::<HotkeyEvent>();
    let (update_tx, update_rx) = mpsc::channel::<HotkeyUpdate>();

    std::thread::spawn(move || {
        // Wait until monitoring is enabled (permissions confirmed)
        while !enabled.load(Ordering::SeqCst) {
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
        log::info!("Hotkey monitoring enabled, starting event tap");
        run_event_tap(initial_hotkey, initial_cancel_key, event_tx, update_rx);
    });

    (event_rx, update_tx)
}

#[cfg(target_os = "macos")]
fn run_event_tap(
    initial_hotkey: HotkeyOption,
    initial_cancel_key: u16,
    event_tx: mpsc::Sender<HotkeyEvent>,
    update_rx: mpsc::Receiver<HotkeyUpdate>,
) {
    use std::os::raw::c_void;
    use std::sync::atomic::{AtomicBool, AtomicU16, AtomicU64, Ordering};

    // Shared state between callback and thread
    static KEY_CODE: AtomicU16 = AtomicU16::new(0x36);
    static FLAG_MASK: AtomicU64 = AtomicU64::new(HotkeyOption::CG_MASK_COMMAND);
    static KEY_HELD: AtomicBool = AtomicBool::new(false);
    static CANCEL_KEY_CODE: AtomicU16 = AtomicU16::new(0x35); // Escape

    KEY_CODE.store(initial_hotkey.key_code, Ordering::SeqCst);
    FLAG_MASK.store(initial_hotkey.flag_mask, Ordering::SeqCst);
    KEY_HELD.store(false, Ordering::SeqCst);
    CANCEL_KEY_CODE.store(initial_cancel_key, Ordering::SeqCst);

    // Store event_tx in a Box leaked into a raw pointer for the callback
    let tx_ptr = Box::into_raw(Box::new(event_tx.clone())) as *mut c_void;

    extern "C" fn callback(
        _proxy: *mut c_void,
        event_type: u32,
        event: *mut c_void,
        user_info: *mut c_void,
    ) -> *mut c_void {
        // CGEventType values
        const KEY_DOWN: u32 = 10;
        const FLAGS_CHANGED: u32 = 12;
        const TAP_DISABLED_BY_TIMEOUT: u32 = 0xFFFFFFFE;
        const TAP_DISABLED_BY_USER: u32 = 0xFFFFFFFF;

        if event_type == TAP_DISABLED_BY_TIMEOUT || event_type == TAP_DISABLED_BY_USER {
            log::warn!("CGEvent tap disabled (type={}), will re-enable", event_type);
            return event;
        }

        if event_type != FLAGS_CHANGED && event_type != KEY_DOWN {
            return event;
        }

        // SAFETY: Called from CGEventTap callback. `event` is a valid CGEventRef.
        // CGEventGetIntegerValueField and CGEventGetFlags are CoreGraphics C functions.
        // user_info is a leaked Box<Sender<HotkeyEvent>> — valid for the lifetime of the tap.
        unsafe {
            #[link(name = "CoreGraphics", kind = "framework")]
            extern "C" {
                fn CGEventGetIntegerValueField(event: *mut c_void, field: u32) -> i64;
                fn CGEventGetFlags(event: *mut c_void) -> u64;
            }

            // kCGKeyboardEventKeycode = 9 (CGEventField enum)
            let key_code = CGEventGetIntegerValueField(event, 9) as u16;

            // Handle cancel key (keyDown only)
            if event_type == KEY_DOWN {
                let cancel_code = CANCEL_KEY_CODE.load(Ordering::SeqCst);
                if cancel_code != 0 && key_code == cancel_code {
                    let tx = &*(user_info as *const mpsc::Sender<HotkeyEvent>);
                    log::debug!("Cancel key pressed (code=0x{:02x})", key_code);
                    let _ = tx.send(HotkeyEvent::CancelPressed);
                }
                return event;
            }

            // Handle modifier hotkey (flagsChanged)
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

    // SAFETY: CGEventTapCreate creates an active event tap for flagsChanged + keyDown events.
    // The callback pointer (tx_ptr) is a leaked Box<Sender> — lives until process exit.
    // CFMachPortCreateRunLoopSource/CFRunLoopAddSource wire the tap into the current runloop.
    // The loop re-enables the tap periodically (macOS may disable it on timeout).
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

        // CGEventMask for keyDown (bit 10) + flagsChanged (bit 12)
        let event_mask: u64 = (1 << 10) | (1 << 12);

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

            // Check for updates
            while let Ok(update) = update_rx.try_recv() {
                match update {
                    HotkeyUpdate::SetHotkey(new_hotkey) => {
                        KEY_CODE.store(new_hotkey.key_code, Ordering::SeqCst);
                        FLAG_MASK.store(new_hotkey.flag_mask, Ordering::SeqCst);
                        KEY_HELD.store(false, Ordering::SeqCst);
                        log::info!("Hotkey changed to {}", new_hotkey.label);
                    }
                    HotkeyUpdate::SetCancelKey(code) => {
                        CANCEL_KEY_CODE.store(code, Ordering::SeqCst);
                        log::info!("Cancel shortcut changed to keycode=0x{:02x}", code);
                    }
                }
            }
        }
    }
}

#[cfg(not(target_os = "macos"))]
pub fn start_monitor(
    _initial_hotkey: HotkeyOption,
    _initial_cancel_key: u16,
    _enabled: Arc<AtomicBool>,
) -> (mpsc::Receiver<HotkeyEvent>, mpsc::Sender<HotkeyUpdate>) {
    let (_event_tx, event_rx) = mpsc::channel();
    let (update_tx, _update_rx) = mpsc::channel();
    log::warn!("Hotkey monitoring not implemented on this platform");
    (event_rx, update_tx)
}
