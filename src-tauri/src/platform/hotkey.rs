//! Global hotkey monitoring via CGEvent tap (macOS).
//! Supports three shortcut kinds:
//! - ModifierOnly: single modifier key (e.g. Right Command)
//! - Combo: modifier(s) + regular key (e.g. Cmd+R, Ctrl+Space)
//! - Key: standalone key without modifiers (e.g. F13, F14)

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

// -- CGEventFlags masks --

const CG_MASK_COMMAND: u64 = 1 << 20;
const CG_MASK_SHIFT: u64 = 1 << 17;
const CG_MASK_ALTERNATE: u64 = 1 << 19;
const CG_MASK_CONTROL: u64 = 1 << 18;
// Combined mask for all 4 modifier bits
const CG_MASK_ALL_MODIFIERS: u64 = CG_MASK_COMMAND | CG_MASK_SHIFT | CG_MASK_ALTERNATE | CG_MASK_CONTROL;

// -- Shortcut types --

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum ShortcutKind {
    ModifierOnly,
    Combo,
    Key,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Shortcut {
    pub key_code: u16,
    pub modifiers: u64,
    pub kind: ShortcutKind,
}

impl Shortcut {
    /// A disabled shortcut (won't match anything).
    pub fn disabled() -> Self {
        Self { key_code: 0, modifiers: 0, kind: ShortcutKind::Key }
    }

    pub fn is_disabled(&self) -> bool {
        self.key_code == 0 && self.modifiers == 0
    }

    /// Parse from a string — tries JSON first, then legacy format.
    pub fn parse(s: &str) -> Self {
        // Try JSON
        if let Ok(shortcut) = serde_json::from_str::<Shortcut>(s) {
            return shortcut;
        }
        // Legacy format
        match s {
            "right_command" => Self {
                key_code: 0x36,
                modifiers: CG_MASK_COMMAND,
                kind: ShortcutKind::ModifierOnly,
            },
            "right_option" => Self {
                key_code: 0x3D,
                modifiers: CG_MASK_ALTERNATE,
                kind: ShortcutKind::ModifierOnly,
            },
            "right_control" => Self {
                key_code: 0x3E,
                modifiers: CG_MASK_CONTROL,
                kind: ShortcutKind::ModifierOnly,
            },
            "right_shift" => Self {
                key_code: 0x3C,
                modifiers: CG_MASK_SHIFT,
                kind: ShortcutKind::ModifierOnly,
            },
            "escape" => Self {
                key_code: 0x35,
                modifiers: 0,
                kind: ShortcutKind::Key,
            },
            "none" | "" => Self::disabled(),
            _ => Self {
                key_code: 0x36,
                modifiers: CG_MASK_COMMAND,
                kind: ShortcutKind::ModifierOnly,
            },
        }
    }

    /// Serialize to JSON string for storage.
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    /// Human-readable display string (e.g. "⌃⌥⇧⌘R", "Right ⌘", "F13").
    pub fn display_string(&self) -> String {
        if self.is_disabled() {
            return String::new();
        }
        match self.kind {
            ShortcutKind::ModifierOnly => {
                // Show the specific modifier key name
                modifier_only_label(self.key_code).to_string()
            }
            ShortcutKind::Combo => {
                let mut s = modifier_symbols(self.modifiers);
                s.push_str(key_code_label(self.key_code));
                s
            }
            ShortcutKind::Key => {
                key_code_label(self.key_code).to_string()
            }
        }
    }
}

/// Modifier symbols in macOS standard order: ⌃⌥⇧⌘
fn modifier_symbols(flags: u64) -> String {
    let mut s = String::new();
    if flags & CG_MASK_CONTROL != 0 { s.push('⌃'); }
    if flags & CG_MASK_ALTERNATE != 0 { s.push('⌥'); }
    if flags & CG_MASK_SHIFT != 0 { s.push('⇧'); }
    if flags & CG_MASK_COMMAND != 0 { s.push('⌘'); }
    s
}

/// Label for a modifier-only key code (Right/Left Cmd/Opt/Ctrl/Shift).
fn modifier_only_label(key_code: u16) -> &'static str {
    match key_code {
        0x36 => "Right ⌘",
        0x37 => "Left ⌘",
        0x3D => "Right ⌥",
        0x3A => "Left ⌥",
        0x3E => "Right ⌃",
        0x3B => "Left ⌃",
        0x3C => "Right ⇧",
        0x38 => "Left ⇧",
        0x3F => "Fn",
        _ => "⌘",
    }
}

