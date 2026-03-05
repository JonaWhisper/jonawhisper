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
pub mod ffi;

#[cfg(target_os = "macos")]
pub mod macos;

#[cfg(target_os = "macos")]
pub mod audio_devices;

#[cfg(target_os = "macos")]
pub mod audio_ducking;

pub mod hotkey;
pub mod paste;

#[cfg(target_os = "macos")]
pub use macos::*;

#[cfg(not(target_os = "macos"))]
pub mod audio_devices {
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub enum AudioTransportType { Unknown }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct AudioDevice {
        pub id: u32,
        pub name: String,
        pub uid: String,
        pub transport_type: AudioTransportType,
        pub is_default: bool,
    }

    pub fn list_input_devices() -> Vec<AudioDevice> { vec![] }
    pub fn start_device_change_listener(_callback: impl Fn() + Send + 'static) {}
}

#[cfg(not(target_os = "macos"))]
pub mod audio_ducking {
    pub fn duck_volume(_ratio: f32) {}
    pub fn restore_volume() {}
}

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
