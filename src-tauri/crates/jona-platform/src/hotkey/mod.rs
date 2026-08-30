//! Global hotkey monitoring via CGEvent tap (macOS).
//! Supports three shortcut kinds:
//! - ModifierOnly: modifier key(s) only (e.g. Right Command, Right ⌘ + Left ⌥)
//! - Combo: modifier(s) + regular key(s) (e.g. Cmd+R, Ctrl+Space, Cmd+A+B)
//! - Key: standalone key(s) without modifiers (e.g. F13, Escape)
//!
//! Multi-key shortcuts: during capture, keys accumulate until the first key-up
//! finalises the shortcut.  In normal mode, the shortcut fires when ALL required
//! keys are held simultaneously.

use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering};
use std::sync::Arc;


mod shortcut;
mod slot;

pub use shortcut::{CaptureControl, HotkeyEvent, HotkeyUpdate, Shortcut, ShortcutKind};
use shortcut::{
    is_modifier_key_code, modifier_flag_for_key_code, packed_add, packed_remove, unpack_keys,
};
/// Only the CGEvent tap compares a shortcut against every pressed key at once;
/// the Windows hook tracks them one event at a time.
#[cfg(target_os = "macos")]
use shortcut::{pack_keys, packed_contains_all};
use slot::{ShortcutSlot, modifier_shortcut_matches, shortcut_matches};

// -- CGEventFlags masks --

/// Key codes for the shortcut names stored in preferences. The names are
/// portable, the codes are not: macOS uses CGEvent codes, Windows virtual-key
/// codes, and 0x36 means Right Command on one and the digit 6 on the other.
#[cfg(target_os = "macos")]
mod keys {
    pub const RIGHT_COMMAND: u16 = 0x36;
    pub const RIGHT_OPTION: u16 = 0x3D;
    pub const RIGHT_CONTROL: u16 = 0x3E;
    pub const RIGHT_SHIFT: u16 = 0x3C;
    pub const ESCAPE: u16 = 0x35;

    pub const LEFT_COMMAND: u16 = 0x37;
    pub const LEFT_OPTION: u16 = 0x3A;
    pub const LEFT_CONTROL: u16 = 0x3B;
    pub const LEFT_SHIFT: u16 = 0x38;
    /// Caps Lock counts as a modifier on macOS.
    pub const EXTRA_MODIFIER: u16 = 0x3F;
}

/// Windows virtual-key codes. Right Command maps to the right Windows key: it
/// sits in the same place and carries the same role as a system modifier.
#[cfg(not(target_os = "macos"))]
mod keys {
    pub const RIGHT_COMMAND: u16 = 0x5C; // VK_RWIN
    pub const RIGHT_OPTION: u16 = 0xA5; // VK_RMENU (right Alt)
    pub const RIGHT_CONTROL: u16 = 0xA3; // VK_RCONTROL
    pub const RIGHT_SHIFT: u16 = 0xA1; // VK_RSHIFT
    pub const ESCAPE: u16 = 0x1B; // VK_ESCAPE

    pub const LEFT_COMMAND: u16 = 0x5B; // VK_LWIN
    pub const LEFT_OPTION: u16 = 0xA4; // VK_LMENU
    pub const LEFT_CONTROL: u16 = 0xA2; // VK_LCONTROL
    pub const LEFT_SHIFT: u16 = 0xA0; // VK_LSHIFT
    /// No Caps Lock equivalent to treat as a held modifier; VK_APPS is inert
    /// here and keeps the table the same shape on both platforms.
    pub const EXTRA_MODIFIER: u16 = 0x5D; // VK_APPS
}

const CG_MASK_COMMAND: u64 = 1 << 20;
const CG_MASK_SHIFT: u64 = 1 << 17;
const CG_MASK_ALTERNATE: u64 = 1 << 19;
const CG_MASK_CONTROL: u64 = 1 << 18;