/// Label for a regular (non-modifier) key code.
fn key_code_label(key_code: u16) -> &'static str {
    match key_code {
        // Letters (QWERTY layout key codes)
        0x00 => "A", 0x0B => "B", 0x08 => "C", 0x02 => "D",
        0x0E => "E", 0x03 => "F", 0x05 => "G", 0x04 => "H",
        0x22 => "I", 0x26 => "J", 0x28 => "K", 0x25 => "L",
        0x2E => "M", 0x2D => "N", 0x1F => "O", 0x23 => "P",
        0x0C => "Q", 0x0F => "R", 0x01 => "S", 0x11 => "T",
        0x20 => "U", 0x09 => "V", 0x0D => "W", 0x07 => "X",
        0x10 => "Y", 0x06 => "Z",
        // Numbers
        0x12 => "1", 0x13 => "2", 0x14 => "3", 0x15 => "4",
        0x17 => "5", 0x16 => "6", 0x1A => "7", 0x1C => "8",
        0x19 => "9", 0x1D => "0",
        // F-keys
        0x7A => "F1", 0x78 => "F2", 0x63 => "F3", 0x76 => "F4",
        0x60 => "F5", 0x61 => "F6", 0x62 => "F7", 0x64 => "F8",
        0x65 => "F9", 0x6D => "F10", 0x67 => "F11", 0x6F => "F12",
        0x69 => "F13", 0x6B => "F14", 0x71 => "F15", 0x6A => "F16",
        0x40 => "F17", 0x4F => "F18", 0x50 => "F19", 0x5A => "F20",
        // Special keys
        0x31 => "Space", 0x24 => "Return", 0x30 => "Tab",
        0x33 => "Delete", 0x75 => "Fwd Delete", 0x35 => "Escape",
        // Arrow keys
        0x7B => "←", 0x7C => "→", 0x7E => "↑", 0x7D => "↓",
        // Navigation
        0x73 => "Home", 0x77 => "End", 0x74 => "Page Up", 0x79 => "Page Down",
        // Punctuation
        0x1B => "-", 0x18 => "=", 0x21 => "[", 0x1E => "]",
        0x2A => "\\", 0x29 => ";", 0x27 => "'", 0x2B => ",",
        0x2F => ".", 0x2C => "/", 0x32 => "`",
        // Keypad
        0x52 => "Pad 0", 0x53 => "Pad 1", 0x54 => "Pad 2",
        0x55 => "Pad 3", 0x56 => "Pad 4", 0x57 => "Pad 5",
        0x58 => "Pad 6", 0x59 => "Pad 7", 0x5B => "Pad 8",
        0x5C => "Pad 9", 0x45 => "Pad +", 0x4E => "Pad -",
        0x43 => "Pad *", 0x4B => "Pad /", 0x41 => "Pad .",
        0x4C => "Pad Enter", 0x51 => "Pad =",
        // Modifier keys (when used as regular key codes in ModifierOnly)
        0x36 => "Right ⌘", 0x37 => "Left ⌘",
        0x3D => "Right ⌥", 0x3A => "Left ⌥",
        0x3E => "Right ⌃", 0x3B => "Left ⌃",
        0x3C => "Right ⇧", 0x38 => "Left ⇧",
        0x3F => "Fn",
        _ => "?",
    }
}

/// Check if a key_code is a modifier key (not a regular key).
fn is_modifier_key_code(key_code: u16) -> bool {
    matches!(key_code, 0x36 | 0x37 | 0x3A | 0x3B | 0x3C | 0x3D | 0x3E | 0x38 | 0x3F)
}

// -- Events --

#[derive(Debug, Clone)]
pub enum HotkeyEvent {
    KeyDown,
    KeyUp,
    CancelPressed,
    CaptureUpdate { modifiers: u64, key_code: Option<u16> },
    CaptureComplete(Shortcut),
    CaptureCancelled,
}

