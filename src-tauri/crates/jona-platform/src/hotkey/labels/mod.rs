//! Key labels. The two platforms share no code here: Apple has glyphs and
//! CGEvent codes, Windows has names and virtual-key codes, and 0x41 means a
//! different key on each.

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub(crate) use macos::{MODIFIER_JOIN, key_code_label, modifier_labels, modifier_only_label};

#[cfg(not(target_os = "macos"))]
mod windows;
#[cfg(not(target_os = "macos"))]
pub(crate) use windows::{MODIFIER_JOIN, key_code_label, modifier_labels, modifier_only_label};
