//! Platform services.
//!
//! Every module here presents one API and picks its implementation from the
//! target, so no caller branches on the operating system.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PermissionStatus {
    Granted,
    Denied,
    Undetermined,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionReport {
    pub microphone: PermissionStatus,
    pub accessibility: PermissionStatus,
    pub input_monitoring: PermissionStatus,
}

impl PermissionReport {
    pub fn all_granted(&self) -> bool {
        self.microphone == PermissionStatus::Granted
            && self.accessibility == PermissionStatus::Granted
            && self.input_monitoring == PermissionStatus::Granted
    }
}

pub mod audio_devices;
pub mod audio_ducking;
pub mod hotkey;
pub mod paste;

#[cfg(target_os = "macos")]
pub mod ffi;

// Permissions, system cues and launch at login. Private modules behind a
// re-export: the name of the platform serving them is nobody else's business.

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::*;

#[cfg(target_os = "windows")]
mod device_watcher;
#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
pub use windows::*;

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
mod fallback;
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub use fallback::*;
