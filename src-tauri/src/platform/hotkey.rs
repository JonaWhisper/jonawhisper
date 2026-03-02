//! Global hotkey monitoring via CGEvent tap (macOS).
//! Supports three shortcut kinds:
//! - ModifierOnly: modifier key(s) only (e.g. Right Command, Right ⌘ + Left ⌥)
//! - Combo: modifier(s) + regular key(s) (e.g. Cmd+R, Ctrl+Space, Cmd+A+B)
//! - Key: standalone key(s) without modifiers (e.g. F13, Escape)
//!
//! Multi-key shortcuts: during capture, keys accumulate until the first key-up
//! finalises the shortcut.  In normal mode, the shortcut fires when ALL required
//! keys are held simultaneously.

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering};
use std::sync::Arc;

const MAX_SHORTCUT_KEYS: usize = 4;

// -- CGEventFlags masks --

const CG_MASK_COMMAND: u64 = 1 << 20;
const CG_MASK_SHIFT: u64 = 1 << 17;
const CG_MASK_ALTERNATE: u64 = 1 << 19;
const CG_MASK_CONTROL: u64 = 1 << 18;
const CG_MASK_ALL_MODIFIERS: u64 =
    CG_MASK_COMMAND | CG_MASK_SHIFT | CG_MASK_ALTERNATE | CG_MASK_CONTROL;

