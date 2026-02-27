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

#[cfg(target_os = "macos")]
pub mod macos;

pub mod hotkey;
pub mod paste;

#[cfg(target_os = "macos")]
pub use macos::*;

#[cfg(not(target_os = "macos"))]
pub mod stub {
    use super::{PermissionReport, PermissionStatus};

    impl Default for PermissionReport {
        fn default() -> Self {
            Self {
                microphone: PermissionStatus::Granted,
                accessibility: PermissionStatus::Granted,
                input_monitoring: PermissionStatus::Granted,
            }
        }
    }

    pub fn check_permissions() -> PermissionReport {
        PermissionReport::default()
    }

    pub fn request_permission(_kind: &str) -> bool {
        true
    }

    pub fn play_sound(_name: &str) {}
}

#[cfg(not(target_os = "macos"))]
pub use stub::*;
