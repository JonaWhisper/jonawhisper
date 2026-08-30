//! The atomics a shortcut becomes once the monitor is watching it, and the
//! matching that reads them. Kept off the platform backends: both need it, and
//! it is where a subtle mistake would go unnoticed.

use super::shortcut::{
    Shortcut, ShortcutKind, pack_keys, packed_contains_all, unpack_keys,
};
use std::sync::atomic::{AtomicU8, AtomicU64, Ordering};

/// One shortcut, stored as atomics so the CGEvent callback can read it without
/// locking. Every shortcut the tap watches uses this — duplicating the four
/// fields per shortcut is what made adding a third one expensive.
pub(super) struct ShortcutSlot {
    pub(super) keys_packed: AtomicU64,
    pub(super) key_count: AtomicU8,
    pub(super) modifiers: AtomicU64,
    pub(super) kind: AtomicU8, // 0=ModifierOnly, 1=Combo, 2=Key
}

impl ShortcutSlot {
    pub(super) fn new(s: &Shortcut) -> Self {
        let (packed, count) = pack_keys(&s.key_codes);
        Self {
            keys_packed: AtomicU64::new(packed),
            key_count: AtomicU8::new(count),
            modifiers: AtomicU64::new(s.modifiers),
            kind: AtomicU8::new(kind_to_u8(s.kind)),
        }
    }

    pub(super) fn load(&self) -> Shortcut {
        Shortcut {
            key_codes: unpack_keys(
                self.keys_packed.load(Ordering::SeqCst),
                self.key_count.load(Ordering::SeqCst),
            ),
            modifiers: self.modifiers.load(Ordering::SeqCst),
            kind: u8_to_kind(self.kind.load(Ordering::SeqCst)),
        }
    }

    pub(super) fn store(&self, s: &Shortcut) {
        let (packed, count) = pack_keys(&s.key_codes);
        self.keys_packed.store(packed, Ordering::SeqCst);
        self.key_count.store(count, Ordering::SeqCst);
        self.modifiers.store(s.modifiers, Ordering::SeqCst);
        self.kind.store(kind_to_u8(s.kind), Ordering::SeqCst);
    }
}

/// Does this shortcut fire for the keys currently down? Combo also requires its
/// modifiers; a plain Key requires none, so it cannot swallow Cmd+key.
pub(super) fn shortcut_matches(shortcut: &Shortcut, pressed_p: u64, pressed_c: u8, mod_flags: u64) -> bool {
    if shortcut.is_disabled() {
        return false;
    }
    let (want_p, want_c) = pack_keys(&shortcut.key_codes);
    if !packed_contains_all(pressed_p, pressed_c, want_p, want_c) {
        return false;
    }
    match shortcut.kind {
        ShortcutKind::Combo => (mod_flags & shortcut.modifiers) == shortcut.modifiers,
        ShortcutKind::Key => mod_flags == 0,
        ShortcutKind::ModifierOnly => false,
    }
}

/// ModifierOnly shortcuts match against the modifier keys held, not the regular ones.
pub(super) fn modifier_shortcut_matches(shortcut: &Shortcut, pressed_p: u64, pressed_c: u8) -> bool {
    if shortcut.is_disabled() || shortcut.kind != ShortcutKind::ModifierOnly {
        return false;
    }
    let (want_p, want_c) = pack_keys(&shortcut.key_codes);
    packed_contains_all(pressed_p, pressed_c, want_p, want_c)
}

pub(super) fn kind_to_u8(kind: ShortcutKind) -> u8 {
    match kind {
        ShortcutKind::ModifierOnly => 0,
        ShortcutKind::Combo => 1,
        ShortcutKind::Key => 2,
    }
}

pub(super) fn u8_to_kind(v: u8) -> ShortcutKind {
    match v {
        0 => ShortcutKind::ModifierOnly,
        1 => ShortcutKind::Combo,
        _ => ShortcutKind::Key,
    }
}


#[cfg(test)]
mod slot_tests {
    use super::*;

    pub(super) fn combo(keys: &[u16], modifiers: u64) -> Shortcut {
        Shortcut { key_codes: keys.to_vec(), modifiers, kind: ShortcutKind::Combo }
    }

    pub(super) fn plain_key(keys: &[u16]) -> Shortcut {
        Shortcut { key_codes: keys.to_vec(), modifiers: 0, kind: ShortcutKind::Key }
    }

    pub(super) fn modifier_only(keys: &[u16]) -> Shortcut {
        Shortcut { key_codes: keys.to_vec(), modifiers: 0, kind: ShortcutKind::ModifierOnly }
    }

    pub(super) fn pressed(keys: &[u16]) -> (u64, u8) {
        pack_keys(keys)
    }

    const CMD: u64 = 1 << 20;
    const SHIFT: u64 = 1 << 17;

    #[test]
    pub(super) fn combo_needs_both_its_keys_and_its_modifiers() {
        let s = combo(&[9], CMD);
        let (p, c) = pressed(&[9]);
        assert!(shortcut_matches(&s, p, c, CMD));
        assert!(!shortcut_matches(&s, p, c, 0), "sans le modificateur");
        let (p2, c2) = pressed(&[10]);
        assert!(!shortcut_matches(&s, p2, c2, CMD), "mauvaise touche");
    }

    #[test]
    pub(super) fn combo_tolerates_extra_modifiers() {
        let s = combo(&[9], CMD);
        let (p, c) = pressed(&[9]);
        assert!(shortcut_matches(&s, p, c, CMD | SHIFT));
    }

    /// A bare key must not fire as part of a chord, or Escape alone would also
    /// trigger on Cmd+Escape.
    #[test]
    pub(super) fn plain_key_refuses_any_modifier() {
        let s = plain_key(&[53]);
        let (p, c) = pressed(&[53]);
        assert!(shortcut_matches(&s, p, c, 0));
        assert!(!shortcut_matches(&s, p, c, CMD));
    }

    #[test]
    pub(super) fn modifier_only_never_matches_the_regular_key_path() {
        let s = modifier_only(&[54]);
        let (p, c) = pressed(&[54]);
        assert!(!shortcut_matches(&s, p, c, CMD));
        assert!(modifier_shortcut_matches(&s, p, c));
    }

    #[test]
    pub(super) fn a_disabled_shortcut_matches_nothing() {
        let s = Shortcut::disabled();
        let (p, c) = pressed(&[9]);
        assert!(!shortcut_matches(&s, p, c, CMD));
        assert!(!modifier_shortcut_matches(&s, p, c));
    }

    #[test]
    pub(super) fn slot_round_trips_every_kind() {
        for original in [combo(&[9, 10], CMD), plain_key(&[53]), modifier_only(&[54])] {
            let slot = ShortcutSlot::new(&original);
            let back = slot.load();
            assert_eq!(back.key_codes, original.key_codes);
            assert_eq!(back.modifiers, original.modifiers);
            assert_eq!(back.kind, original.kind);
        }
    }

    #[test]
    pub(super) fn storing_over_a_slot_replaces_it() {
        let slot = ShortcutSlot::new(&plain_key(&[53]));
        slot.store(&combo(&[9], CMD));
        let back = slot.load();
        assert_eq!(back.key_codes, vec![9]);
        assert_eq!(back.modifiers, CMD);
        assert_eq!(back.kind, ShortcutKind::Combo);
    }
}

