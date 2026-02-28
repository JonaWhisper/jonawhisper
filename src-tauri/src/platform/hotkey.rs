//! Global hotkey monitoring via CGEvent tap (macOS).
//! Detects modifier-only keys (Right Command, Right Option, etc.)
//! by watching flagsChanged events — same approach as the Swift KeyMonitor.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

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
) -> (crossbeam_channel::Receiver<HotkeyEvent>, crossbeam_channel::Sender<HotkeyUpdate>) {
    let (event_tx, event_rx) = crossbeam_channel::unbounded::<HotkeyEvent>();
    let (update_tx, update_rx) = crossbeam_channel::unbounded::<HotkeyUpdate>();

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

/// Shared state between the CGEvent callback and the main hotkey thread.
/// Passed via user_info pointer (leaked Box) instead of static atomics.
#[cfg(target_os = "macos")]
struct TapState {
    key_code: std::sync::atomic::AtomicU16,
    flag_mask: std::sync::atomic::AtomicU64,
    key_held: AtomicBool,
    cancel_key_code: std::sync::atomic::AtomicU16,
    event_tx: crossbeam_channel::Sender<HotkeyEvent>,
}

#[cfg(target_os = "macos")]
fn run_event_tap(
    initial_hotkey: HotkeyOption,
    initial_cancel_key: u16,
    event_tx: crossbeam_channel::Sender<HotkeyEvent>,
    update_rx: crossbeam_channel::Receiver<HotkeyUpdate>,
) {
    use std::os::raw::c_void;
    use std::sync::atomic::{AtomicU16, AtomicU64, Ordering};

    // All callback state in a single struct, passed via user_info
    let state = Box::new(TapState {
        key_code: AtomicU16::new(initial_hotkey.key_code),
        flag_mask: AtomicU64::new(initial_hotkey.flag_mask),
        key_held: AtomicBool::new(false),
        cancel_key_code: AtomicU16::new(initial_cancel_key),
        event_tx,
    });
    let state_ptr = Box::into_raw(state);

    extern "C" fn callback(
        _proxy: *mut c_void,
        event_type: u32,
        event: *mut c_void,
        user_info: *mut c_void,
    ) -> *mut c_void {
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

        // SAFETY: user_info is a leaked Box<TapState> — valid for the lifetime of the tap.
        // CGEventGetIntegerValueField and CGEventGetFlags are CoreGraphics C functions.
        unsafe {
            use std::sync::atomic::Ordering;
            use super::ffi::{CGEventGetIntegerValueField, CGEventGetFlags};

            let state = &*(user_info as *const TapState);
            let key_code = CGEventGetIntegerValueField(event, 9) as u16;

            // Handle cancel key (keyDown only)
            if event_type == KEY_DOWN {
                let cancel_code = state.cancel_key_code.load(Ordering::SeqCst);
                if cancel_code != 0 && key_code == cancel_code {
                    log::debug!("Cancel key pressed (code=0x{:02x})", key_code);
                    let _ = state.event_tx.send(HotkeyEvent::CancelPressed);
                }
                return event;
            }

            // Handle modifier hotkey (flagsChanged)
            let flags = CGEventGetFlags(event);
            log::debug!("flagsChanged: keycode=0x{:02x} flags=0x{:x}", key_code, flags);

            let expected_code = state.key_code.load(Ordering::SeqCst);
            let expected_mask = state.flag_mask.load(Ordering::SeqCst);

            if key_code == expected_code {
                if (flags & expected_mask) != 0 {
                    if !state.key_held.load(Ordering::SeqCst) {
                        state.key_held.store(true, Ordering::SeqCst);
                        log::debug!("Hotkey callback: KeyDown (code={}, flags=0x{:x})", key_code, flags);
                        let _ = state.event_tx.send(HotkeyEvent::KeyDown);
                    }
                } else if state.key_held.load(Ordering::SeqCst) {
                    state.key_held.store(false, Ordering::SeqCst);
                    log::debug!("Hotkey callback: KeyUp (code={}, flags=0x{:x})", key_code, flags);
                    let _ = state.event_tx.send(HotkeyEvent::KeyUp);
                }
            }
        }

        event
    }

    // SAFETY: CGEventTapCreate creates an active event tap for flagsChanged + keyDown events.
    // state_ptr is a leaked Box<TapState> — lives until process exit.
    // CFMachPortCreateRunLoopSource/CFRunLoopAddSource wire the tap into the current runloop.
    unsafe {
        use super::ffi;

        let event_mask: u64 = (1 << 10) | (1 << 12);

        let tap = ffi::CGEventTapCreate(
            1, 0, 0, event_mask, callback,
            state_ptr as *mut c_void,
        );

        if tap.is_null() {
            log::error!("Failed to create CGEvent tap. Input Monitoring permission required.");
            return;
        }

        let source = ffi::CFMachPortCreateRunLoopSource(std::ptr::null(), tap, 0);
        let rl = ffi::CFRunLoopGetCurrent();
        ffi::CFRunLoopAddSource(rl, source, ffi::kCFRunLoopCommonModes);
        ffi::CGEventTapEnable(tap, true);

        log::info!("Hotkey monitor started ({})", initial_hotkey.label);

        let state = &*state_ptr;
        loop {
            ffi::CFRunLoopRunInMode(ffi::kCFRunLoopDefaultMode, 0.5, false);
            ffi::CGEventTapEnable(tap, true);

            while let Ok(update) = update_rx.try_recv() {
                match update {
                    HotkeyUpdate::SetHotkey(new_hotkey) => {
                        state.key_code.store(new_hotkey.key_code, Ordering::SeqCst);
                        state.flag_mask.store(new_hotkey.flag_mask, Ordering::SeqCst);
                        state.key_held.store(false, Ordering::SeqCst);
                        log::info!("Hotkey changed to {}", new_hotkey.label);
                    }
                    HotkeyUpdate::SetCancelKey(code) => {
                        state.cancel_key_code.store(code, Ordering::SeqCst);
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
) -> (crossbeam_channel::Receiver<HotkeyEvent>, crossbeam_channel::Sender<HotkeyUpdate>) {
    let (_event_tx, event_rx) = crossbeam_channel::unbounded();
    let (update_tx, _update_rx) = crossbeam_channel::unbounded();
    log::warn!("Hotkey monitoring not implemented on this platform");
    (event_rx, update_tx)
}
