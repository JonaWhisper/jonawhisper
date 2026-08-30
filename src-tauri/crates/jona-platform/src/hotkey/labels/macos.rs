//! Key labels for macOS: Apple glyphs, and CGEvent key codes.

use super::super::{CG_MASK_ALTERNATE, CG_MASK_COMMAND, CG_MASK_CONTROL, CG_MASK_SHIFT};

/// Glyphs concatenate with nothing between them.
pub(crate) const MODIFIER_JOIN: &str = "";

pub(crate) fn modifier_labels(flags: u64) -> Vec<&'static str> {
    let mut v = Vec::new();
    if flags & CG_MASK_CONTROL != 0 { v.push("\u{2303}") }
    if flags & CG_MASK_ALTERNATE != 0 { v.push("\u{2325}") }
    if flags & CG_MASK_SHIFT != 0 { v.push("\u{21e7}") }
    if flags & CG_MASK_COMMAND != 0 { v.push("\u{2318}") }
    v
}

pub(crate) fn modifier_only_label(key_code: u16) -> &'static str {
    match key_code {
        0x36 => "Right \u{2318}",
        0x37 => "Left \u{2318}",
        0x3D => "Right \u{2325}",
        0x3A => "Left \u{2325}",
        0x3E => "Right \u{2303}",
        0x3B => "Left \u{2303}",
        0x3C => "Right \u{21e7}",
        0x38 => "Left \u{21e7}",
        0x3F => "Fn",
        _ => "\u{2318}",
    }
}

pub(crate) fn key_code_label(key_code: u16) -> &'static str {
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