// -- Shortcut types --

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum ShortcutKind {
    ModifierOnly,
    Combo,
    Key,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Shortcut {
    pub key_codes: Vec<u16>,
    pub modifiers: u64,
    pub kind: ShortcutKind,
}

impl Shortcut {
    /// A disabled shortcut (won't match anything).
    pub fn disabled() -> Self {
        Self {
            key_codes: vec![],
            modifiers: 0,
            kind: ShortcutKind::Key,
        }
    }

    pub fn is_disabled(&self) -> bool {
        self.key_codes.is_empty() && self.modifiers == 0
    }

    /// Parse from a string — tries new JSON, old JSON, then legacy format.
    pub fn parse(s: &str) -> Self {
        // New JSON format: { "key_codes": [...], "modifiers": ..., "kind": ... }
        if let Ok(shortcut) = serde_json::from_str::<Shortcut>(s) {
            return shortcut;
        }
        // Old JSON format: { "key_code": ..., "modifiers": ..., "kind": ... }
        #[derive(Deserialize)]
        struct OldShortcut {
            key_code: u16,
            modifiers: u64,
            kind: ShortcutKind,
        }
        if let Ok(old) = serde_json::from_str::<OldShortcut>(s) {
            let key_codes = if old.key_code == 0 && old.modifiers == 0 {
                vec![]
            } else {
                vec![old.key_code]
            };
            return Shortcut {
                key_codes,
                modifiers: old.modifiers,
                kind: old.kind,
            };
        }
        // Legacy string format
        match s {
            "right_command" => Self {
                key_codes: vec![0x36],
                modifiers: CG_MASK_COMMAND,
                kind: ShortcutKind::ModifierOnly,
            },
            "right_option" => Self {
                key_codes: vec![0x3D],
                modifiers: CG_MASK_ALTERNATE,
                kind: ShortcutKind::ModifierOnly,
            },
            "right_control" => Self {
                key_codes: vec![0x3E],
                modifiers: CG_MASK_CONTROL,
                kind: ShortcutKind::ModifierOnly,
            },
            "right_shift" => Self {
                key_codes: vec![0x3C],
                modifiers: CG_MASK_SHIFT,
                kind: ShortcutKind::ModifierOnly,
            },
            "escape" => Self {
                key_codes: vec![0x35],
                modifiers: 0,
                kind: ShortcutKind::Key,
            },
            "none" | "" => Self::disabled(),
            _ => Self {
                key_codes: vec![0x36],
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
                self.key_codes
                    .iter()
                    .map(|&kc| modifier_only_label(kc))
                    .collect::<Vec<_>>()
                    .join("+")
            }
            ShortcutKind::Combo => {
                let mut s = modifier_symbols(self.modifiers);
                for &kc in &self.key_codes {
                    s.push_str(key_code_label(kc));
                }
                s
            }
            ShortcutKind::Key => self
                .key_codes
                .iter()
                .map(|&kc| key_code_label(kc))
                .collect::<Vec<_>>()
                .join("+"),
        }
    }
}

/// Modifier symbols in macOS standard order: ⌃⌥⇧⌘
fn modifier_symbols(flags: u64) -> String {
    let mut s = String::new();
    if flags & CG_MASK_CONTROL != 0 {
        s.push('⌃');
    }
    if flags & CG_MASK_ALTERNATE != 0 {
        s.push('⌥');
    }
    if flags & CG_MASK_SHIFT != 0 {
        s.push('⇧');
    }
    if flags & CG_MASK_COMMAND != 0 {
        s.push('⌘');
    }
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
    matches!(
        key_code,
        0x36 | 0x37 | 0x3A | 0x3B | 0x3C | 0x3D | 0x3E | 0x38 | 0x3F
    )
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

// -- Packed key helpers --
// Pack up to 4 u16 key codes into a single u64 for lock-free atomic access.
// The CGEvent callback is single-threaded, so load-modify-store is safe.

fn pack_keys(keys: &[u16]) -> (u64, u8) {
    let count = keys.len().min(MAX_SHORTCUT_KEYS);
    let mut packed: u64 = 0;
    for (i, &k) in keys[..count].iter().enumerate() {
        packed |= (k as u64) << (i * 16);
    }
    (packed, count as u8)
}

fn unpack_keys(packed: u64, count: u8) -> Vec<u16> {
    (0..count as usize)
        .map(|i| ((packed >> (i * 16)) & 0xFFFF) as u16)
        .collect()
}

fn packed_contains(packed: u64, count: u8, key: u16) -> bool {
    for i in 0..count as usize {
        if ((packed >> (i * 16)) & 0xFFFF) as u16 == key {
            return true;
        }
    }
    false
}

/// Add a key to a packed set (deduplicated). Returns true if added.
fn packed_add(packed: &AtomicU64, count: &AtomicU8, key: u16) -> bool {
    let p = packed.load(Ordering::SeqCst);
    let c = count.load(Ordering::SeqCst) as usize;
    if packed_contains(p, c as u8, key) {
        return false;
    }
    if c >= MAX_SHORTCUT_KEYS {
        return false;
    }
    packed.store(p | ((key as u64) << (c * 16)), Ordering::SeqCst);
    count.store((c + 1) as u8, Ordering::SeqCst);
    true
}

/// Remove a key from a packed set. Returns true if removed.
fn packed_remove(packed: &AtomicU64, count: &AtomicU8, key: u16) -> bool {
    let p = packed.load(Ordering::SeqCst);
    let c = count.load(Ordering::SeqCst) as usize;
    let mut idx = None;
    for i in 0..c {
        if ((p >> (i * 16)) & 0xFFFF) as u16 == key {
            idx = Some(i);
            break;
        }
    }
    let i = match idx {
        Some(i) => i,
        None => return false,
    };
    let mut new: u64 = 0;
    let mut j = 0;
    for k in 0..c {
        if k != i {
            let v = ((p >> (k * 16)) & 0xFFFF) as u16;
            new |= (v as u64) << (j * 16);
            j += 1;
        }
    }
    packed.store(new, Ordering::SeqCst);
    count.store((c - 1) as u8, Ordering::SeqCst);
    true
}

/// Check if all keys in `need` are present in `have`.
fn packed_contains_all(
    have_packed: u64,
    have_count: u8,
    need_packed: u64,
    need_count: u8,
) -> bool {
    if need_count == 0 {
        return false;
    }
    for i in 0..need_count as usize {
        let key = ((need_packed >> (i * 16)) & 0xFFFF) as u16;
        if !packed_contains(have_packed, have_count, key) {
            return false;
        }
    }
    true
}

// -- Capture control --

/// Shared capture-mode state, accessible from both the CGEvent callback thread
/// and the Tauri command handler.
pub struct CaptureControl {
    pub mode: AtomicBool,
    /// Cumulative OR of modifier flags (never reduced during capture).
    pub peak_modifiers: AtomicU64,
    /// Accumulated regular key codes (packed 4×u16).
    pub keys_packed: AtomicU64,
    pub key_count: AtomicU8,
    /// Accumulated modifier key codes (packed 4×u16).
    pub mod_keys_packed: AtomicU64,
    pub mod_key_count: AtomicU8,
    pub active: AtomicBool,
}

impl CaptureControl {
    pub fn new() -> Self {
        Self {
            mode: AtomicBool::new(false),
            peak_modifiers: AtomicU64::new(0),
            keys_packed: AtomicU64::new(0),
            key_count: AtomicU8::new(0),
            mod_keys_packed: AtomicU64::new(0),
            mod_key_count: AtomicU8::new(0),
            active: AtomicBool::new(false),
        }
    }

    /// Enter capture mode: reset fields then set mode=true.
    pub fn enter(&self) {
        self.peak_modifiers.store(0, Ordering::SeqCst);
        self.keys_packed.store(0, Ordering::SeqCst);
        self.key_count.store(0, Ordering::SeqCst);
        self.mod_keys_packed.store(0, Ordering::SeqCst);
        self.mod_key_count.store(0, Ordering::SeqCst);
        self.active.store(false, Ordering::SeqCst);
        self.mode.store(true, Ordering::SeqCst);
        log::info!("Entering shortcut capture mode");
    }

    /// Exit capture mode: set mode=false then reset fields.
    pub fn exit(&self) {
        self.mode.store(false, Ordering::SeqCst);
        self.peak_modifiers.store(0, Ordering::SeqCst);
        self.keys_packed.store(0, Ordering::SeqCst);
        self.key_count.store(0, Ordering::SeqCst);
        self.mod_keys_packed.store(0, Ordering::SeqCst);
        self.mod_key_count.store(0, Ordering::SeqCst);
        self.active.store(false, Ordering::SeqCst);
        log::info!("Exiting shortcut capture mode");
    }

    fn reset(&self) {
        self.peak_modifiers.store(0, Ordering::SeqCst);
        self.keys_packed.store(0, Ordering::SeqCst);
        self.key_count.store(0, Ordering::SeqCst);
        self.mod_keys_packed.store(0, Ordering::SeqCst);
        self.mod_key_count.store(0, Ordering::SeqCst);
        self.active.store(false, Ordering::SeqCst);
    }
}

// -- Events --

#[derive(Debug, Clone)]
pub enum HotkeyEvent {
    KeyDown,
    KeyUp,
    CancelPressed,
    CaptureUpdate {
        modifiers: u64,
        key_codes: Vec<u16>,
    },
    CaptureComplete(Shortcut),
}

/// Messages to update hotkey configuration at runtime.
pub enum HotkeyUpdate {
    SetRecordShortcut(Shortcut),
    SetCancelShortcut(Shortcut),
}

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
    // Record shortcut (atomics for lock-free callback access)
    rec_keys_packed: AtomicU64,
    rec_key_count: AtomicU8,
    rec_modifiers: AtomicU64,
    rec_kind: AtomicU8, // 0=ModifierOnly, 1=Combo, 2=Key
    rec_held: AtomicBool,

    // Cancel shortcut
    cancel_keys_packed: AtomicU64,
    cancel_key_count: AtomicU8,
    cancel_modifiers: AtomicU64,
    cancel_kind: AtomicU8,

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
        let packed = self.rec_keys_packed.load(Ordering::SeqCst);
        let count = self.rec_key_count.load(Ordering::SeqCst);
        Shortcut {
            key_codes: unpack_keys(packed, count),
            modifiers: self.rec_modifiers.load(Ordering::SeqCst),
            kind: Self::u8_to_kind(self.rec_kind.load(Ordering::SeqCst)),
        }
    }

    fn store_rec_shortcut(&self, s: &Shortcut) {
        let (packed, count) = pack_keys(&s.key_codes);
        self.rec_keys_packed.store(packed, Ordering::SeqCst);
        self.rec_key_count.store(count, Ordering::SeqCst);
        self.rec_modifiers.store(s.modifiers, Ordering::SeqCst);
        self.rec_kind
            .store(Self::kind_to_u8(s.kind), Ordering::SeqCst);
        self.rec_held.store(false, Ordering::SeqCst);
    }

    fn load_cancel_shortcut(&self) -> Shortcut {
        let packed = self.cancel_keys_packed.load(Ordering::SeqCst);
        let count = self.cancel_key_count.load(Ordering::SeqCst);
        Shortcut {
            key_codes: unpack_keys(packed, count),
            modifiers: self.cancel_modifiers.load(Ordering::SeqCst),
            kind: Self::u8_to_kind(self.cancel_kind.load(Ordering::SeqCst)),
        }
    }

    fn store_cancel_shortcut(&self, s: &Shortcut) {
        let (packed, count) = pack_keys(&s.key_codes);
        self.cancel_keys_packed.store(packed, Ordering::SeqCst);
        self.cancel_key_count.store(count, Ordering::SeqCst);
        self.cancel_modifiers.store(s.modifiers, Ordering::SeqCst);
        self.cancel_kind
            .store(Self::kind_to_u8(s.kind), Ordering::SeqCst);
    }
}