// Modifier masks are CGEvent's today. A Windows backend will report its own
// flags; these aliases mark the values that will have to follow.
const MASK_COMMAND: u64 = CG_MASK_COMMAND;
const MASK_SHIFT: u64 = CG_MASK_SHIFT;
const MASK_ALTERNATE: u64 = CG_MASK_ALTERNATE;
const MASK_CONTROL: u64 = CG_MASK_CONTROL;
#[cfg(target_os = "macos")]
const CG_MASK_ALL_MODIFIERS: u64 =
    CG_MASK_COMMAND | CG_MASK_SHIFT | CG_MASK_ALTERNATE | CG_MASK_CONTROL;

// -- Monitor --

/// Start monitoring the hotkey on a background thread.
/// Returns a receiver for hotkey events and a sender to update config.
#[cfg(target_os = "macos")]
pub fn start_monitor(
    initial_record: Shortcut,
    initial_cancel: Shortcut,
    enabled: Arc<AtomicBool>,
    capture: Arc<CaptureControl>,
) -> (
    crossbeam_channel::Receiver<HotkeyEvent>,
    crossbeam_channel::Sender<HotkeyUpdate>,
) {
    let (event_tx, event_rx) = crossbeam_channel::unbounded::<HotkeyEvent>();
    let (update_tx, update_rx) = crossbeam_channel::unbounded::<HotkeyUpdate>();

    std::thread::spawn(move || {
        while !enabled.load(Ordering::SeqCst) {
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
        log::info!("Hotkey monitoring enabled, starting event tap");
        run_event_tap(
            initial_record,
            initial_cancel,
            event_tx,
            update_rx,
            capture,
        );
    });

    (event_rx, update_tx)
}

// -- TapState: shared between CGEvent callback and hotkey thread --

#[cfg(target_os = "macos")]
struct TapState {
    record: ShortcutSlot,
    rec_held: AtomicBool,
    cancel: ShortcutSlot,

    // Currently pressed keys (for multi-key matching in normal mode)
    pressed_keys_packed: AtomicU64,
    pressed_key_count: AtomicU8,
    pressed_mod_keys_packed: AtomicU64,
    pressed_mod_key_count: AtomicU8,

    // Capture mode (shared with Tauri commands via Arc)
    capture: Arc<CaptureControl>,

    event_tx: crossbeam_channel::Sender<HotkeyEvent>,
}

#[cfg(target_os = "macos")]
impl TapState {
    /// Storing a record shortcut also clears the held flag: the old shortcut's
    /// key-up will never arrive.
    fn store_record(&self, s: &Shortcut) {
        self.record.store(s);
        self.rec_held.store(false, Ordering::SeqCst);
    }
}

#[cfg(target_os = "macos")]
// Only the second arm of each symmetric Combo/Key pair is flagged; a guard there splits the pair.
#[allow(clippy::collapsible_match)]
fn run_event_tap(
    initial_record: Shortcut,
    initial_cancel: Shortcut,
    event_tx: crossbeam_channel::Sender<HotkeyEvent>,
    update_rx: crossbeam_channel::Receiver<HotkeyUpdate>,
    capture: Arc<CaptureControl>,
) {
    use std::os::raw::c_void;

    let state = Box::new(TapState {
        record: ShortcutSlot::new(&initial_record),
        rec_held: AtomicBool::new(false),
        cancel: ShortcutSlot::new(&initial_cancel),

        pressed_keys_packed: AtomicU64::new(0),
        pressed_key_count: AtomicU8::new(0),
        pressed_mod_keys_packed: AtomicU64::new(0),
        pressed_mod_key_count: AtomicU8::new(0),

        capture,

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
        const KEY_UP: u32 = 11;
        const FLAGS_CHANGED: u32 = 12;
        const TAP_DISABLED_BY_TIMEOUT: u32 = 0xFFFFFFFE;
        const TAP_DISABLED_BY_USER: u32 = 0xFFFFFFFF;

        if event_type == TAP_DISABLED_BY_TIMEOUT || event_type == TAP_DISABLED_BY_USER {
            log::warn!(
                "CGEvent tap disabled (type={}), will re-enable",
                event_type
            );
            // Safety: if recording was active, we may have missed the KeyUp.
            // Send a KeyUp now to prevent the mic from staying active forever.
            unsafe {
                let state = &*(user_info as *const TapState);
                if state.rec_held.swap(false, Ordering::SeqCst) {
                    log::warn!("Tap disabled while recording held — sending safety KeyUp");
                    let _ = state.event_tx.send(HotkeyEvent::KeyUp);
                }
                // Clear pressed key tracking (events during disable are lost)
                state.pressed_keys_packed.store(0, Ordering::SeqCst);
                state.pressed_key_count.store(0, Ordering::SeqCst);
                state.pressed_mod_keys_packed.store(0, Ordering::SeqCst);
                state.pressed_mod_key_count.store(0, Ordering::SeqCst);
            }
            return event;
        }

        if event_type != FLAGS_CHANGED && event_type != KEY_DOWN && event_type != KEY_UP {
            return event;
        }

        unsafe {
            use super::ffi::{CGEventGetFlags, CGEventGetIntegerValueField};

            let state = &*(user_info as *const TapState);
            let key_code = CGEventGetIntegerValueField(event, 9) as u16;
            let flags = CGEventGetFlags(event);
            let mod_flags = flags & CG_MASK_ALL_MODIFIERS;

            // -- Capture mode --
            if state.capture.mode.load(Ordering::SeqCst) {
                handle_capture(state, event_type, key_code, mod_flags);
                return event;
            }

            // -- Normal mode --
            handle_normal(state, event_type, key_code, mod_flags);
        }

        event
    }

    /// Handle events in capture mode: accumulate keys, finalise on first release.
    #[cfg(target_os = "macos")]
    unsafe fn handle_capture(
        state: &TapState,
        event_type: u32,
        key_code: u16,
        mod_flags: u64,
    ) {
        const KEY_DOWN: u32 = 10;
        const KEY_UP: u32 = 11;
        const FLAGS_CHANGED: u32 = 12;

        log::info!("CAPTURE-DIAG: tap saw type={event_type} key=0x{key_code:02X} flags=0x{mod_flags:X}");

        let cap = &state.capture;

        match event_type {
            KEY_DOWN => {
                if !is_modifier_key_code(key_code) {
                    packed_add(&cap.keys_packed, &cap.key_count, key_code);
                    cap.peak_modifiers.fetch_or(mod_flags, Ordering::SeqCst);
                    cap.active.store(true, Ordering::SeqCst);

                    let peak = cap.peak_modifiers.load(Ordering::SeqCst);
                    let keys = unpack_keys(
                        cap.keys_packed.load(Ordering::SeqCst),
                        cap.key_count.load(Ordering::SeqCst),
                    );
                    let _ = state.event_tx.send(HotkeyEvent::CaptureUpdate {
                        modifiers: peak,
                        key_codes: keys,
                    });
                }
            }
            KEY_UP => {
                if !is_modifier_key_code(key_code)
                    && cap.key_count.load(Ordering::SeqCst) > 0
                {
                    let peak = cap.peak_modifiers.load(Ordering::SeqCst);
                    let mut keys = unpack_keys(
                        cap.keys_packed.load(Ordering::SeqCst),
                        cap.key_count.load(Ordering::SeqCst),
                    );
                    keys.sort();

                    let kind = if peak != 0 {
                        ShortcutKind::Combo
                    } else {
                        ShortcutKind::Key
                    };

                    let shortcut = Shortcut {
                        key_codes: keys,
                        modifiers: peak,
                        kind,
                    };

                    cap.reset();
                    let _ = state
                        .event_tx
                        .send(HotkeyEvent::CaptureComplete(shortcut));
                }
            }
            FLAGS_CHANGED => {
                let modifier_pressed =
                    modifier_flag_for_key_code(key_code) & mod_flags != 0;

                if modifier_pressed {
                    packed_add(&cap.mod_keys_packed, &cap.mod_key_count, key_code);
                    cap.peak_modifiers.fetch_or(mod_flags, Ordering::SeqCst);
                    cap.active.store(true, Ordering::SeqCst);
                }

                if mod_flags != 0 || cap.key_count.load(Ordering::SeqCst) > 0 {
                    // Still have active modifiers or regular keys — emit update
                    let peak = cap.peak_modifiers.load(Ordering::SeqCst);
                    let keys = unpack_keys(
                        cap.keys_packed.load(Ordering::SeqCst),
                        cap.key_count.load(Ordering::SeqCst),
                    );
                    let _ = state.event_tx.send(HotkeyEvent::CaptureUpdate {
                        modifiers: peak,
                        key_codes: keys,
                    });
                } else if cap.active.load(Ordering::SeqCst)
                    && cap.key_count.load(Ordering::SeqCst) == 0
                {
                    // All modifiers released, no regular keys → ModifierOnly
                    let mut mod_keys = unpack_keys(
                        cap.mod_keys_packed.load(Ordering::SeqCst),
                        cap.mod_key_count.load(Ordering::SeqCst),
                    );
                    mod_keys.sort();
                    let peak = cap.peak_modifiers.load(Ordering::SeqCst);

                    let shortcut = Shortcut {
                        key_codes: mod_keys,
                        modifiers: peak,
                        kind: ShortcutKind::ModifierOnly,
                    };

                    cap.reset();
                    let _ = state
                        .event_tx
                        .send(HotkeyEvent::CaptureComplete(shortcut));
                }
            }
            _ => {}
        }
    }

    /// Handle events in normal mode (record/cancel shortcuts).
    #[cfg(target_os = "macos")]
    unsafe fn handle_normal(
        state: &TapState,
        event_type: u32,
        key_code: u16,
        mod_flags: u64,
    ) {
        const KEY_DOWN: u32 = 10;
        const KEY_UP: u32 = 11;
        const FLAGS_CHANGED: u32 = 12;

        let rec = state.record.load();
        let cancel = state.cancel.load();

        match event_type {
            KEY_DOWN => {
                if is_modifier_key_code(key_code) {
                    return;
                }
                // Track pressed regular keys
                packed_add(
                    &state.pressed_keys_packed,
                    &state.pressed_key_count,
                    key_code,
                );

                let pressed_p = state.pressed_keys_packed.load(Ordering::SeqCst);
                let pressed_c = state.pressed_key_count.load(Ordering::SeqCst);

                // Check cancel shortcut (Combo/Key)
                if shortcut_matches(&cancel, pressed_p, pressed_c, mod_flags) {
                    let _ = state.event_tx.send(HotkeyEvent::CancelPressed);
                    return;
                }

                // Check record shortcut (Combo/Key)
                if !rec.is_disabled() {
                    let (rec_p, rec_c) = pack_keys(&rec.key_codes);
                    match rec.kind {
                        ShortcutKind::Combo => {
                            if packed_contains_all(pressed_p, pressed_c, rec_p, rec_c)
                                && (mod_flags & rec.modifiers) == rec.modifiers
                                && !state.rec_held.load(Ordering::SeqCst)
                            {
                                state.rec_held.store(true, Ordering::SeqCst);
                                log::debug!(
                                    "Hotkey KeyDown (Combo): key=0x{:02x} mods=0x{:x}",
                                    key_code,
                                    mod_flags
                                );
                                let _ =
                                    state.event_tx.send(HotkeyEvent::KeyDown);
                            }
                        }
                        ShortcutKind::Key => {
                            if packed_contains_all(pressed_p, pressed_c, rec_p, rec_c)
                                && mod_flags == 0
                                && !state.rec_held.load(Ordering::SeqCst)
                            {
                                state.rec_held.store(true, Ordering::SeqCst);
                                log::debug!(
                                    "Hotkey KeyDown (Key): key=0x{:02x}",
                                    key_code
                                );
                                let _ =
                                    state.event_tx.send(HotkeyEvent::KeyDown);
                            }
                        }
                        _ => {}
                    }
                }
            }
            KEY_UP => {
                if is_modifier_key_code(key_code) {
                    return;
                }
                packed_remove(
                    &state.pressed_keys_packed,
                    &state.pressed_key_count,
                    key_code,
                );

                if !rec.is_disabled() && state.rec_held.load(Ordering::SeqCst) {
                    match rec.kind {
                        ShortcutKind::Combo | ShortcutKind::Key => {
                            // If a required key was released, send KeyUp
                            if rec.key_codes.contains(&key_code) {
                                state.rec_held.store(false, Ordering::SeqCst);
                                log::debug!(
                                    "Hotkey KeyUp: key=0x{:02x}",
                                    key_code
                                );
                                let _ =
                                    state.event_tx.send(HotkeyEvent::KeyUp);
                            }
                        }
                        _ => {}
                    }
                }
            }
            FLAGS_CHANGED => {
                // Track pressed modifier key codes
                let modifier_pressed =
                    modifier_flag_for_key_code(key_code) & mod_flags != 0;
                if modifier_pressed {
                    packed_add(
                        &state.pressed_mod_keys_packed,
                        &state.pressed_mod_key_count,
                        key_code,
                    );
                } else {
                    packed_remove(
                        &state.pressed_mod_keys_packed,
                        &state.pressed_mod_key_count,
                        key_code,
                    );
                }

                let pressed_mod_p =
                    state.pressed_mod_keys_packed.load(Ordering::SeqCst);
                let pressed_mod_c =
                    state.pressed_mod_key_count.load(Ordering::SeqCst);

                // -- Record shortcut --
                if !rec.is_disabled() {
                    match rec.kind {
                        ShortcutKind::ModifierOnly => {
                            let (rec_p, rec_c) = pack_keys(&rec.key_codes);
                            if packed_contains_all(
                                pressed_mod_p,
                                pressed_mod_c,
                                rec_p,
                                rec_c,
                            ) {
                                if !state.rec_held.load(Ordering::SeqCst) {
                                    state.rec_held.store(true, Ordering::SeqCst);
                                    log::debug!(
                                        "Hotkey KeyDown (ModifierOnly): key=0x{:02x} flags=0x{:x}",
                                        key_code,
                                        mod_flags
                                    );
                                    let _ =
                                        state.event_tx.send(HotkeyEvent::KeyDown);
                                }
                            } else if state.rec_held.load(Ordering::SeqCst) {
                                // A required modifier was released
                                state.rec_held.store(false, Ordering::SeqCst);
                                log::debug!(
                                    "Hotkey KeyUp (ModifierOnly): key=0x{:02x} flags=0x{:x}",
                                    key_code,
                                    mod_flags
                                );
                                let _ = state.event_tx.send(HotkeyEvent::KeyUp);
                            }
                        }
                        ShortcutKind::Combo => {
                            // If modifier released while holding combo, send KeyUp
                            if state.rec_held.load(Ordering::SeqCst)
                                && (mod_flags & rec.modifiers) != rec.modifiers
                            {
                                state.rec_held.store(false, Ordering::SeqCst);
                                log::debug!(
                                    "Hotkey KeyUp (Combo modifier released): flags=0x{:x}",
                                    mod_flags
                                );
                                let _ = state.event_tx.send(HotkeyEvent::KeyUp);
                            }
                        }
                        _ => {}
                    }
                }

                // -- Cancel shortcut (ModifierOnly) --
                if modifier_shortcut_matches(&cancel, pressed_mod_p, pressed_mod_c) {
                    let _ = state.event_tx.send(HotkeyEvent::CancelPressed);
                }
            }
            _ => {}
        }
    }

    // Event mask: flagsChanged(12) + keyDown(10) + keyUp(11)
    let event_mask: u64 = (1 << 10) | (1 << 11) | (1 << 12);

    // SAFETY: CGEventTapCreate creates an active event tap.
    // state_ptr is a leaked Box<TapState> — lives until process exit.
    unsafe {
        use super::ffi;
        use std::os::raw::c_void;

        let tap = ffi::CGEventTapCreate(
            1,  // kCGHIDEventTap
            0,  // kCGHeadInsertEventTap
            1,  // kCGEventTapOptionListenOnly — less prone to timeout disable
            event_mask,
            callback,
            state_ptr as *mut c_void,
        );

        if tap.is_null() {
            log::error!(
                "Failed to create CGEvent tap. Input Monitoring permission required."
            );
            return;
        }

        let source = ffi::CFMachPortCreateRunLoopSource(std::ptr::null(), tap, 0);
        let rl = ffi::CFRunLoopGetCurrent();
        ffi::CFRunLoopAddSource(rl, source, ffi::kCFRunLoopCommonModes);
        ffi::CGEventTapEnable(tap, true);

        log::info!(
            "Hotkey monitor started (record={}, cancel={})",
            initial_record.display_string(),
            initial_cancel.display_string()
        );

        let state = &*state_ptr;
        loop {
            ffi::CFRunLoopRunInMode(ffi::kCFRunLoopDefaultMode, 0.5, false);
            ffi::CGEventTapEnable(tap, true);

            // -- Safety watchdog --
            // If rec_held is true but the modifier/key is no longer physically pressed,
            // we missed a KeyUp (tap disabled, Secure Input, etc.). Send safety KeyUp.
            if state.rec_held.load(Ordering::SeqCst) {
                let rec = state.record.load();
                let current_flags =
                    ffi::CGEventSourceFlagsState(1) & CG_MASK_ALL_MODIFIERS;
                let key_still_held = match rec.kind {
                    ShortcutKind::ModifierOnly => {
                        (current_flags & rec.modifiers) == rec.modifiers
                    }
                    ShortcutKind::Combo => {
                        // For combos, at minimum the modifiers must still be held
                        (current_flags & rec.modifiers) == rec.modifiers
                    }
                    // For Key-only shortcuts, we can't poll key state easily —
                    // rely on tap-disabled handler only
                    ShortcutKind::Key => true,
                };
                if !key_still_held {
                    log::warn!(
                        "Watchdog: rec_held but modifier no longer pressed (flags=0x{:x}), sending safety KeyUp",
                        current_flags
                    );
                    state.rec_held.store(false, Ordering::SeqCst);
                    state.pressed_keys_packed.store(0, Ordering::SeqCst);
                    state.pressed_key_count.store(0, Ordering::SeqCst);
                    state.pressed_mod_keys_packed.store(0, Ordering::SeqCst);
                    state.pressed_mod_key_count.store(0, Ordering::SeqCst);
                    let _ = state.event_tx.send(HotkeyEvent::KeyUp);
                }
            }

            while let Ok(update) = update_rx.try_recv() {
                match update {
                    HotkeyUpdate::SetRecordShortcut(s) => {
                        log::info!(
                            "Record shortcut changed to {}",
                            s.display_string()
                        );
                        state.store_record(&s);
                    }
                    HotkeyUpdate::SetCancelShortcut(s) => {
                        log::info!(
                            "Cancel shortcut changed to {}",
                            s.display_string()
                        );
                        state.cancel.store(&s);
                    }
                }
            }
        }
    }
}

#[cfg(target_os = "windows")]
mod win {
    use super::*;
    use std::sync::OnceLock;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CallNextHookEx, DispatchMessageW, KBDLLHOOKSTRUCT, MSG, PM_REMOVE, PeekMessageW,
        SetWindowsHookExW, TranslateMessage, UnhookWindowsHookEx, WH_KEYBOARD_LL, WM_KEYDOWN,
        WM_SYSKEYDOWN,
    };

    /// The low-level keyboard hook receives no user pointer, so its state has to
    /// be reachable from a plain function.
    struct HookState {
        record: ShortcutSlot,
        rec_held: AtomicBool,
        cancel: ShortcutSlot,
        pressed_packed: AtomicU64,
        pressed_count: AtomicU8,
        capture: Arc<CaptureControl>,
        event_tx: crossbeam_channel::Sender<HotkeyEvent>,
    }

    static STATE: OnceLock<HookState> = OnceLock::new();

    /// Windows delivers modifiers as ordinary virtual-key codes, so one pressed
    /// set covers both and the flags are derived from it.
    fn modifier_flags(packed: u64, count: u8) -> u64 {
        unpack_keys(packed, count)
            .iter()
            .fold(0, |acc, &k| acc | modifier_flag_for_key_code(k))
    }

    fn on_key(state: &HookState, vk: u16, down: bool) {
        log::debug!("CAPTURE-DIAG: on_key vk=0x{vk:02X} down={down} capture={}", state.capture.mode.load(Ordering::SeqCst));
        if down {
            packed_add(&state.pressed_packed, &state.pressed_count, vk);
        } else {
            packed_remove(&state.pressed_packed, &state.pressed_count, vk);
        }

        let pressed_p = state.pressed_packed.load(Ordering::SeqCst);
        let pressed_c = state.pressed_count.load(Ordering::SeqCst);

        if state.capture.mode.load(Ordering::SeqCst) {
            log::info!("CAPTURE-DIAG: hook saw vk=0x{vk:02X} down={down}");
            if down {
                capture_key(&state.capture, vk);
                let _ = state.event_tx.send(HotkeyEvent::CaptureUpdate {
                    modifiers: state.capture.peak_modifiers.load(Ordering::SeqCst),
                    key_codes: capture_key_codes(&state.capture),
                });
            } else {
                finish_capture(state);
            }
            return;
        }

        let mod_flags = modifier_flags(pressed_p, pressed_c);

        let cancel = state.cancel.load();
        if down
            && (shortcut_matches(&cancel, pressed_p, pressed_c, mod_flags)
                || modifier_shortcut_matches(&cancel, pressed_p, pressed_c))
        {
            let _ = state.event_tx.send(HotkeyEvent::CancelPressed);
            return;
        }

        let rec = state.record.load();
        let matched = shortcut_matches(&rec, pressed_p, pressed_c, mod_flags)
            || modifier_shortcut_matches(&rec, pressed_p, pressed_c);

        if matched && !state.rec_held.load(Ordering::SeqCst) {
            state.rec_held.store(true, Ordering::SeqCst);
            let _ = state.event_tx.send(HotkeyEvent::KeyDown);
        } else if !matched && state.rec_held.load(Ordering::SeqCst) {
            state.rec_held.store(false, Ordering::SeqCst);
            let _ = state.event_tx.send(HotkeyEvent::KeyUp);
        }
    }

    fn capture_key(capture: &CaptureControl, vk: u16) {
        capture.active.store(true, Ordering::SeqCst);
        if is_modifier_key_code(vk) {
            capture.peak_modifiers.fetch_or(
                modifier_flag_for_key_code(vk),
                Ordering::SeqCst,
            );
            packed_add(&capture.mod_keys_packed, &capture.mod_key_count, vk);
        } else {
            packed_add(&capture.keys_packed, &capture.key_count, vk);
        }
    }

    fn capture_key_codes(capture: &CaptureControl) -> Vec<u16> {
        let mut codes = unpack_keys(
            capture.mod_keys_packed.load(Ordering::SeqCst),
            capture.mod_key_count.load(Ordering::SeqCst),
        );
        codes.extend(unpack_keys(
            capture.keys_packed.load(Ordering::SeqCst),
            capture.key_count.load(Ordering::SeqCst),
        ));
        codes
    }

    /// First key release ends capture, matching the macOS backend.
    fn finish_capture(state: &HookState) {
        let capture = &state.capture;
        if !capture.active.load(Ordering::SeqCst) {
            return;
        }
        let key_codes = capture_key_codes(capture);
        let modifiers = capture.peak_modifiers.load(Ordering::SeqCst);
        let regular = capture.key_count.load(Ordering::SeqCst) > 0;

        let kind = if !regular {
            ShortcutKind::ModifierOnly
        } else if modifiers != 0 {
            ShortcutKind::Combo
        } else {
            ShortcutKind::Key
        };

        capture.active.store(false, Ordering::SeqCst);
        capture.mode.store(false, Ordering::SeqCst);
        let _ = state.event_tx.send(HotkeyEvent::CaptureComplete(Shortcut {
            key_codes,
            modifiers,
            kind,
        }));
    }

    unsafe extern "system" fn keyboard_hook(code: i32, wparam: usize, lparam: isize) -> isize {
        if code >= 0 {
            if let Some(state) = STATE.get() {
                // SAFETY: for HC_ACTION the hook contract says lparam points at a
                // KBDLLHOOKSTRUCT owned by the caller for the duration of the call.
                let kb = unsafe { &*(lparam as *const KBDLLHOOKSTRUCT) };
                let msg = wparam as u32;
                let down = msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN;
                on_key(state, kb.vkCode as u16, down);
            }
        }
        // Listen only: never swallow the keystroke.
        unsafe { CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam) }
    }

    pub fn run(
        initial_record: Shortcut,
        initial_cancel: Shortcut,
        event_tx: crossbeam_channel::Sender<HotkeyEvent>,
        update_rx: crossbeam_channel::Receiver<HotkeyUpdate>,
        capture: Arc<CaptureControl>,
    ) {
        let state = HookState {
            record: ShortcutSlot::new(&initial_record),
            rec_held: AtomicBool::new(false),
            cancel: ShortcutSlot::new(&initial_cancel),
            pressed_packed: AtomicU64::new(0),
            pressed_count: AtomicU8::new(0),
            capture,
            event_tx,
        };
        if STATE.set(state).is_err() {
            log::error!("Hotkey monitor already running");
            return;
        }

        // SAFETY: the hook procedure is a plain function and stays valid for the
        // lifetime of the process.
        let hook = unsafe {
            SetWindowsHookExW(
                WH_KEYBOARD_LL,
                Some(keyboard_hook),
                std::ptr::null_mut(),
                0,
            )
        };
        if hook.is_null() {
            log::error!("SetWindowsHookEx failed; the hotkey will not respond");
            return;
        }
        log::info!(
            "Hotkey monitor started (record={}, cancel={})",
            initial_record.display_string(),
            initial_cancel.display_string()
        );

        // A low-level hook is only delivered while its thread pumps messages, so
        // the loop drains the queue rather than blocking in GetMessage — that
        // leaves room to apply shortcut updates between passes.
        let state = STATE.get().expect("state was just set");
        loop {
            let mut msg = MSG::default();
            // SAFETY: msg is a valid, writable MSG for the duration of each call.
            while unsafe { PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_REMOVE) } != 0 {
                unsafe {
                    TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }
            while let Ok(update) = update_rx.try_recv() {
                match update {
                    HotkeyUpdate::SetRecordShortcut(s) => {
                        state.record.store(&s);
                        state.rec_held.store(false, Ordering::SeqCst);
                    }
                    HotkeyUpdate::SetCancelShortcut(s) => state.cancel.store(&s),
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        // Unreachable today; kept so the hook has an owner if the loop ever ends.
        #[allow(unreachable_code)]
        {
            unsafe { UnhookWindowsHookEx(hook) };
        }
    }
}

#[cfg(target_os = "windows")]
pub fn start_monitor(
    initial_record: Shortcut,
    initial_cancel: Shortcut,
    enabled: Arc<AtomicBool>,
    capture: Arc<CaptureControl>,
) -> (
    crossbeam_channel::Receiver<HotkeyEvent>,
    crossbeam_channel::Sender<HotkeyUpdate>,
) {
    let (event_tx, event_rx) = crossbeam_channel::unbounded::<HotkeyEvent>();
    let (update_tx, update_rx) = crossbeam_channel::unbounded::<HotkeyUpdate>();

    std::thread::spawn(move || {
        while !enabled.load(Ordering::SeqCst) {
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
        log::info!("Hotkey monitoring enabled, installing keyboard hook");
        win::run(initial_record, initial_cancel, event_tx, update_rx, capture);
    });

    (event_rx, update_tx)
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub fn start_monitor(
    _initial_record: Shortcut,
    _initial_cancel: Shortcut,
    _enabled: Arc<AtomicBool>,
    _capture: Arc<CaptureControl>,
) -> (
    crossbeam_channel::Receiver<HotkeyEvent>,
    crossbeam_channel::Sender<HotkeyUpdate>,
) {
    let (_event_tx, event_rx) = crossbeam_channel::unbounded();
    let (update_tx, _update_rx) = crossbeam_channel::unbounded();
    log::warn!("Hotkey monitoring not implemented on this platform");
    (event_rx, update_tx)
}

