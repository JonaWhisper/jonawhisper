//! Key labels for Windows: names spelled out, and virtual-key codes.

use super::super::{CG_MASK_ALTERNATE, CG_MASK_COMMAND, CG_MASK_CONTROL, CG_MASK_SHIFT};

/// Words need a separator; glyphs do not.
pub(crate) const MODIFIER_JOIN: &str = "+";

pub(crate) fn modifier_labels(flags: u64) -> Vec<&'static str> {
    let mut v = Vec::new();
    if flags & CG_MASK_CONTROL != 0 { v.push("Ctrl") }
    if flags & CG_MASK_ALTERNATE != 0 { v.push("Alt") }
    if flags & CG_MASK_SHIFT != 0 { v.push("Maj") }
    if flags & CG_MASK_COMMAND != 0 { v.push("Win") }
    v
}

pub(crate) fn modifier_only_label(key_code: u16) -> &'static str {
    match key_code {
        0x5C => "Right Win",
        0x5B => "Left Win",
        0xA5 => "Right Alt",
        0xA4 => "Left Alt",
        0xA3 => "Right Ctrl",
        0xA2 => "Left Ctrl",
        0xA1 => "Right Maj",
        0xA0 => "Left Maj",
        0x5D => "Menu",
        _ => "Win",
    }
}

/// Label for a regular (non-modifier) key code.
/// Windows virtual-key codes. Nothing is shared with the table above: 0x41 is
/// "A" here and "Pad ." on macOS, so a single table would mislabel every key.
pub(crate) fn key_code_label(key_code: u16) -> &'static str {
    match key_code {
        // Letters — VK_A..VK_Z are the ASCII codes
        0x41 => "A", 0x42 => "B", 0x43 => "C", 0x44 => "D",
        0x45 => "E", 0x46 => "F", 0x47 => "G", 0x48 => "H",
        0x49 => "I", 0x4A => "J", 0x4B => "K", 0x4C => "L",
        0x4D => "M", 0x4E => "N", 0x4F => "O", 0x50 => "P",
        0x51 => "Q", 0x52 => "R", 0x53 => "S", 0x54 => "T",
        0x55 => "U", 0x56 => "V", 0x57 => "W", 0x58 => "X",
        0x59 => "Y", 0x5A => "Z",
        // Digits
        0x30 => "0", 0x31 => "1", 0x32 => "2", 0x33 => "3",
        0x34 => "4", 0x35 => "5", 0x36 => "6", 0x37 => "7",
        0x38 => "8", 0x39 => "9",
        // F-keys
        0x70 => "F1", 0x71 => "F2", 0x72 => "F3", 0x73 => "F4",
        0x74 => "F5", 0x75 => "F6", 0x76 => "F7", 0x77 => "F8",
        0x78 => "F9", 0x79 => "F10", 0x7A => "F11", 0x7B => "F12",
        0x7C => "F13", 0x7D => "F14", 0x7E => "F15",
        // Editing and navigation
        0x1B => "Esc", 0x09 => "Tab", 0x20 => "Space", 0x0D => "Entree",
        0x08 => "Retour", 0x2E => "Suppr", 0x24 => "Origine", 0x23 => "Fin",
        0x21 => "Page haut", 0x22 => "Page bas", 0x2D => "Inser",
        0x25 => "Gauche", 0x26 => "Haut", 0x27 => "Droite", 0x28 => "Bas",
        // Punctuation (VK_OEM_*, US layout)
        0xBD => "-", 0xBB => "=", 0xDB => "[", 0xDD => "]",
        0xDC => "\\", 0xBA => ";", 0xDE => "'", 0xBC => ",",
        0xBE => ".", 0xBF => "/", 0xC0 => "`",
        // Numpad
        0x60 => "Pad 0", 0x61 => "Pad 1", 0x62 => "Pad 2",
        0x63 => "Pad 3", 0x64 => "Pad 4", 0x65 => "Pad 5",
        0x66 => "Pad 6", 0x67 => "Pad 7", 0x68 => "Pad 8",
        0x69 => "Pad 9", 0x6B => "Pad +", 0x6D => "Pad -",
        0x6A => "Pad *", 0x6F => "Pad /", 0x6E => "Pad .",
        // Modifier keys, when used as a shortcut of their own
        0x5C => "Right Win", 0x5B => "Left Win",
        0xA5 => "Right Alt", 0xA4 => "Left Alt",
        0xA3 => "Right Ctrl", 0xA2 => "Left Ctrl",
        0xA1 => "Right Maj", 0xA0 => "Left Maj",
        0x5D => "Menu",
        _ => "?",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn letters_use_virtual_key_codes() {
        assert_eq!(key_code_label(0x41), "A");
        assert_eq!(key_code_label(0x5A), "Z");
        // 0x00 is "A" on macOS; here it means nothing.
        assert_eq!(key_code_label(0x00), "?");
    }

    #[test]
    fn no_label_carries_an_apple_glyph() {
        let apple = ['\u{2318}', '\u{2325}', '\u{2303}', '\u{21e7}'];
        for code in 0u16..=0xFF {
            let label = key_code_label(code);
            assert!(
                !label.chars().any(|c| apple.contains(&c)),
                "0x{code:02X} rend {label:?}"
            );
            let modifier = modifier_only_label(code);
            assert!(
                !modifier.chars().any(|c| apple.contains(&c)),
                "modificateur 0x{code:02X} rend {modifier:?}"
            );
        }
    }

    #[test]
    fn modifiers_are_spelled_out() {
        assert_eq!(modifier_labels(CG_MASK_CONTROL), vec!["Ctrl"]);
        assert_eq!(modifier_labels(CG_MASK_COMMAND), vec!["Win"]);
        assert_eq!(MODIFIER_JOIN, "+");
    }
}