#[cfg(target_os = "macos")]
fn run_event_tap(
    initial_record: Shortcut,
    initial_cancel: Shortcut,
    event_tx: crossbeam_channel::Sender<HotkeyEvent>,
    update_rx: crossbeam_channel::Receiver<HotkeyUpdate>,
    capture: Arc<CaptureControl>,
) {
    use std::os::raw::c_void;

    let (rec_packed, rec_count) = pack_keys(&initial_record.key_codes);
    let (cancel_packed, cancel_count) = pack_keys(&initial_cancel.key_codes);

    let state = Box::new(TapState {
        rec_keys_packed: AtomicU64::new(rec_packed),
        rec_key_count: AtomicU8::new(rec_count),
        rec_modifiers: AtomicU64::new(initial_record.modifiers),
        rec_kind: AtomicU8::new(TapState::kind_to_u8(initial_record.kind)),
        rec_held: AtomicBool::new(false),

        cancel_keys_packed: AtomicU64::new(cancel_packed),
        cancel_key_count: AtomicU8::new(cancel_count),
        cancel_modifiers: AtomicU64::new(initial_cancel.modifiers),
        cancel_kind: AtomicU8::new(TapState::kind_to_u8(initial_cancel.kind)),

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

        let rec = state.load_rec_shortcut();
        let cancel = state.load_cancel_shortcut();

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
                if !cancel.is_disabled() {
                    let (cancel_p, cancel_c) = pack_keys(&cancel.key_codes);
                    match cancel.kind {
                        ShortcutKind::Combo => {
                            if packed_contains_all(pressed_p, pressed_c, cancel_p, cancel_c)
                                && (mod_flags & cancel.modifiers) == cancel.modifiers
                            {
                                let _ =
                                    state.event_tx.send(HotkeyEvent::CancelPressed);
                                return;
                            }
                        }
                        ShortcutKind::Key => {
                            if packed_contains_all(pressed_p, pressed_c, cancel_p, cancel_c)
                                && mod_flags == 0
                            {
                                let _ =
                                    state.event_tx.send(HotkeyEvent::CancelPressed);
                                return;
                            }
                        }
                        _ => {}
                    }
                }

                // Check record shortcut (Combo/Key)
                if !rec.is_disabled() {
                    let (rec_p, rec_c) = pack_keys(&rec.key_codes);
                    match rec.kind {
                        ShortcutKind::Combo => {
                            if packed_contains_all(pressed_p, pressed_c, rec_p, rec_c)
                                && (mod_flags & rec.modifiers) == rec.modifiers
                            {
                                if !state.rec_held.load(Ordering::SeqCst) {
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
                        }
                        ShortcutKind::Key => {
                            if packed_contains_all(pressed_p, pressed_c, rec_p, rec_c)
                                && mod_flags == 0
                            {
                                if !state.rec_held.load(Ordering::SeqCst) {
                                    state.rec_held.store(true, Ordering::SeqCst);
                                    log::debug!(
                                        "Hotkey KeyDown (Key): key=0x{:02x}",
                                        key_code
                                    );
                                    let _ =
                                        state.event_tx.send(HotkeyEvent::KeyDown);
                                }
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
                if !cancel.is_disabled()
                    && cancel.kind == ShortcutKind::ModifierOnly
                {
                    let (cancel_p, cancel_c) = pack_keys(&cancel.key_codes);
                    if packed_contains_all(
                        pressed_mod_p,
                        pressed_mod_c,
                        cancel_p,
                        cancel_c,
                    ) {
                        let _ = state.event_tx.send(HotkeyEvent::CancelPressed);
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
        use super::ffi;
        use std::os::raw::c_void;

        let tap = ffi::CGEventTapCreate(
            1,
            0,
            0,
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

            while let Ok(update) = update_rx.try_recv() {
                match update {
                    HotkeyUpdate::SetRecordShortcut(s) => {
                        log::info!(
                            "Record shortcut changed to {}",
                            s.display_string()
                        );
                        state.store_rec_shortcut(&s);
                    }
                    HotkeyUpdate::SetCancelShortcut(s) => {
                        log::info!(
                            "Cancel shortcut changed to {}",
                            s.display_string()
                        );
                        state.store_cancel_shortcut(&s);
                    }
                }
            }
        }
    }
}

#[cfg(not(target_os = "macos"))]
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
