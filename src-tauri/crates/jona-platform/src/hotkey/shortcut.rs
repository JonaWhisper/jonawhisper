//! Shortcut values: what a shortcut is, how it reads on screen, and how its
//! keys pack into an atomic. No platform call — only the key codes differ, and
//! those come from `super::keys`.

use super::keys;
use super::{
    CG_MASK_ALTERNATE, CG_MASK_COMMAND, CG_MASK_CONTROL, CG_MASK_SHIFT, MASK_ALTERNATE,
    MASK_COMMAND, MASK_CONTROL, MASK_SHIFT,
};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering};

pub(super) const MAX_SHORTCUT_KEYS: usize = 4;

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
            "right_command" => Self::modifier(keys::RIGHT_COMMAND, MASK_COMMAND),
            "right_option" => Self::modifier(keys::RIGHT_OPTION, MASK_ALTERNATE),
            "right_control" => Self::modifier(keys::RIGHT_CONTROL, MASK_CONTROL),
            "right_shift" => Self::modifier(keys::RIGHT_SHIFT, MASK_SHIFT),
            "escape" => Self {
                key_codes: vec![keys::ESCAPE],
                modifiers: 0,
                kind: ShortcutKind::Key,
            },
            "none" | "" => Self::disabled(),
            _ => Self::modifier(keys::RIGHT_COMMAND, MASK_COMMAND),
        }
    }

    fn modifier(key_code: u16, modifiers: u64) -> Self {
        Self {
            key_codes: vec![key_code],
            modifiers,
            kind: ShortcutKind::ModifierOnly,
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
pub(super) fn is_modifier_key_code(key_code: u16) -> bool {
    modifier_flag_for_key_code(key_code) != 0 || key_code == keys::EXTRA_MODIFIER
}

/// Get the modifier flag for a specific modifier key code.
pub(super) fn modifier_flag_for_key_code(key_code: u16) -> u64 {
    match key_code {
        keys::RIGHT_COMMAND | keys::LEFT_COMMAND => MASK_COMMAND,
        keys::RIGHT_OPTION | keys::LEFT_OPTION => MASK_ALTERNATE,
        keys::RIGHT_CONTROL | keys::LEFT_CONTROL => MASK_CONTROL,
        keys::RIGHT_SHIFT | keys::LEFT_SHIFT => MASK_SHIFT,
        _ => 0,
    }
}

// -- Packed key helpers --
// Pack up to 4 u16 key codes into a single u64 for lock-free atomic access.
// The CGEvent callback is single-threaded, so load-modify-store is safe.

pub(super) fn pack_keys(keys: &[u16]) -> (u64, u8) {
    let count = keys.len().min(MAX_SHORTCUT_KEYS);
    let mut packed: u64 = 0;
    for (i, &k) in keys[..count].iter().enumerate() {
        packed |= (k as u64) << (i * 16);
    }
    (packed, count as u8)
}

pub(super) fn unpack_keys(packed: u64, count: u8) -> Vec<u16> {
    (0..count as usize)
        .map(|i| ((packed >> (i * 16)) & 0xFFFF) as u16)
        .collect()
}

pub(super) fn packed_contains(packed: u64, count: u8, key: u16) -> bool {
    for i in 0..count as usize {
        if ((packed >> (i * 16)) & 0xFFFF) as u16 == key {
            return true;
        }
    }
    false
}

/// Add a key to a packed set (deduplicated). Returns true if added.
pub(super) fn packed_add(packed: &AtomicU64, count: &AtomicU8, key: u16) -> bool {
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
pub(super) fn packed_remove(packed: &AtomicU64, count: &AtomicU8, key: u16) -> bool {
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
pub(super) fn packed_contains_all(
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

impl Default for CaptureControl {
    fn default() -> Self {
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
}

impl CaptureControl {
    pub fn new() -> Self {
        Self::default()
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

    #[cfg(target_os = "macos")]
    pub(super) fn reset(&self) {
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


#[cfg(test)]
mod key_name_tests {
    use super::*;

    /// The stored name is portable; the code it resolves to is not. On Windows,
    /// macOS's 0x36 is the digit 6, so a shared table would bind the default
    /// shortcut to a character key.
    #[test]
    fn names_resolve_to_this_platform_codes() {
        let cmd = Shortcut::parse("right_command");
        assert_eq!(cmd.kind, ShortcutKind::ModifierOnly);
        assert_eq!(cmd.key_codes, vec![keys::RIGHT_COMMAND]);

        let esc = Shortcut::parse("escape");
        assert_eq!(esc.kind, ShortcutKind::Key);
        assert_eq!(esc.key_codes, vec![keys::ESCAPE]);
    }

    #[test]
    fn every_modifier_name_is_distinct() {
        let codes = [
            keys::RIGHT_COMMAND,
            keys::RIGHT_OPTION,
            keys::RIGHT_CONTROL,
            keys::RIGHT_SHIFT,
            keys::ESCAPE,
        ];
        let mut seen = codes.to_vec();
        seen.sort_unstable();
        seen.dedup();
        assert_eq!(seen.len(), codes.len(), "deux noms partagent un code");
    }

    #[test]
    fn unknown_names_fall_back_to_the_default_shortcut() {
        assert_eq!(
            Shortcut::parse("this-is-not-a-shortcut").key_codes,
            Shortcut::parse("right_command").key_codes
        );
    }

    #[test]
    fn none_and_empty_are_disabled() {
        assert!(Shortcut::parse("none").is_disabled());
        assert!(Shortcut::parse("").is_disabled());
    }
}