/// Messages to update hotkey configuration at runtime.
pub enum HotkeyUpdate {
    SetRecordShortcut(Shortcut),
    SetCancelShortcut(Shortcut),
    EnterCaptureMode,
    ExitCaptureMode,
}

// -- Monitor --

/// Start monitoring the hotkey on a background thread.
/// Returns a receiver for hotkey events and a sender to update config.
#[cfg(target_os = "macos")]
pub fn start_monitor(
    initial_record: Shortcut,
    initial_cancel: Shortcut,
    enabled: Arc<AtomicBool>,
) -> (crossbeam_channel::Receiver<HotkeyEvent>, crossbeam_channel::Sender<HotkeyUpdate>) {
    let (event_tx, event_rx) = crossbeam_channel::unbounded::<HotkeyEvent>();
    let (update_tx, update_rx) = crossbeam_channel::unbounded::<HotkeyUpdate>();

    std::thread::spawn(move || {
        while !enabled.load(Ordering::SeqCst) {
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
        log::info!("Hotkey monitoring enabled, starting event tap");
        run_event_tap(initial_record, initial_cancel, event_tx, update_rx);
    });

    (event_rx, update_tx)
}

/// Shared state between the CGEvent callback and the main hotkey thread.
#[cfg(target_os = "macos")]
struct TapState {
    // Record shortcut (atomics for lock-free callback access)
    rec_key_code: std::sync::atomic::AtomicU16,
    rec_modifiers: std::sync::atomic::AtomicU64,
    rec_kind: std::sync::atomic::AtomicU8, // 0=ModifierOnly, 1=Combo, 2=Key
    rec_held: AtomicBool,

    // Cancel shortcut
    cancel_key_code: std::sync::atomic::AtomicU16,
    cancel_modifiers: std::sync::atomic::AtomicU64,
    cancel_kind: std::sync::atomic::AtomicU8,

    // Capture mode
    capture_mode: AtomicBool,
    capture_modifiers: std::sync::atomic::AtomicU64,
    capture_key: std::sync::atomic::AtomicU16,
    capture_had_key: AtomicBool,
    capture_active: AtomicBool, // any key/modifier is currently held

    event_tx: crossbeam_channel::Sender<HotkeyEvent>,
}

#[cfg(target_os = "macos")]
impl TapState {
    fn kind_to_u8(kind: ShortcutKind) -> u8 {
        match kind {
            ShortcutKind::ModifierOnly => 0,
            ShortcutKind::Combo => 1,
            ShortcutKind::Key => 2,
        }
    }

    fn u8_to_kind(v: u8) -> ShortcutKind {
        match v {
            0 => ShortcutKind::ModifierOnly,
            1 => ShortcutKind::Combo,
            _ => ShortcutKind::Key,
        }
    }

    fn load_rec_shortcut(&self) -> Shortcut {
        Shortcut {
            key_code: self.rec_key_code.load(Ordering::SeqCst),
            modifiers: self.rec_modifiers.load(Ordering::SeqCst),
            kind: Self::u8_to_kind(self.rec_kind.load(Ordering::SeqCst)),
        }
    }

    fn store_rec_shortcut(&self, s: &Shortcut) {
        self.rec_key_code.store(s.key_code, Ordering::SeqCst);
        self.rec_modifiers.store(s.modifiers, Ordering::SeqCst);
        self.rec_kind.store(Self::kind_to_u8(s.kind), Ordering::SeqCst);
        self.rec_held.store(false, Ordering::SeqCst);
    }

    fn load_cancel_shortcut(&self) -> Shortcut {
        Shortcut {
            key_code: self.cancel_key_code.load(Ordering::SeqCst),
            modifiers: self.cancel_modifiers.load(Ordering::SeqCst),
            kind: Self::u8_to_kind(self.cancel_kind.load(Ordering::SeqCst)),
        }
    }

    fn store_cancel_shortcut(&self, s: &Shortcut) {
        self.cancel_key_code.store(s.key_code, Ordering::SeqCst);
        self.cancel_modifiers.store(s.modifiers, Ordering::SeqCst);
        self.cancel_kind.store(Self::kind_to_u8(s.kind), Ordering::SeqCst);
    }
}

#[cfg(target_os = "macos")]
fn run_event_tap(
    initial_record: Shortcut,
    initial_cancel: Shortcut,
    event_tx: crossbeam_channel::Sender<HotkeyEvent>,
    update_rx: crossbeam_channel::Receiver<HotkeyUpdate>,
) {
    use std::os::raw::c_void;
    use std::sync::atomic::{AtomicU16, AtomicU64};

    let state = Box::new(TapState {
        rec_key_code: AtomicU16::new(initial_record.key_code),
        rec_modifiers: AtomicU64::new(initial_record.modifiers),
        rec_kind: std::sync::atomic::AtomicU8::new(TapState::kind_to_u8(initial_record.kind)),
        rec_held: AtomicBool::new(false),

        cancel_key_code: AtomicU16::new(initial_cancel.key_code),
        cancel_modifiers: AtomicU64::new(initial_cancel.modifiers),
        cancel_kind: std::sync::atomic::AtomicU8::new(TapState::kind_to_u8(initial_cancel.kind)),

        capture_mode: AtomicBool::new(false),
        capture_modifiers: AtomicU64::new(0),
        capture_key: AtomicU16::new(0),
        capture_had_key: AtomicBool::new(false),
        capture_active: AtomicBool::new(false),

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
            log::warn!("CGEvent tap disabled (type={}), will re-enable", event_type);
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
            if state.capture_mode.load(Ordering::SeqCst) {
                handle_capture(state, event_type, key_code, mod_flags);
                return event;
            }

            // -- Normal mode --
            handle_normal(state, event_type, key_code, mod_flags);
        }

        event
    }

    /// Handle events in capture mode.
    #[cfg(target_os = "macos")]
    unsafe fn handle_capture(state: &TapState, event_type: u32, key_code: u16, mod_flags: u64) {
        const KEY_DOWN: u32 = 10;
        const KEY_UP: u32 = 11;
        const FLAGS_CHANGED: u32 = 12;

        match event_type {
            KEY_DOWN => {
                // Escape cancels capture
                if key_code == 0x35 && mod_flags == 0 {
                    let _ = state.event_tx.send(HotkeyEvent::CaptureCancelled);
                    return;
                }

                if !is_modifier_key_code(key_code) {
                    state.capture_key.store(key_code, Ordering::SeqCst);
                    state.capture_had_key.store(true, Ordering::SeqCst);
                    state.capture_active.store(true, Ordering::SeqCst);

                    let _ = state.event_tx.send(HotkeyEvent::CaptureUpdate {
                        modifiers: mod_flags,
                        key_code: Some(key_code),
                    });
                }
            }
            KEY_UP => {
                if !is_modifier_key_code(key_code) && state.capture_had_key.load(Ordering::SeqCst) {
                    let captured_key = state.capture_key.load(Ordering::SeqCst);
                    let captured_mods = state.capture_modifiers.load(Ordering::SeqCst);

                    let kind = if captured_mods != 0 {
                        ShortcutKind::Combo
                    } else {
                        ShortcutKind::Key
                    };

                    let shortcut = Shortcut {
                        key_code: captured_key,
                        modifiers: captured_mods,
                        kind,
                    };

                    // Reset capture state
                    state.capture_key.store(0, Ordering::SeqCst);
                    state.capture_modifiers.store(0, Ordering::SeqCst);
                    state.capture_had_key.store(false, Ordering::SeqCst);
                    state.capture_active.store(false, Ordering::SeqCst);

                    let _ = state.event_tx.send(HotkeyEvent::CaptureComplete(shortcut));
                }
            }
            FLAGS_CHANGED => {
                state.capture_modifiers.store(mod_flags, Ordering::SeqCst);

                if mod_flags != 0 {
                    state.capture_active.store(true, Ordering::SeqCst);
                    let cap_key = state.capture_key.load(Ordering::SeqCst);
                    let _ = state.event_tx.send(HotkeyEvent::CaptureUpdate {
                        modifiers: mod_flags,
                        key_code: if state.capture_had_key.load(Ordering::SeqCst) { Some(cap_key) } else { None },
                    });
                } else if state.capture_active.load(Ordering::SeqCst) {
                    // All modifiers released
                    if !state.capture_had_key.load(Ordering::SeqCst) {
                        // ModifierOnly: we need to figure out which modifier was released
                        // The key_code in flagsChanged tells us which modifier changed
                        let shortcut = Shortcut {
                            key_code,
                            modifiers: modifier_flag_for_key_code(key_code),
                            kind: ShortcutKind::ModifierOnly,
                        };

                        state.capture_key.store(0, Ordering::SeqCst);
                        state.capture_modifiers.store(0, Ordering::SeqCst);
                        state.capture_active.store(false, Ordering::SeqCst);

                        let _ = state.event_tx.send(HotkeyEvent::CaptureComplete(shortcut));
                    }
                    // If had_key, we already sent CaptureComplete on KeyUp
                }
            }
            _ => {}
        }
    }

    /// Handle events in normal mode (record/cancel shortcuts).
    #[cfg(target_os = "macos")]
    unsafe fn handle_normal(state: &TapState, event_type: u32, key_code: u16, mod_flags: u64) {
        const KEY_DOWN: u32 = 10;
        const KEY_UP: u32 = 11;
        const FLAGS_CHANGED: u32 = 12;

        let rec = state.load_rec_shortcut();
        let cancel = state.load_cancel_shortcut();

        match event_type {
            KEY_DOWN => {
                // Check cancel shortcut
                if !cancel.is_disabled() {
                    match cancel.kind {
                        ShortcutKind::Combo => {
                            if key_code == cancel.key_code && (mod_flags & cancel.modifiers) == cancel.modifiers {
                                let _ = state.event_tx.send(HotkeyEvent::CancelPressed);
                                return;
                            }
                        }
                        ShortcutKind::Key => {
                            if key_code == cancel.key_code && mod_flags == 0 {
                                let _ = state.event_tx.send(HotkeyEvent::CancelPressed);
                                return;
                            }
                        }
                        _ => {}
                    }
                }

                // Check record shortcut
                if !rec.is_disabled() {
                    match rec.kind {
                        ShortcutKind::Combo => {
                            if key_code == rec.key_code && (mod_flags & rec.modifiers) == rec.modifiers {
                                if !state.rec_held.load(Ordering::SeqCst) {
                                    state.rec_held.store(true, Ordering::SeqCst);
                                    log::debug!("Hotkey KeyDown (Combo): key=0x{:02x} mods=0x{:x}", key_code, mod_flags);
                                    let _ = state.event_tx.send(HotkeyEvent::KeyDown);
                                }
                            }
                        }
                        ShortcutKind::Key => {
                            if key_code == rec.key_code && mod_flags == 0 {
                                if !state.rec_held.load(Ordering::SeqCst) {
                                    state.rec_held.store(true, Ordering::SeqCst);
                                    log::debug!("Hotkey KeyDown (Key): key=0x{:02x}", key_code);
                                    let _ = state.event_tx.send(HotkeyEvent::KeyDown);
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            KEY_UP => {
                if !rec.is_disabled() && state.rec_held.load(Ordering::SeqCst) {
                    match rec.kind {
                        ShortcutKind::Combo | ShortcutKind::Key => {
                            if key_code == rec.key_code {
                                state.rec_held.store(false, Ordering::SeqCst);
                                log::debug!("Hotkey KeyUp: key=0x{:02x}", key_code);
                                let _ = state.event_tx.send(HotkeyEvent::KeyUp);
                            }
                        }
                        _ => {}
                    }
                }
            }
            FLAGS_CHANGED => {
                if rec.is_disabled() {
                    return;
                }

                match rec.kind {
                    ShortcutKind::ModifierOnly => {
                        if key_code == rec.key_code {
                            if (mod_flags & rec.modifiers) != 0 {
                                if !state.rec_held.load(Ordering::SeqCst) {
                                    state.rec_held.store(true, Ordering::SeqCst);
                                    log::debug!("Hotkey KeyDown (ModifierOnly): key=0x{:02x} flags=0x{:x}", key_code, mod_flags);
                                    let _ = state.event_tx.send(HotkeyEvent::KeyDown);
                                }
                            } else if state.rec_held.load(Ordering::SeqCst) {
                                state.rec_held.store(false, Ordering::SeqCst);
                                log::debug!("Hotkey KeyUp (ModifierOnly): key=0x{:02x} flags=0x{:x}", key_code, mod_flags);
                                let _ = state.event_tx.send(HotkeyEvent::KeyUp);
                            }
                        }
                    }
                    ShortcutKind::Combo => {
                        // If the modifier was released while holding a combo, send KeyUp
                        if state.rec_held.load(Ordering::SeqCst) {
                            if (mod_flags & rec.modifiers) != rec.modifiers {
                                state.rec_held.store(false, Ordering::SeqCst);
                                log::debug!("Hotkey KeyUp (Combo modifier released): flags=0x{:x}", mod_flags);
                                let _ = state.event_tx.send(HotkeyEvent::KeyUp);
                            }
                        }
                    }
                    _ => {}
                }

                // Cancel shortcut (ModifierOnly)
                if !cancel.is_disabled() && cancel.kind == ShortcutKind::ModifierOnly {
                    if key_code == cancel.key_code {
                        // Detect press (flags set) then release (flags cleared)
                        // For cancel we only need the press event
                        if (mod_flags & cancel.modifiers) != 0 {
                            // We could track state, but for cancel a simple press is enough
                        } else {
                            // Modifier released — this could be a cancel tap
                            // But this gets complex; cancel with ModifierOnly is unusual.
                            // For now, send CancelPressed on modifier press.
                        }
                    }
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
        use std::os::raw::c_void;
        use super::ffi;

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

        log::info!("Hotkey monitor started (record={}, cancel={})",
            initial_record.display_string(),
            initial_cancel.display_string());

        let state = &*state_ptr;
        loop {
            ffi::CFRunLoopRunInMode(ffi::kCFRunLoopDefaultMode, 0.5, false);
            ffi::CGEventTapEnable(tap, true);

            while let Ok(update) = update_rx.try_recv() {
                match update {
                    HotkeyUpdate::SetRecordShortcut(s) => {
                        log::info!("Record shortcut changed to {}", s.display_string());
                        state.store_rec_shortcut(&s);
                    }
                    HotkeyUpdate::SetCancelShortcut(s) => {
                        log::info!("Cancel shortcut changed to {}", s.display_string());
                        state.store_cancel_shortcut(&s);
                    }
                    HotkeyUpdate::EnterCaptureMode => {
                        log::info!("Entering shortcut capture mode");
                        state.capture_mode.store(true, Ordering::SeqCst);
                        state.capture_modifiers.store(0, Ordering::SeqCst);
                        state.capture_key.store(0, Ordering::SeqCst);
                        state.capture_had_key.store(false, Ordering::SeqCst);
                        state.capture_active.store(false, Ordering::SeqCst);
                    }
                    HotkeyUpdate::ExitCaptureMode => {
                        log::info!("Exiting shortcut capture mode");
                        state.capture_mode.store(false, Ordering::SeqCst);
                    }
                }
            }
        }
    }
}

/// Get the modifier flag for a specific modifier key code.
fn modifier_flag_for_key_code(key_code: u16) -> u64 {
    match key_code {
        0x36 | 0x37 => CG_MASK_COMMAND,
        0x3A | 0x3D => CG_MASK_ALTERNATE,
        0x3B | 0x3E => CG_MASK_CONTROL,
        0x38 | 0x3C => CG_MASK_SHIFT,
        _ => 0,
    }
}

#[cfg(not(target_os = "macos"))]
pub fn start_monitor(
    _initial_record: Shortcut,
    _initial_cancel: Shortcut,
    _enabled: Arc<AtomicBool>,
) -> (crossbeam_channel::Receiver<HotkeyEvent>, crossbeam_channel::Sender<HotkeyUpdate>) {
    let (_event_tx, event_rx) = crossbeam_channel::unbounded();
    let (update_tx, _update_rx) = crossbeam_channel::unbounded();
    log::warn!("Hotkey monitoring not implemented on this platform");
    (event_rx, update_tx)
}
