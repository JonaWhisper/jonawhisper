//! Audio input devices: one shape of data, one enumeration per platform.
//!
//! The types live here rather than beside each implementation, which used to
//! keep a copy — a variant could be added to one and not the other.

use serde::{Deserialize, Serialize};

/// How a device is attached. The names come from CoreAudio's transport types,
/// which is also the vocabulary the tray icons speak.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[allow(clippy::upper_case_acronyms)]
pub enum AudioTransportType {
    BuiltIn,
    USB,
    Bluetooth,
    Virtual,
    Aggregate,
    Thunderbolt,
    HDMI,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioDevice {
    pub id: u32,
    pub name: String,
    pub uid: String,
    pub transport_type: AudioTransportType,
    pub is_default: bool,
}

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::{list_input_devices, start_device_change_listener};

#[cfg(not(target_os = "macos"))]
mod portable;
#[cfg(not(target_os = "macos"))]
pub use portable::{list_input_devices, start_device_change_listener};
