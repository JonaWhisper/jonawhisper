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

    // Same variants as the macOS enum: shared code (menu_icons) matches on all
    // of them, and USB/Bluetooth/HDMI are not macOS concepts.
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

    /// Enumerate inputs through cpal rather than a native API. cpal already
    /// talks to WASAPI here, and CoreAudio's extras — a numeric id, a stable
    /// uid, a transport type — have no equivalent worth the FFI: the device name
    /// is what Windows itself shows the user.
    pub fn list_input_devices() -> Vec<AudioDevice> {
        use cpal::traits::{DeviceTrait, HostTrait};

        let host = cpal::default_host();
        let default_name = host
            .default_input_device()
            .and_then(|d| d.description().ok())
            .map(|desc| desc.name().to_string());

        let Ok(devices) = host.input_devices() else {
            log::warn!("Audio devices: cpal could not enumerate inputs");
            return vec![];
        };

        devices
            .enumerate()
            .filter_map(|(i, d)| {
                let name = d.description().ok()?.name().to_string();
                Some(AudioDevice {
                    id: i as u32,
                    is_default: Some(&name) == default_name.as_ref(),
                    // No stable identifier here: the name is what the picker
                    // stores and matches on later.
                    uid: name.clone(),
                    name,
                    transport_type: AudioTransportType::Unknown,
                })
            })
            .collect()
    }

    /// No device-change notification: WASAPI exposes one through
    /// IMMNotificationClient, which needs COM plumbing this does not have yet.
    /// The list is rebuilt whenever the panel opens, so a change is picked up
    /// the next time the user looks.
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

    pub fn get_launch_at_login_status() -> &'static str {
        "disabled"
    }

    pub fn set_launch_at_login(_enabled: bool) -> Result<&'static str, String> {
        Err("Launch at login not supported on this platform".to_string())
    }
}

#[cfg(not(target_os = "macos"))]
pub use stub::*;
