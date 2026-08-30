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

/// The tables above, handed to the frontend so it stops keeping its own copy —
/// which was Apple's, on every platform.
pub fn table() -> KeyLabels {
    let mut keys = std::collections::BTreeMap::new();
    for code in 0u16..=0xFF {
        let label = key_code_label(code);
        if label != "?" {
            keys.insert(code, label.to_string());
        }
    }
    KeyLabels {
        keys,
        modifier_join: MODIFIER_JOIN.to_string(),
        control: modifier_labels(super::CG_MASK_CONTROL).join(""),
        alternate: modifier_labels(super::CG_MASK_ALTERNATE).join(""),
        shift: modifier_labels(super::CG_MASK_SHIFT).join(""),
        command: modifier_labels(super::CG_MASK_COMMAND).join(""),
    }
}

#[derive(serde::Serialize)]
pub struct KeyLabels {
    /// Key code to label, omitting the codes that mean nothing on this platform.
    pub keys: std::collections::BTreeMap<u16, String>,
    pub modifier_join: String,
    pub control: String,
    pub alternate: String,
    pub shift: String,
    pub command: String,
}
